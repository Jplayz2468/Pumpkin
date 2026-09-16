use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use rustc_hash::FxHashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum ModifierOperation {
    Add = 0,           // add value
    MultiplyBase = 1,  // multiply base (base * (1 + x))
    MultiplyTotal = 2, // multiply total (applied last)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub id: String,
    pub amount: f64,
    pub operation: ModifierOperation,
}

/// Per-entity attribute instance used at runtime.
#[derive(Debug)]
pub struct AttributeInstance {
    pub base_value: f64,
    pub modifiers: Vec<Modifier>,
    permanent_modifiers: std::collections::BTreeSet<String>,
    pub cached_value: AtomicU64,
    pub dirty: AtomicBool,
}

impl AttributeInstance {
    #[must_use]
    pub const fn new(base_value: f64) -> Self {
        Self {
            base_value,
            modifiers: Vec::new(),
            permanent_modifiers: std::collections::BTreeSet::new(),
            cached_value: AtomicU64::new(base_value.to_bits()),
            dirty: AtomicBool::new(false),
        }
    }

    pub fn set_base_value(&mut self, value: f64) {
        // Java's equality check keeps the existing sign when replacing +0 with -0.
        if self.base_value != value {
            self.base_value = value;
            self.dirty.store(true, Ordering::Relaxed);
        }
    }

    pub fn value(&self) -> f64 {
        if !self.dirty.load(Ordering::Relaxed) {
            return f64::from_bits(self.cached_value.load(Ordering::Relaxed));
        }

        let mut value = self.base_value;

        // Java applies each operation in sequence; collecting sums/products first
        // changes floating-point rounding and overflow behavior.
        for modifier in &self.modifiers {
            if modifier.operation == ModifierOperation::Add {
                value += modifier.amount;
            }
        }
        let base = value;
        for modifier in &self.modifiers {
            if modifier.operation == ModifierOperation::MultiplyBase {
                value += base * modifier.amount;
            }
        }
        for modifier in &self.modifiers {
            if modifier.operation == ModifierOperation::MultiplyTotal {
                value *= 1.0 + modifier.amount;
            }
        }

        self.cached_value.store(value.to_bits(), Ordering::Relaxed);
        self.dirty.store(false, Ordering::Relaxed);

        value
    }

    pub fn add_or_replace_modifier(&mut self, modifier: Modifier) {
        if let Some(pos) = self.modifiers.iter().position(|m| m.id == modifier.id) {
            self.modifiers.remove(pos);
        }
        self.modifiers.push(modifier);
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn add_permanent_modifier(&mut self, modifier: Modifier) {
        self.permanent_modifiers.insert(modifier.id.clone());
        self.add_or_replace_modifier(modifier);
    }

    pub fn write_nbt(&self, name: &str) -> pumpkin_nbt::compound::NbtCompound {
        use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
        let mut result = NbtCompound::new();
        result.put_string("id", name.to_owned());
        result.put_double("base", self.base_value);
        let modifiers: Vec<_> = self
            .modifiers
            .iter()
            .filter(|m| self.permanent_modifiers.contains(&m.id))
            .map(|m| {
                let mut data = NbtCompound::new();
                data.put_string("id", m.id.clone());
                data.put_double("amount", m.amount);
                data.put_string(
                    "operation",
                    match m.operation {
                        ModifierOperation::Add => "add_value",
                        ModifierOperation::MultiplyBase => "add_multiplied_base",
                        ModifierOperation::MultiplyTotal => "add_multiplied_total",
                    }
                    .to_owned(),
                );
                NbtTag::Compound(data)
            })
            .collect();
        if !modifiers.is_empty() {
            result.put("modifiers", NbtTag::List(modifiers));
        }
        result
    }

    pub fn read_nbt(&mut self, data: &pumpkin_nbt::compound::NbtCompound) {
        self.base_value = data
            .get("base")
            .and_then(super::nbt_number::double)
            .unwrap_or(0.0);
        if let Some(modifiers) = data.get_list("modifiers") {
            for tag in modifiers {
                let Some(m) = tag.extract_compound() else {
                    continue;
                };
                let (Some(id), Some(amount), Some(operation)) = (
                    m.get_string("id"),
                    m.get("amount").and_then(super::nbt_number::double),
                    m.get_string("operation"),
                ) else {
                    continue;
                };
                let operation = match operation {
                    "add_value" => ModifierOperation::Add,
                    "add_multiplied_base" => ModifierOperation::MultiplyBase,
                    "add_multiplied_total" => ModifierOperation::MultiplyTotal,
                    _ => continue,
                };
                self.add_permanent_modifier(Modifier {
                    id: id.to_owned(),
                    amount,
                    operation,
                });
            }
        }
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn remove_modifier(&mut self, id: &str) {
        self.permanent_modifiers.remove(id);
        if let Some(pos) = self.modifiers.iter().position(|m| m.id == id) {
            self.modifiers.swap_remove(pos);
        }
        self.dirty.store(true, Ordering::Relaxed);
    }
}

/// Send updates for multiple attributes in a single packet for the given living entity.
pub fn send_attribute_updates_for_living(
    living: &crate::entity::living::LivingEntity,
    attributes: Vec<Attributes>,
) {
    use pumpkin_protocol::bedrock::client::update_attributes::{
        AttributeData as BeAttribute, CUpdateAttributes as BePacket,
    };
    use pumpkin_protocol::codec::var_int::VarInt;
    use pumpkin_protocol::codec::var_ulong::VarULong;
    use pumpkin_protocol::java::client::play::AttributeModifier as JeAttrMod;
    use pumpkin_protocol::java::client::play::CUpdateAttributes as JePacket;
    use pumpkin_protocol::java::client::play::Property as JeProperty;

    let mut je_properties: Vec<JeProperty> = Vec::with_capacity(attributes.len());
    let mut be_attributes: Vec<BeAttribute> = Vec::with_capacity(attributes.len());

    for attribute in attributes {
        let base_value = living.get_attribute_base(&attribute);
        let effective_value = living.get_attribute_value(&attribute);

        // Pull modifiers for this attribute
        let mut modifiers = Vec::new();
        if let Some(inst) = living
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&attribute.id)
        {
            for mod_inst in &inst.modifiers {
                modifiers.push(JeAttrMod::new(
                    mod_inst.id.clone(),
                    mod_inst.amount,
                    mod_inst.operation as i8,
                ));
            }
        }

        // Move modifiers into the property
        je_properties.push(JeProperty::new(
            VarInt(i32::from(attribute.id)),
            base_value,
            modifiers,
        ));

        let name = match attribute.id {
            id if id == Attributes::MOVEMENT_SPEED.id => "minecraft:movement".to_string(),
            id if id == Attributes::MAX_HEALTH.id => "minecraft:health".to_string(),
            id if id == Attributes::MAX_ABSORPTION.id => "minecraft:absorption".to_string(),
            id if id == Attributes::ATTACK_DAMAGE.id => "minecraft:attack_damage".to_string(),
            id if id == Attributes::KNOCKBACK_RESISTANCE.id => {
                "minecraft:knockback_resistance".to_string()
            }
            id if id == Attributes::LUCK.id => "minecraft:luck".to_string(),
            id if id == Attributes::FOLLOW_RANGE.id => "minecraft:follow_range".to_string(),
            id if id == Attributes::JUMP_STRENGTH.id => "minecraft:horse.jump_strength".to_string(),
            // Java-only attributes must not be sent under unsupported Bedrock names.
            _ => continue,
        };

        let be_attribute = BeAttribute {
            min_value: 0.0,
            max_value: 3.402_823_5E38,
            current_value: effective_value as f32,
            default_min_value: 0.0,
            default_max_value: 3.402_823_5E38,
            default_value: base_value as f32,
            name,
            // Bedrock receives the already-computed effective value above. Do not advertise
            // modifier entries until their payload is encoded as well.
            modifiers: Vec::new(),
        };

        be_attributes.push(be_attribute);
    }

    let je_packet = JePacket::new(living.entity.entity_id.into(), je_properties);
    let world = living.entity.world.load();
    if be_attributes.is_empty() {
        world.broadcast_packet_all(&je_packet);
        return;
    }

    let runtime_id = living.entity.entity_id as u64;
    let be_packet = BePacket {
        target_runtime_id: VarULong(runtime_id),
        attribute_list: be_attributes,
        tick: VarULong(0),
    };

    world.broadcast_editioned(&je_packet, &be_packet);
}

impl Clone for AttributeInstance {
    fn clone(&self) -> Self {
        Self {
            base_value: self.base_value,
            modifiers: self.modifiers.clone(),
            permanent_modifiers: self.permanent_modifiers.clone(),
            cached_value: AtomicU64::new(self.cached_value.load(Ordering::Relaxed)),
            dirty: AtomicBool::new(self.dirty.load(Ordering::Relaxed)),
        }
    }
}

/// Registry storing per-entity-type base attribute overrides.
/// Internally stores a map from `entity_type.id` -> `FxHashMap`<attribute.id, f64> for O(1) lookup.
#[derive(Default)]
pub struct AttributeRegistry {
    map: FxHashMap<u16, FxHashMap<u8, f64>>,
}

impl AttributeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the base value for `attribute` for the given entity type id.
    /// If no override exists, returns `attribute.default_value`.
    #[must_use]
    pub fn get_base_value(&self, entity_type_id: u16, attribute: &Attributes) -> f64 {
        self.map
            .get(&entity_type_id)
            .and_then(|map| map.get(&attribute.id))
            .copied()
            .unwrap_or(attribute.default_value)
    }

    /// Return a vector of overrides for the given entity type id.
    /// This allows populating per-entity local attribute instances at spawn time.
    #[must_use]
    pub fn get_overrides_for_entity(&self, entity_type_id: u16) -> Option<Vec<(u8, f64)>> {
        self.map
            .get(&entity_type_id)
            .map(|m| m.iter().map(|(&k, &v)| (k, v)).collect())
    }
}

/// Builder to declaratively assemble attribute overrides for an entity type.
#[derive(Default)]
pub struct AttributeBuilder {
    entries: Vec<(Attributes, f64)>,
}

impl AttributeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn add(mut self, attribute: Attributes, base: f64) -> Self {
        self.entries.push((attribute, base));
        self
    }

    #[must_use]
    pub fn build(self) -> Vec<(Attributes, f64)> {
        self.entries
    }
}

impl AttributeRegistry {
    /// Register overrides created by an `AttributeBuilder` for `entity_type`.
    pub fn register_builder(
        &mut self,
        entity_type: &'static EntityType,
        builder: AttributeBuilder,
    ) {
        let inner = self.map.entry(entity_type.id).or_default();
        for (attr, val) in builder.build() {
            inner.insert(attr.id, val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::attributes::Attributes;
    use pumpkin_data::entity::EntityType;

    #[test]
    fn permanent_spawn_bonus_survives_nbt_without_baby_speed_duplication() {
        let mut original = AttributeInstance::new(0.23);
        original.add_permanent_modifier(Modifier {
            id: "minecraft:random_spawn_bonus".into(),
            amount: 0.05,
            operation: ModifierOperation::MultiplyBase,
        });
        original.add_or_replace_modifier(Modifier {
            id: "minecraft:baby".into(),
            amount: 0.5,
            operation: ModifierOperation::MultiplyBase,
        });
        let saved = original.write_nbt("minecraft:movement_speed");
        assert_eq!(saved.get_list("modifiers").unwrap().len(), 1);
        let mut restored = AttributeInstance::new(0.0);
        restored.read_nbt(&saved);
        assert_eq!(restored.base_value, 0.23);
        assert_eq!(restored.modifiers.len(), 1);
        restored.add_or_replace_modifier(Modifier {
            id: "minecraft:baby".into(),
            amount: 0.5,
            operation: ModifierOperation::MultiplyBase,
        });
        assert_eq!(restored.value(), original.value());
        restored.remove_modifier("minecraft:random_spawn_bonus");
        assert!(
            restored
                .write_nbt("minecraft:movement_speed")
                .get_list("modifiers")
                .is_none()
        );
    }

    #[test]
    fn player_base_attributes() {
        let speed_attr = EntityType::PLAYER
            .attributes
            .iter()
            .find(|(attr, _)| attr.id == Attributes::MOVEMENT_SPEED.id);
        assert!(speed_attr.is_some());
        let (_, base_speed) = speed_attr.unwrap();
        assert!((base_speed - 0.1).abs() < 1e-4);
    }

    #[test]
    fn sprinting_modifier_calculation() {
        let mut instance = AttributeInstance::new(0.1);
        assert!((instance.value() - 0.1).abs() < f64::EPSILON);

        let sprinting_mod = Modifier {
            id: "minecraft:sprinting".to_string(),
            amount: 0.300_000_011_920_928_96,
            operation: ModifierOperation::MultiplyTotal,
        };

        instance.add_or_replace_modifier(sprinting_mod);
        let sprinting_value = instance.value();
        // 0.1 * (1.0 + 0.30000001192092896) = 0.1300000011920929
        assert!((sprinting_value - 0.130_000_001_192_092_9).abs() < 1e-9);

        instance.remove_modifier("minecraft:sprinting");
        assert!((instance.value() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn attribute_modifier_operations() {
        let mut instance = AttributeInstance::new(10.0);

        // ADD: 10.0 + 2.0 + 3.0 = 15.0
        instance.add_or_replace_modifier(Modifier {
            id: "add_1".to_string(),
            amount: 2.0,
            operation: ModifierOperation::Add,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "add_2".to_string(),
            amount: 3.0,
            operation: ModifierOperation::Add,
        });
        assert!((instance.value() - 15.0).abs() < f64::EPSILON);

        // MULTIPLY_BASE: 15.0 * (1.0 + 0.5 + 0.2) = 15.0 * 1.7 = 25.5
        instance.add_or_replace_modifier(Modifier {
            id: "mul_base_1".to_string(),
            amount: 0.5,
            operation: ModifierOperation::MultiplyBase,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "mul_base_2".to_string(),
            amount: 0.2,
            operation: ModifierOperation::MultiplyBase,
        });
        assert!((instance.value() - 25.5).abs() < f64::EPSILON);

        // MULTIPLY_TOTAL: 25.5 * (1.0 + 0.1) * (1.0 + 0.2) = 25.5 * 1.1 * 1.2 = 33.66
        instance.add_or_replace_modifier(Modifier {
            id: "mul_total_1".to_string(),
            amount: 0.1,
            operation: ModifierOperation::MultiplyTotal,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "mul_total_2".to_string(),
            amount: 0.2,
            operation: ModifierOperation::MultiplyTotal,
        });
        assert!((instance.value() - 33.66).abs() < 1e-9);
    }
}
