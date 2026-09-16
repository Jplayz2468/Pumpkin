use pumpkin_data::item::Item;
use pumpkin_data::particle::Particle;
use pumpkin_data::potion::Effect;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data;
use pumpkin_inventory::build_equipment_slots;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_protocol::bedrock::client::take_item_actor::CTakeItemActor;
use pumpkin_protocol::bedrock::server::actor_event::{ActorEventID, SActorEvent};
use pumpkin_protocol::codec::var_ulong::VarULong;
use pumpkin_util::GameMode;
use pumpkin_util::Hand;
use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicU8,
    Ordering::{Relaxed, SeqCst},
};

use super::effect_instance::EffectInstance;
use super::experience_orb::ExperienceOrbEntity;
use super::{Entity, EntityBase, NBTStorageInit};
use crate::block::OnLandedUponArgs;
use crate::entity::NBTStorage;
use crate::entity::ageable::AgeableMob;
use crate::entity::attributes::AttributeInstance;
use crate::entity::attributes::Modifier;
use crate::entity::attributes::ModifierOperation;
use crate::entity::combat::{CombatRules, CombatTracker, FallLocation, knockback_after_resistance};
use crate::entity::mob::equipment::DEFAULT_EQUIPMENT_DROP_CHANCE;
use crate::entity::player::statistics::{CustomStatistic, StatisticCategory};
use crate::server::Server;
use crate::world::loot::LootContextParameters;
use crossbeam::atomic::AtomicCell;
use pumpkin_data::AttributeModifierSlot;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::Operation;
use pumpkin_data::data_component_impl::food::{ConsumableImpl, ConsumeEffect, UseRemainderImpl};
use pumpkin_data::data_component_impl::{
    AttributeModifiersImpl, BlocksAttacksImpl, DeathProtectionImpl, EnchantmentsImpl,
    EquipmentSlot, EquippableImpl, FoodImpl,
};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityPose, EntityStatus, EntityType};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::game_rules::{GameRule, GameRuleValue};
use pumpkin_data::item_stack::{DamageResult, ItemStack};
use pumpkin_data::sound::SoundCategory;
use pumpkin_data::{Block, Enchantment};
use pumpkin_data::{
    damage::{DamageScaling, DamageType},
    sound::Sound,
};
use pumpkin_inventory::entity_equipment::EntityEquipment;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{
    CHurtAnimation, CSetPlayerInventory, CTakeItemEntity, CUpdateMobEffect,
};
use pumpkin_protocol::{
    codec::item_stack_seralizer::ItemStackSerializer,
    java::client::play::{CSetEquipment, MetadataSerializer},
    ser::{NetworkWriteExt, WritingError},
};
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use rand::RngExt;
use std::sync::RwLock;

/// Represents a living entity within the game world.
///
/// This struct encapsulates the core properties and behaviors of living entities, including players, mobs, and other creatures.
/// `LivingEntity.swing`'s restart guard (LivingEntity.java): a swing may begin if none is
/// running, the current one is past its halfway point, or it started this very tick
/// (`swingTime < 0`). Pulled out as a free function so it can be tested without a world.
#[must_use]
pub fn should_restart_swing(swinging: bool, swing_time: i32, duration: i32) -> bool {
    !swinging || swing_time >= duration / 2 || swing_time < 0
}

pub struct LivingEntity {
    /// The underlying entity object, providing basic entity information and functionality.
    pub entity: Entity,
    /// Tracks the remaining time until the entity can regenerate health.
    pub hurt_cooldown: AtomicI32,
    /// Stores the amount of damage the entity last received.
    pub last_damage_taken: AtomicCell<f32>,
    /// The current health level of the entity.
    pub health: AtomicCell<f32>,
    pub stinger_count: AtomicI32,
    remove_stinger_time: AtomicI32,
    /// The current absorption (yellow hearts) on the entity.
    pub absorption: AtomicCell<f32>,
    pub item_use_time: AtomicI32,
    pub item_in_use: std::sync::Mutex<Option<ItemStack>>,
    pub active_hand: std::sync::Mutex<Option<Hand>>,
    pub recent_kinetic_enemies: std::sync::Mutex<FxHashMap<i32, i32>>,
    pub death_time: AtomicU8,
    /// Indicates whether the entity is dead. (`on_death` called)
    pub dead: AtomicBool,
    /// Transient vanilla LivingEntity XP-consumption flag, shared by nearby catalysts.
    pub experience_consumed: AtomicBool,
    pub last_damage_source_entity_id: AtomicI32,
    pub last_damage_source_time: AtomicI64,
    /// The distance the entity has been falling.
    pub fall_distance: Arc<AtomicCell<f64>>,
    pub active_effects: std::sync::Mutex<FxHashMap<&'static StatusEffect, EffectInstance>>,
    pub entity_equipment: Arc<std::sync::Mutex<EntityEquipment>>,
    pub equipment_drop_chances: Arc<std::sync::Mutex<FxHashMap<EquipmentSlot, f32>>>,
    pub movement_input: AtomicCell<Vector3<f64>>,
    /// Separate Java AI speed, enabled as each mob adopts the Brain movement path.
    pub controlled_speed: AtomicCell<Option<f32>>,
    pub equipment_slots: Arc<FxHashMap<usize, EquipmentSlot>>,

    pub jumping: AtomicBool,

    /// `LivingEntity.swinging`: an arm swing is in progress.
    pub swinging: AtomicBool,
    /// `LivingEntity.swingTime`: ticks into the current swing, or -1 the tick it starts.
    pub swing_time: AtomicI32,
    /// `LivingEntity.swingingArm`: which arm the current swing belongs to.
    pub swinging_arm: AtomicCell<Hand>,

    pub jumping_cooldown: AtomicU8,

    pub climbing: AtomicBool,

    /// The position where the entity was last climbing, used for death messages
    pub climbing_pos: AtomicCell<Option<BlockPos>>,
    pub(crate) impulse_context: std::sync::Mutex<super::impulse_context::ImpulseContext>,
    pub(crate) extra_particles_on_fall: AtomicBool,

    /// The entity ID of the entity that last attacked this living entity.
    pub last_attacker_id: AtomicI32,
    /// The tick at which this entity was last attacked (entity age).
    pub last_attacked_time: AtomicI32,

    /// The entity ID of the entity this living entity last attacked.
    pub last_attacking_id: AtomicI32,
    /// The tick at which this entity last attacked something (entity age).
    pub last_attack_time: AtomicI32,

    /// Tracks combat entries, assisted falls, kill credit, and death messages.
    pub combat_tracker: std::sync::Mutex<CombatTracker>,

    /// The ID of the player that last hurt this entity.
    pub last_hurt_by_player_id: AtomicI32,
    /// The tick at which this entity was last hurt by a player.
    pub last_hurt_by_player_time: AtomicI64,
    /// The ID of the mob/entity that last hurt this entity.
    pub last_hurt_by_mob_id: AtomicI32,
    /// The tick at which this entity was last hurt by a mob/entity.
    pub last_hurt_by_mob_time: AtomicI64,

    water_movement_speed_multiplier: f32,
    livings_flags: AtomicU8,

    /// The last block position the entity occupied, used to trigger location changed effects.
    pub last_block_pos: AtomicCell<Option<BlockPos>>,

    /// The attributes of the entity
    pub attributes: RwLock<FxHashMap<u8, AttributeInstance>>,
    pub dimensions_dirty: AtomicBool,
    /// Modifier ids applied from the current item in each equipment slot.
    /// Used to remove them on unequip without the previous stack.
    equipment_attribute_modifier_ids: std::sync::Mutex<FxHashMap<EquipmentSlot, Vec<(u8, String)>>>,
}

#[derive(Clone)]
struct EffectParticle {
    particle_id: VarInt,
    color: i32,
}

#[derive(Clone)]
struct EffectParticles(Vec<EffectParticle>);

impl MetadataSerializer for EffectParticles {
    fn write_metadata(
        &self,
        writer: &mut impl std::io::Write,
        _version: &pumpkin_util::version::JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let count = i32::try_from(self.0.len())
            .map_err(|_| WritingError::Message("Too many effect particles".into()))?;
        writer.write_var_int(&VarInt(count))?;
        for particle in &self.0 {
            writer.write_var_int(&particle.particle_id)?;
            writer.write_i32(particle.color)?;
        }
        Ok(())
    }
}

impl EffectParticle {
    const fn from_effect(effect: &Effect) -> Self {
        Self {
            particle_id: VarInt(Particle::EntityEffect as i32),
            color: (((if effect.ambient { 38 } else { 255 }) as u32) << 24
                | effect.effect_type.color as u32) as i32,
        }
    }
}

fn is_allowed_by_team_rules(
    own_team: Option<&crate::world::scoreboard::Team>,
    their_team: Option<&crate::world::scoreboard::Team>,
) -> bool {
    use crate::world::scoreboard::CollisionRule;

    let own_rule = own_team.map_or(CollisionRule::Always, |team| team.collision_rule);
    let their_rule = their_team.map_or(CollisionRule::Always, |team| team.collision_rule);

    if own_rule == CollisionRule::Never || their_rule == CollisionRule::Never {
        return false;
    }

    let same_team = own_team
        .zip(their_team)
        .is_some_and(|(own, their)| own.name == their.name);

    if (own_rule == CollisionRule::PushOwnTeam || their_rule == CollisionRule::PushOwnTeam)
        && same_team
    {
        return false;
    }

    (own_rule != CollisionRule::PushOtherTeams && their_rule != CollisionRule::PushOtherTeams)
        || same_team
}

/// Resolves an entity's scoreboard team. Players are
/// tracked by name; all other entities are tracked by their UUID string.
pub(crate) fn get_entity_team(entity: &dyn EntityBase) -> Option<crate::world::scoreboard::Team> {
    if let Some(player) = entity.get_player() {
        return player.get_team();
    }

    let entity_ref = entity.get_entity();
    entity_ref
        .world
        .load()
        .scoreboard
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_entity_team(&entity_ref.entity_uuid.to_string())
        .cloned()
}

impl LivingEntity {
    pub fn set_stinger_count(&self, count: i32) {
        self.stinger_count.store(count, Relaxed);
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::living_entity::DATA_STINGER_COUNT_ID,
            pumpkin_protocol::codec::var_int::VarInt(count),
        );
    }

    const USING_ITEM_FLAG: u8 = 1;
    const OFF_HAND_ACTIVE_FLAG: u8 = 2;
    const RANDOM_TELEPORT_ATTEMPTS: usize = 16;
    #[expect(dead_code)]
    const USING_RIPTIDE_FLAG: u8 = 4;

    fn hurt_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.hurt_sound.unwrap_or(Sound::EntityGenericHurt)
    }

    fn death_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.death_sound.unwrap_or(Sound::EntityGenericDeath)
    }

    fn get_pitch(&self) -> f32 {
        let is_baby = self
            .get_mob()
            .and_then(|x| x.as_ageable())
            .is_some_and(AgeableMob::is_baby);

        let mut rng = rand::rng();
        if is_baby {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.5
        } else {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.0
        }
    }

    pub fn new(entity: Entity) -> Self {
        let fall_distance = Arc::clone(&entity.fall_distance);
        let water_movement_speed_multiplier = if entity.entity_type == &EntityType::POLAR_BEAR {
            0.98
        } else if entity.entity_type == &EntityType::SKELETON_HORSE {
            0.96
        } else {
            0.8
        };
        let mut max_health: f32 = 20.0; // Overridden by attribute base below
        Self {
            // Populate local attribute instances from the default registry and get initial vars
            dimensions_dirty: AtomicBool::new(false),
            attributes: {
                let mut m = FxHashMap::default();

                for (attr, base) in entity.entity_type.attributes {
                    if attr.id == Attributes::MAX_HEALTH.id {
                        max_health = *base as f32;
                    }
                    m.insert(attr.id, AttributeInstance::new(*base));
                }
                std::sync::RwLock::new(m)
            },
            stinger_count: AtomicI32::new(0),
            remove_stinger_time: AtomicI32::new(0),
            health: AtomicCell::new(max_health), // Initial health value from attributes
            entity,
            hurt_cooldown: AtomicI32::new(0),
            last_damage_taken: AtomicCell::new(0.0),
            absorption: AtomicCell::new(0.0),
            fall_distance,
            death_time: AtomicU8::new(0),
            dead: AtomicBool::new(false),
            experience_consumed: AtomicBool::new(false),
            last_damage_source_entity_id: AtomicI32::new(-1),
            last_damage_source_time: AtomicI64::new(0),
            item_use_time: AtomicI32::new(0),
            item_in_use: std::sync::Mutex::new(None),
            active_hand: std::sync::Mutex::new(None),
            recent_kinetic_enemies: std::sync::Mutex::new(FxHashMap::default()),
            livings_flags: AtomicU8::new(0),
            active_effects: std::sync::Mutex::new(FxHashMap::default()),
            entity_equipment: Arc::new(std::sync::Mutex::new(EntityEquipment::new())),
            equipment_drop_chances: Arc::new(std::sync::Mutex::new(FxHashMap::default())),
            equipment_slots: Arc::new(build_equipment_slots()),
            jumping: AtomicBool::new(false),
            swinging: AtomicBool::new(false),
            swing_time: AtomicI32::new(0),
            swinging_arm: AtomicCell::new(Hand::Right),
            jumping_cooldown: AtomicU8::new(0),
            climbing: AtomicBool::new(false),
            climbing_pos: AtomicCell::new(None),
            impulse_context: std::sync::Mutex::new(super::impulse_context::ImpulseContext::default()),
            extra_particles_on_fall: AtomicBool::new(false),
            last_attacker_id: AtomicI32::new(0),
            last_attacked_time: AtomicI32::new(0),
            last_attacking_id: AtomicI32::new(0),
            last_attack_time: AtomicI32::new(0),
            combat_tracker: std::sync::Mutex::new(CombatTracker::new()),
            last_hurt_by_player_id: AtomicI32::new(0),
            last_hurt_by_player_time: AtomicI64::new(0),
            last_hurt_by_mob_id: AtomicI32::new(0),
            last_hurt_by_mob_time: AtomicI64::new(0),
            movement_input: AtomicCell::new(Vector3::default()),
            controlled_speed: AtomicCell::new(None),
            water_movement_speed_multiplier,
            last_block_pos: AtomicCell::new(None),
            equipment_attribute_modifier_ids: std::sync::Mutex::new(FxHashMap::default()),
        }
    }

    /// Returns the entity that should receive kill credit for this entity's death.
    /// Following vanilla Java logic (`LivingEntity.getKillCredit`):
    /// 1. Prioritize `last_hurt_by_player` if hurt within the last 100 ticks (5 seconds).
    /// 2. Then `last_hurt_by_mob` if hurt within the last 100 ticks.
    /// 3. Fall back to combat tracker's killer entry if available.
    pub fn get_kill_credit(&self) -> Option<Arc<dyn EntityBase>> {
        let world = self.entity.world.load();
        let current_tick = world.level_info.load().day_time;

        let player_id = self.last_hurt_by_player_id.load(Relaxed);
        let player_time = self.last_hurt_by_player_time.load(Relaxed);
        if player_id != 0
            && (current_tick - player_time).abs() <= 100
            && let Some(player) = world.get_entity_by_id(player_id)
        {
            return Some(player);
        }

        let mob_id = self.last_hurt_by_mob_id.load(Relaxed);
        let mob_time = self.last_hurt_by_mob_time.load(Relaxed);
        if mob_id != 0
            && (current_tick - mob_time).abs() <= 100
            && let Some(mob) = world.get_entity_by_id(mob_id)
        {
            return Some(mob);
        }

        let tracker = self
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(killer) = tracker.get_killer_entry()
            && let Some(killer_id) = killer.attacker_id
        {
            return world.get_entity_by_id(killer_id);
        }

        None
    }

    /// Triggers location-based enchantment effects (e.g. Frost Walker) when the entity's block position changes.
    pub fn on_changed_block(&self, caller: &dyn EntityBase, _pos: BlockPos) {
        let pos_f64 = self.entity.pos.load();
        if let Some(player) = caller.get_player() {
            let boots = player.inventory.get_slot(36);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        } else {
            let boots = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&EquipmentSlot::FEET);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        }
    }

    pub fn send_equipment_changes(&self, equipment: &[(EquipmentSlot, ItemStack)]) {
        if equipment.is_empty() {
            return;
        }
        self.apply_and_send_equipment_attribute_modifiers(equipment);

        if equipment
            .iter()
            .any(|(slot, _)| *slot == EquipmentSlot::FEET)
        {
            let pos_f64 = self.entity.pos.load();
            for (slot, stack) in equipment {
                if *slot == EquipmentSlot::FEET && !stack.is_empty() {
                    crate::enchantment::EnchantmentHelper::on_location_changed(
                        &self.entity,
                        stack,
                        pos_f64,
                    );
                }
            }
        }

        let equipment_java: Vec<(i8, ItemStackSerializer)> = equipment
            .iter()
            .map(|(slot, stack)| {
                (
                    slot.discriminant(),
                    ItemStackSerializer::from(stack.clone()),
                )
            })
            .collect();
        let je_packet = CSetEquipment::new(self.entity_id().into(), equipment_java);

        let mut sent_editioned = false;
        for (slot, stack) in equipment {
            if *slot == EquipmentSlot::MAIN_HAND {
                self.update_weapon_attributes(stack);
            }
            if *slot == EquipmentSlot::MAIN_HAND || *slot == EquipmentSlot::OFF_HAND {
                let window_id = if *slot == EquipmentSlot::OFF_HAND {
                    120
                } else {
                    0
                };

                let be_packet = pumpkin_protocol::bedrock::client::CMobEquipment {
                    target_runtime_id: (self.entity_id() as u64).into(),
                    item: pumpkin_protocol::bedrock::network_item::NetworkItemStackDescriptor::from(
                        stack,
                    ),
                    slot: 0,
                    selected_slot: 0,
                    container_id: window_id,
                };
                self.entity.world.load().send_to_tracking_players_editioned(
                    &self.entity,
                    &je_packet,
                    &be_packet,
                );
                sent_editioned = true;
            }
        }

        if !sent_editioned {
            self.entity
                .world
                .load()
                .send_to_tracking_players(&self.entity, &je_packet);
        }
    }

    /// Applies the held item's attack attribute modifiers to this entity's
    /// attribute map and sends the changed attributes to clients. Without this
    /// the client never sees the reduced attack speed and does not show the
    /// crosshair attack indicator.
    fn update_weapon_attributes(&self, stack: &ItemStack) {
        let component = stack.get_data_component::<AttributeModifiersImpl>();

        // Single pass over the item's modifiers, split by attribute.
        let mut speed_modifiers: Vec<Modifier> = Vec::new();
        let mut damage_modifiers: Vec<Modifier> = Vec::new();
        for modifier in component
            .into_iter()
            .flat_map(|c| c.attribute_modifiers.iter())
        {
            let target = if modifier.r#type == &Attributes::ATTACK_SPEED {
                &mut speed_modifiers
            } else if modifier.r#type == &Attributes::ATTACK_DAMAGE {
                &mut damage_modifiers
            } else {
                continue;
            };
            target.push(Modifier {
                id: modifier.id.to_string(),
                amount: modifier.amount,
                operation: match modifier.operation {
                    Operation::AddValue => ModifierOperation::Add,
                    Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                    Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
                },
            });
        }

        let mut changed: Vec<Attributes> = Vec::new();
        {
            let mut attributes = self
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (attribute, modifiers) in [
                (Attributes::ATTACK_SPEED, speed_modifiers),
                (Attributes::ATTACK_DAMAGE, damage_modifiers),
            ] {
                let instance = attributes
                    .entry(attribute.id)
                    .or_insert_with(|| AttributeInstance::new(attribute.default_value));
                if instance.modifiers == modifiers {
                    continue;
                }
                instance.modifiers = modifiers;
                instance.dirty.store(true, Ordering::Relaxed);
                changed.push(attribute);
            }
        }
        if !changed.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, changed);
        }
    }

    /// Applies item `attribute_modifiers` for the given slots and notifies clients.
    ///
    /// The local HUD armor bar is driven by `minecraft:armor` / `minecraft:armor_toughness`
    /// on `UPDATE_ATTRIBUTES`, not by `SET_EQUIPMENT`.
    pub fn apply_and_send_equipment_attribute_modifiers(
        &self,
        equipment: &[(EquipmentSlot, ItemStack)],
    ) {
        let mut touched = Vec::new();
        for (slot, stack) in equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
        if !touched.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, touched);
        }
    }

    /// Re-applies modifiers from every currently equipped stack without notifying clients.
    pub fn apply_current_equipment_attribute_modifiers(&self) {
        let equipment = self.snapshot_equipped_stacks();
        let mut touched = Vec::new();
        for (slot, stack) in &equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
    }

    /// Re-applies modifiers from every currently equipped stack and sends updates.
    pub fn send_current_equipment_attribute_modifiers(&self) {
        self.apply_and_send_equipment_attribute_modifiers(&self.snapshot_equipped_stacks());
    }

    fn snapshot_equipped_stacks(&self) -> Vec<(EquipmentSlot, ItemStack)> {
        let guard = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .equipment
            .iter()
            .map(|(slot, stack)| (slot.clone(), stack.clone()))
            .collect()
    }

    fn apply_equipment_slot_attribute_modifiers(
        &self,
        slot: &EquipmentSlot,
        stack: &ItemStack,
        touched: &mut Vec<Attributes>,
    ) {
        let previous = {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.remove(slot).unwrap_or_default()
        };
        for (attr_id, modifier_id) in previous {
            if let Some(attr) = attributes_by_id(attr_id) {
                self.update_attribute(attr, |inst| inst.remove_modifier(&modifier_id));
                push_unique_attribute(touched, attr);
            }
        }

        if stack.is_empty() {
            return;
        }
        let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>() else {
            return;
        };

        let mut applied = Vec::new();
        for item_mod in modifiers.attribute_modifiers.iter() {
            if !attribute_modifier_slot_matches(&item_mod.slot, slot) {
                continue;
            }
            let operation = match item_mod.operation {
                Operation::AddValue => ModifierOperation::Add,
                Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
            };
            self.update_attribute(item_mod.r#type, |inst| {
                inst.add_or_replace_modifier(Modifier {
                    id: item_mod.id.to_string(),
                    amount: item_mod.amount,
                    operation,
                });
            });
            applied.push((item_mod.r#type.id, item_mod.id.to_string()));
            push_unique_attribute(touched, item_mod.r#type);
        }

        if !applied.is_empty() {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.insert(slot.clone(), applied);
        }
    }

    /// Picks up an Item entity or XP Orb
    pub fn pickup(&self, item: &Entity, stack_amount: u32) {
        let mut pickup_event =
            crate::plugin::api::events::entity::entity_pickup_item::EntityPickupItemEvent::new(
                self.entity.entity_id,
                item.entity_type.id.to_string(),
                stack_amount as u8,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut pickup_event);
            if pickup_event.cancelled {
                return;
            }
        }

        let chunk_pos = self.entity.chunk_pos.load();
        self.entity.world.load().broadcast_to_chunk_editioned(
            chunk_pos,
            &CTakeItemEntity::new(
                item.entity_id.into(),
                self.entity.entity_id.into(),
                VarInt(stack_amount as i32),
            ),
            &CTakeItemActor {
                item_runtime_id: VarULong(item.entity_id as u64),
                actor_runtime_id: VarULong(self.entity.entity_id as u64),
            },
        );
    }

    /// Sends the Hand animation to all others, used when Eating for example
    pub fn set_active_hand(&self, hand: Hand, stack: ItemStack, duration: i32) {
        self.item_use_time.store(duration, Ordering::Relaxed);
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stack);
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(hand);
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.set_living_flag(Self::USING_ITEM_FLAG, true);
        self.set_living_flag(Self::OFF_HAND_ACTIVE_FLAG, hand == Hand::Left);
    }

    fn set_living_flag(&self, flag: u8, value: bool) {
        let index = flag;
        let mut b = self.livings_flags.load(Ordering::Relaxed);
        if value {
            b |= index;
        } else {
            b &= !index;
        }
        self.livings_flags.store(b, Ordering::Relaxed);

        let bedrock_meta = (flag == Self::USING_ITEM_FLAG).then(|| {
            let index =
                pumpkin_protocol::bedrock::client::set_actor_data::entity_data_flag::USING_ITEM;
            let mask = 1i64 << index;
            if value {
                self.entity.bedrock_flags.fetch_or(mask, Ordering::Relaxed);
            } else {
                self.entity
                    .bedrock_flags
                    .fetch_and(!mask, Ordering::Relaxed);
            }

            let mut meta =
                pumpkin_protocol::bedrock::client::set_actor_data::SyncedActorDataList::new();
            meta.set(
                pumpkin_protocol::bedrock::client::set_actor_data::entity_data_key::FLAGS,
                pumpkin_protocol::bedrock::client::set_actor_data::MetadataValue::Int64(
                    self.entity.bedrock_flags.load(Ordering::Relaxed),
                ),
            );
            meta
        });

        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_LIVING_ENTITY_FLAGS, b);
        if let Some(bedrock_meta) = &bedrock_meta {
            self.entity.send_bedrock_actor_data(bedrock_meta);
        }
    }

    pub fn clear_active_hand(&self) {
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.item_use_time.store(0, Ordering::Relaxed);

        self.set_living_flag(Self::USING_ITEM_FLAG, false);
    }

    pub fn was_recently_stabbed(&self, target_id: i32, now: i32, allowed_ticks: i32) -> bool {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&target_id)
            .is_some_and(|stabbed_at| now - stabbed_at < allowed_ticks)
    }

    pub fn remember_stabbed_entity(&self, target_id: i32, now: i32) {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(target_id, now);
    }

    pub fn is_blocking(&self) -> bool {
        self.blocking_item().is_some()
    }

    fn blocking_item(&self) -> Option<ItemStack> {
        let item_in_use = self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(item) = item_in_use.as_ref()
            && let Some(blocks) = item.get_data_component::<BlocksAttacksImpl>()
        {
            let use_time = self.item_use_time.load(Ordering::Relaxed);
            let required_time = if let Some(dyn_self) = self
                .entity
                .world
                .load()
                .get_entity_by_id(self.entity.entity_id)
                && let Some(player) = dyn_self
                    .cast_any()
                    .downcast_ref::<crate::entity::player::Player>()
                && matches!(
                    player.client.as_ref(),
                    crate::net::ClientPlatform::Bedrock(_)
                ) {
                0
            } else {
                blocks.block_delay_ticks()
            };
            if item.get_max_use_time() - use_time >= required_time {
                return Some(item.clone());
            }
        }
        None
    }

    /// LivingEntity.applyItemBlocking and BlocksAttacks.hurtBlockingItem.
    /// Keep the component snapshot even when durability loss breaks the item.
    fn apply_item_blocking(
        &self,
        caller: &dyn EntityBase,
        damage: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
    ) -> (f32, Option<BlocksAttacksImpl>) {
        let Some(stack) = self.blocking_item().filter(|_| damage > 0.0) else {
            return (0.0, None);
        };
        let Some(blocks) = stack.get_data_component::<BlocksAttacksImpl>().cloned() else {
            return (0.0, None);
        };
        if blocks.bypasses(damage_type)
            || source.is_some_and(|entity| {
                entity
                    .cast_any()
                    .downcast_ref::<crate::entity::projectile::arrow::ArrowEntity>()
                    .is_some_and(|arrow| arrow.pierce_level.load(Relaxed) > 0)
            })
        {
            return (0.0, Some(blocks));
        }
        let angle = position
            .or_else(|| source.map(|entity| entity.get_entity().pos.load()))
            .map_or(f64::from(std::f32::consts::PI), |origin| {
                let delta = origin - self.entity.pos.load();
                let direction = Vector3::new(delta.x, 0.0, delta.z).normalize();
                let view = Vector3::rotation_vector(0.0, f64::from(self.entity.head_yaw.load()));
                direction.dot(&view).clamp(-1.0, 1.0).acos()
            });
        let blocked = blocks.blocked_damage(damage_type, damage, angle);
        // Vanilla only damages blocking items held by players.
        if let Some(player) = caller.get_player() {
            player.increment_stat(StatisticCategory::Used, stack.item.id as i32, 1);
            let hand = *self
                .active_hand
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(hand) = hand {
                let cost = blocks.item_damage.apply(blocked);
                let slot = if hand == Hand::Right {
                    EquipmentSlot::MAIN_HAND
                } else {
                    EquipmentSlot::OFF_HAND
                };
                if cost > 0
                    && player.damage_item_in_slot(&slot, cost)
                    && player.inventory.get_stack_in_hand(hand).is_empty()
                {
                    self.clear_active_hand();
                }
            }
        }
        if blocked > 0.0
            && !damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_PROJECTILE)
            && let Some(attacker) = source
            && let Some(living) = attacker.get_living_entity()
        {
            // LivingEntity.blockedByItem default impulse. Specialized mob overrides
            // (such as the ravager stun) still require separate integration.
            let delta = self.entity.pos.load() - attacker.get_entity().pos.load();
            self.entity.apply_knockback(
                knockback_after_resistance(
                    0.5,
                    self.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE),
                ),
                delta.x,
                delta.z,
            );
            // Player.blockUsingItem disables only the still-present blocking item.
            if let Some(player) = caller.get_player()
                && self.blocking_item().is_some()
            {
                let active_hand = *living
                    .active_hand
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if active_hand != Some(Hand::Left) {
                    let weapon = living.held_item(attacker);
                    let seconds = weapon
                        .get_data_component::<pumpkin_data::data_component_impl::WeaponImpl>()
                        .map_or(0.0, |component| component.disable_blocking_for_seconds);
                    let ticks = blocks.disable_ticks(seconds);
                    if ticks > 0 {
                        let group = stack
                            .get_use_cooldown()
                            .and_then(|c| c.cooldown_group.clone())
                            .unwrap_or_else(|| stack.item.registry_key.to_string());
                        player.start_cooldown(group, ticks);
                        self.clear_active_hand();
                        if let Some(sound) = &blocks.disabled_sound {
                            self.entity.world.load().play_sound_event_fine(
                                sound,
                                SoundCategory::Players,
                                &self.entity.pos.load(),
                                0.8,
                                0.8 + rand::random::<f32>() * 0.4,
                            );
                        }
                    }
                }
            }
        }
        (blocked, Some(blocks))
    }

    pub fn heal(&self, additional_health: f32) {
        assert!(additional_health > 0.0);
        let mut event =
            crate::plugin::api::events::entity::entity_regain_health::EntityRegainHealthEvent::new(
                self.entity.entity_id,
                additional_health,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        self.set_health(self.health.load() + additional_health);
    }

    pub fn set_health(&self, health: f32) {
        // Clamp to [0, max_health]
        let max_health = self.get_max_health();
        let clamped = health.max(0.0).min(max_health);
        self.health.store(clamped);
        // tell everyone entities health changed
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, clamped);
    }

    /// Returns the current maximum health for this entity
    pub fn get_max_health(&self) -> f32 {
        self.get_attribute_value(&Attributes::MAX_HEALTH) as f32
    }

    /// Sets the maximum health for this entity
    pub fn set_max_health(&self, max_health: f32) {
        // Update base attribute
        self.set_attribute_base(&Attributes::MAX_HEALTH, max_health as f64);

        // Broadcast the attribute change
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_HEALTH],
        );

        // Clamp current health to new max if needed and send metadata update
        let current_health = self.health.load();
        if current_health > max_health {
            self.set_health(max_health);
        }
    }

    /// Returns the current absorption amount for this entity (yellow hearts)
    pub fn get_absorption(&self) -> f32 {
        self.absorption.load()
    }

    /// Sets the current absorption amount for this entity (yellow hearts)
    pub fn set_absorption(&self, new_abs: f32) {
        let maximum = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
        let new_abs = new_abs.max(0.0).min(maximum);

        // Set local state
        self.absorption.store(new_abs);

        // Broadcast attribute update for max_absorption so clients receive
        // the updated absorption value via the attribute packet.
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_ABSORPTION],
        );

        // Send absorption metadata for players (visual yellow hearts)
        if self.entity.entity_type == &EntityType::PLAYER {
            self.entity
                .set_synced_data(tracked_data::player::DATA_PLAYER_ABSORPTION_ID, new_abs);
        }
    }

    /// Convenience helper to mutate an attribute instance. Automatically inserts
    /// a new instance populated from the registry base if needed.
    pub fn update_attribute<F: FnOnce(&mut AttributeInstance)>(
        &self,
        attribute: &Attributes,
        f: F,
    ) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let inst = map.entry(attribute.id).or_insert_with(|| {
            let base = self
                .entity
                .entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map_or_else(
                    || {
                        tracing::warn!(
                            "Entity type {:?} has no base value for attribute {:?}; falling back to default {}",
                            self.entity.entity_type,
                            attribute.id,
                            attribute.default_value,
                        );
                        attribute.default_value
                    },
                    |a| a.1,
                );
            AttributeInstance::new(base)
        });

        f(inst);
        inst.dirty.store(true, Ordering::Relaxed);
        if attribute.id == Attributes::SCALE.id {
            self.dimensions_dirty.store(true, Relaxed);
        }
    }

    /// Returns the computed value for `attribute` using the local instance, falling back
    /// to `attribute.default_value` if no local instance exists.
    pub fn get_attribute_value(&self, attribute: &Attributes) -> f64 {
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(&attribute.id)
            .map_or(attribute.default_value, AttributeInstance::value)
    }

    /// Returns the base attribute value for `attribute` for this entity's type.
    pub fn get_attribute_base(&self, attribute: &Attributes) -> f64 {
        // Check the local base value first (could be modified)
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(instance) = map.get(&attribute.id) {
            return instance.base_value;
        }

        // Fall back to registry base value if no local instance exists
        self.entity
            .entity_type
            .attributes
            .iter()
            .find(|a| a.0.id == attribute.id)
            .map_or(attribute.default_value, |a| a.1)
    }

    /// Update or insert the base value for an attribute on this entity.
    /// If the attribute doesn't exist locally yet, it will be inserted.
    pub fn set_attribute_base(&self, attribute: &Attributes, new_base: f64) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(inst) = map.get_mut(&attribute.id) {
            inst.base_value = new_base;
            inst.dirty.store(true, Ordering::Relaxed);
        } else {
            let ai = AttributeInstance::new(new_base);
            ai.dirty.store(true, Ordering::Relaxed);
            map.insert(attribute.id, ai);
        }
    }

    pub fn reset_effects_and_attributes(&self) {
        // Clear active effects and reset modified attributes
        let effects_to_remove: Vec<_> = {
            let lock = self
                .active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.keys().copied().collect()
        };

        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }
    }

    pub const fn entity_id(&self) -> i32 {
        self.entity.entity_id
    }

    pub fn add_effect(&self, effect: Effect) {
        self.try_add_effect(effect);
    }

    pub fn try_add_effect(&self, effect: Effect) -> bool {
        let entity_type = self.entity.entity_type;
        // These entity overrides apply to every effect source, including flowers,
        // potions and commands (EnderDragon.addEffect / Wither*.canBeAffected).
        if entity_type == &EntityType::ENDER_DRAGON
            || (effect.effect_type == &StatusEffect::WITHER
                && (entity_type == &EntityType::WITHER
                    || entity_type == &EntityType::WITHER_SKELETON))
        {
            return false;
        }
        let eligible = if entity_type.has_tag(&tag::EntityType::MINECRAFT_IMMUNE_TO_INFESTED) {
            effect.effect_type != &StatusEffect::INFESTED
        } else if entity_type.has_tag(&tag::EntityType::MINECRAFT_IMMUNE_TO_OOZING) {
            effect.effect_type != &StatusEffect::OOZING
        } else if entity_type.has_tag(&tag::EntityType::MINECRAFT_IGNORES_POISON_AND_REGEN) {
            effect.effect_type != &StatusEffect::POISON
                && effect.effect_type != &StatusEffect::REGENERATION
        } else {
            true
        };
        if !eligible {
            return false;
        }
        let mut effect_event =
            crate::plugin::api::events::entity::entity_potion_effect::EntityPotionEffectEvent::new(
                self.entity.entity_id,
                effect.effect_type.translation_key.to_string(),
                effect.duration,
                effect.amplifier,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut effect_event);
        }
        if effect_event.cancelled {
            return false;
        }

        // Apply instant effects immediately before storing
        if effect.effect_type == &StatusEffect::INSTANT_HEALTH {
            let heal_amount = 4.0 * (1 << effect.amplifier) as f32;
            self.heal(heal_amount);
            // Like vanilla, instant effects are never sent or stored as active effects.
            return true;
        } else if effect.effect_type == &StatusEffect::INSTANT_DAMAGE {
            let damage_amount = 6.0 * (1 << effect.amplifier) as f32;
            let dyn_self = self
                .entity
                .world
                .load()
                .get_entity_by_id(self.entity.entity_id);
            if let Some(dyn_self) = dyn_self {
                let _ = dyn_self.damage(&*dyn_self, damage_amount, DamageType::MAGIC);
            }
            return true;
        }

        let (changed, effective) = {
            let mut effects = self
                .active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(current) = effects.get_mut(effect.effect_type) {
                let changed = current.update(&effect);
                (changed, current.effect.clone())
            } else {
                effects.insert(effect.effect_type, EffectInstance::new(effect.clone()));
                (true, effect.clone())
            }
        };
        if changed {
            self.on_effect_updated(effective);
        }
        // Java calls onEffectStarted even if an update only changes hidden state.
        if effect.effect_type == &StatusEffect::ABSORPTION {
            let requested = 4.0 * (f32::from(effect.amplifier) + 1.0);
            self.set_absorption(self.absorption.load().max(requested));
        }
        changed
    }

    #[expect(clippy::too_many_lines)]
    fn on_effect_updated(&self, effect: Effect) {
        // Apply non-instant effects

        // Effects that modify attributes (ex. speed) should also update the
        // entity's attribute instances (server-side) and then notify clients.
        if !effect.effect_type.attribute_modifiers.is_empty() {
            // Apply each attribute modifier into the local AttributeInstance
            for m in effect.effect_type.attribute_modifiers {
                let id = m.id.to_string();
                let op = match m.operation {
                    Operation::AddValue => ModifierOperation::Add,
                    Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                    Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
                };
                let scaled_amount = m.base_value * (f64::from(effect.amplifier) + 1.);
                let mod_inst = Modifier {
                    id,
                    amount: scaled_amount,
                    operation: op,
                };

                self.update_attribute(m.attribute, |inst| {
                    inst.add_or_replace_modifier(mod_inst.clone());
                });
            }

            // Recompute packet modifiers from active effects for each affected attribute
            let mut touched_attrs: Vec<pumpkin_data::attributes::Attributes> = Vec::new();
            for m in effect.effect_type.attribute_modifiers {
                if !touched_attrs.iter().any(|a| a.id == m.attribute.id) {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // Attribute changes can lower the absorption cap during a hidden downgrade.
        if effect.effect_type == &StatusEffect::ABSORPTION {
            self.set_absorption(self.absorption.load());
        }

        // Apply invisible effect
        if effect.effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(true);
        }

        // Apply glowing effect
        if effect.effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(true);
        }

        // Broadcast effect to nearby players
        let mut flag: i8 = 0;
        if effect.ambient {
            flag |= 1;
        }
        if effect.show_particles {
            flag |= 2;
        }
        if effect.show_icon {
            flag |= 4;
        }
        if effect.blend {
            flag |= 8;
        }

        let je_packet = CUpdateMobEffect::new(
            self.entity.entity_id.into(),
            VarInt(i32::from(effect.effect_type.id)),
            effect.amplifier.into(),
            effect.duration.into(),
            flag,
        );

        let be_packet = pumpkin_protocol::bedrock::client::CMobEffect {
            target_runtime_id: VarULong(self.entity.entity_id as u64),
            event_id: pumpkin_protocol::bedrock::client::CMobEffect::EVENT_ADD,
            effect_id: VarInt(effect.effect_type.to_bedrock_id()),
            effect_amplifier: VarInt(i32::from(effect.amplifier)),
            show_particles: effect.show_particles,
            effect_duration_ticks: VarInt(effect.duration),
            tick: VarULong(0),
            ambient: effect.ambient,
        };

        let chunk_pos = self.entity.chunk_pos.load();
        self.entity
            .world
            .load()
            .broadcast_to_chunk_editioned(chunk_pos, &je_packet, &be_packet);
        self.sync_effect_particles();
    }

    fn sync_effect_particles(&self) {
        let effects = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let has_effects = !effects.is_empty();
        let particles = EffectParticles(
            effects
                .values()
                .filter(|effect| effect.show_particles)
                .map(|effect| EffectParticle::from_effect(&effect.effect))
                .collect(),
        );
        let ambient = effects
            .values()
            .filter(|effect| effect.show_particles)
            .all(|effect| effect.ambient);
        drop(effects);

        self.entity
            .set_synced_data(tracked_data::living_entity::EFFECT_PARTICLES, particles);
        if has_effects {
            self.entity
                .set_synced_data(tracked_data::living_entity::EFFECT_AMBIENCE_ID, ambient);
        }
    }

    pub fn remove_effect(&self, effect_type: &'static StatusEffect) -> bool {
        // Remove the effect
        let succeeded = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&effect_type)
            .is_some();

        // Broadcast effect removal
        self.entity
            .world
            .load()
            .send_remove_mob_effect(&self.entity, effect_type);

        // Remove attribute modifiers, if any
        if !effect_type.attribute_modifiers.is_empty() {
            let mut touched_attrs = Vec::new();

            for m in effect_type.attribute_modifiers {
                let id = m.id.to_string();

                // Clean local server state
                self.update_attribute(m.attribute, |inst| {
                    inst.remove_modifier(&id);
                });

                // Track unique attributes for the packet update
                if !touched_attrs
                    .iter()
                    .any(|a: &Attributes| a.id == m.attribute.id)
                {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            // Sync the clean state to the client
            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // If absorption effect removed, clear current absorption amount and notify clients
        if effect_type == &StatusEffect::ABSORPTION {
            self.set_absorption(0.0);
        }

        // If health boost effect removed, clamp current health to new max and notify clients
        if effect_type == &StatusEffect::HEALTH_BOOST {
            let new_max = self.get_max_health();
            if self.health.load() > new_max {
                // Update local health and send both health and absorption metadata together
                self.set_health(new_max.max(0.0));
            }
        }

        // If invisible effect removed, disable invisibility
        if effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(false);
        }

        // If glowing effect removed, disable glowing
        if effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(false);
        }

        if succeeded {
            self.sync_effect_particles();
        }

        succeeded
    }

    pub fn has_effect(&self, effect: &'static StatusEffect) -> bool {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&effect)
    }

    pub fn get_effect(&self, effect: &'static StatusEffect) -> Option<Effect> {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&effect)
            .map(|instance| instance.effect.clone())
    }

    // Check if the entity is in water
    pub fn is_in_water(&self) -> bool {
        self.entity.touching_water.load(Ordering::Relaxed)
    }

    // Check if the entity is in powder snow
    pub fn is_in_powder_snow(&self) -> bool {
        self.entity.is_in_powder_snow.load(Ordering::Relaxed)
    }

    pub fn is_immune_to_fall_damage(&self) -> bool {
        self.entity
            .entity_type
            .has_tag(&tag::EntityType::MINECRAFT_FALL_DAMAGE_IMMUNE)
    }

    pub(crate) fn get_effective_gravity(&self, caller: &dyn EntityBase) -> f64 {
        let final_gravity = caller.get_gravity();

        if self.entity.velocity.load().y <= 0.0 && self.has_effect(&StatusEffect::SLOW_FALLING) {
            final_gravity.min(0.01)
        } else {
            final_gravity
        }
    }

    /// `LivingEntity.getCurrentSwingDuration`: how many ticks the held item's swing takes,
    /// shortened by haste or conduit power and lengthened by mining fatigue.
    pub fn current_swing_duration(&self, caller: &dyn EntityBase) -> i32 {
        use pumpkin_data::data_component_impl::SwingAnimationImpl;

        let hand = self.swinging_arm.load();
        let stack = self.get_stack_in_hand(caller, hand);
        let duration = stack
            .get_data_component::<SwingAnimationImpl>()
            .map_or(SwingAnimationImpl::DEFAULT.duration, |a| a.duration);

        // MobEffectUtil.hasDigSpeed / getDigSpeedAmplification: haste and conduit power
        // both count, and the stronger of the two wins.
        let haste = self
            .get_effect(&StatusEffect::HASTE)
            .map(|e| i32::from(e.amplifier));
        let conduit = self
            .get_effect(&StatusEffect::CONDUIT_POWER)
            .map(|e| i32::from(e.amplifier));
        if haste.is_some() || conduit.is_some() {
            let amplification = haste.unwrap_or(0).max(conduit.unwrap_or(0));
            return duration - (1 + amplification);
        }
        if let Some(fatigue) = self.get_effect(&StatusEffect::MINING_FATIGUE) {
            return duration + (1 + i32::from(fatigue.amplifier)) * 2;
        }
        duration
    }

    /// `LivingEntity.swing(hand)` (LivingEntity.java): start an arm swing.
    ///
    /// The guard matters. A swing only restarts if none is running, the current one is
    /// past its halfway point, or it began this very tick (`swingTime < 0`). Without it a
    /// mob attacking every tick re-sends the animation from frame zero each time and the
    /// arm never actually moves on the client.
    pub fn swing(&self, caller: &dyn EntityBase, hand: Hand) {
        if !should_restart_swing(
            self.swinging.load(Relaxed),
            self.swing_time.load(Relaxed),
            self.current_swing_duration(caller),
        ) {
            return;
        }

        self.swing_time.store(-1, Relaxed);
        self.swinging.store(true, Relaxed);
        self.swinging_arm.store(hand);

        let world = self.entity.world.load();
        let entity_id = self.entity_id();
        let je_packet = pumpkin_protocol::java::client::play::CEntityAnimation::new(
            entity_id.into(),
            match hand {
                Hand::Right => pumpkin_protocol::java::client::play::Animation::SwingMainArm,
                Hand::Left => pumpkin_protocol::java::client::play::Animation::SwingOffhand,
            },
        );
        let be_packet = pumpkin_protocol::bedrock::server::animate::SAnimate {
            action: pumpkin_protocol::bedrock::server::animate::AnimateAction::SwingArm,
            target_actor_runtime_id: pumpkin_protocol::codec::var_ulong::VarULong(entity_id as u64),
            data: 0.0,
            swing_source: None,
        };
        world.broadcast_editioned(&je_packet, &be_packet);
    }

    /// `LivingEntity.updateSwingTime`: advance the swing one tick, ending it once it has
    /// run its full duration. Ticked for every living entity, as vanilla does in `aiStep`.
    pub fn update_swing_time(&self, caller: &dyn EntityBase) {
        let duration = self.current_swing_duration(caller);
        if self.swinging.load(Relaxed) {
            let next = self.swing_time.load(Relaxed) + 1;
            if next >= duration {
                self.swing_time.store(0, Relaxed);
                self.swinging.store(false, Relaxed);
            } else {
                self.swing_time.store(next, Relaxed);
            }
        } else {
            self.swing_time.store(0, Relaxed);
        }
    }

    fn tick_movement(&self, caller: &dyn EntityBase) {
        self.check_climbing(caller);
        if self.jumping_cooldown.load(Relaxed) != 0 {
            self.jumping_cooldown.fetch_sub(1, Relaxed);
        }

        let should_swim_in_fluids = caller.get_player().is_none_or(|player| !player.is_flying());

        self.entity.check_zero_velo();

        let mut movement_input = self.movement_input.load();

        // Brain-navigation inputs were damped before its AI and controllers.
        if self.controlled_speed.load().is_none() {
            movement_input.x *= 0.98;
            movement_input.z *= 0.98;
        }

        self.movement_input.store(movement_input);

        // TODO: Tick AI

        if self.jumping.load(SeqCst) && should_swim_in_fluids {
            let in_lava = self.entity.is_in_lava();

            let in_water = self.entity.touching_water.load(SeqCst);

            let fluid_height = if in_lava {
                self.entity.lava_height.load()
            } else {
                self.entity.water_height.load()
            };

            let swim_height = self.get_swim_height();

            let on_ground = self.entity.on_ground.load(SeqCst);

            if (in_water || in_lava) && (!on_ground || fluid_height > swim_height) {
                // Swim upward

                let mut velo = self.entity.velocity.load();

                if self.controlled_speed.load().is_some() {
                    velo.y += caller
                        .get_mob()
                        .map_or(0.04_f32 as f64, |mob| mob.liquid_jump_strength());
                } else {
                    velo.y += 0.04;
                }

                self.entity.velocity.store(velo);
            } else if (on_ground || in_water && fluid_height <= swim_height)
                && self.jumping_cooldown.load(SeqCst) == 0
            {
                self.jump();

                self.jumping_cooldown.store(10, SeqCst);
            }
        } else {
            self.jumping_cooldown.store(0, SeqCst);
        }

        if self.has_effect(&StatusEffect::SLOW_FALLING)
            || self.has_effect(&StatusEffect::LEVITATION)
        {
            self.fall_distance.store(0.0);
        }

        let touching_water = self.entity.touching_water.load(SeqCst);

        // Strider is the only entity that has canWalkOnFluid = false

        let simulate_movement = caller.can_simulate_movement()
            && (self.controlled_speed.load().is_none()
                || caller.get_mob().is_none_or(|mob| !mob.get_mob_entity().is_no_ai()));
        if !simulate_movement {
            // Java NoAI disables travel; block effects and collision processing
            // still run below. Preserve existing velocity instead of adding gravity.
        } else if (touching_water || self.entity.is_in_lava())
            && should_swim_in_fluids
            && self.entity.entity_type != &EntityType::STRIDER
        {
            self.travel_in_fluid(caller, touching_water);
        } else {
            // TODO: Gliding

            self.travel_in_air(caller);
        }

        let suffocating = self.entity.tick_block_collisions(caller);

        if suffocating {
            caller.damage(caller, 1.0, DamageType::IN_WALL);
        }

        self.push_entities(caller);
    }

    fn push_entities(&self, dyn_self: &dyn EntityBase) {
        let world = self.entity.world.load();
        let entity_bb = self.entity.bounding_box.load();
        let own_team = get_entity_team(dyn_self);

        let pushable: Vec<Arc<dyn EntityBase>> = world
            .get_all_at_box(&entity_bb)
            .into_iter()
            .filter(|entity| {
                let entity_ref = entity.get_entity();
                entity_ref.entity_id != self.entity.entity_id
                    && !entity.is_spectator()
                    && entity.is_pushable()
                    && is_allowed_by_team_rules(
                        own_team.as_ref(),
                        get_entity_team(&**entity).as_ref(),
                    )
            })
            .collect();

        if pushable.is_empty() {
            return;
        }

        // Entity cramming check
        let max_cramming = match world.get_game_rule(&GameRule::MaxEntityCramming) {
            GameRuleValue::Int(value) => value,
            GameRuleValue::Bool(_) => 0,
        };
        if max_cramming > 0
            && pushable.len() as i64 > max_cramming - 1
            && rand::random::<u32>().is_multiple_of(4)
        {
            let count = pushable
                .iter()
                .filter(|entity| !entity.is_passenger())
                .count();
            if count as i64 > max_cramming - 1 {
                dyn_self.damage(dyn_self, 6.0, DamageType::CRAMMING);
            }
        }

        for entity in pushable {
            entity.push(dyn_self);
        }
    }

    /// Decays player velocity like vanilla `travelInAir` friction.
    fn apply_travel_friction(&self) {
        let mut velo = self.entity.velocity.load();
        if velo.x == 0.0 && velo.z == 0.0 {
            return;
        }

        let friction = if self.entity.on_ground.load(Relaxed) {
            f64::from(
                self.entity
                    .get_block_with_y_offset(0.500_001)
                    .1
                    .slipperiness,
            ) * 0.91
        } else {
            0.91
        };

        velo.x *= friction;

        velo.z *= friction;

        self.entity.velocity.store(velo);
    }

    fn apply_movement_input(&self, input: Vector3<f64>, speed: f64) {
        let [x, y, z] = crate::entity::ai::control::travel_input::velocity(
            [input.x, input.y, input.z],
            speed as f32,
            self.entity.yaw.load(),
        );
        self.entity
            .velocity
            .store(self.entity.velocity.load() + Vector3::new(x, y, z));
    }

    fn travel_in_air(&self, caller: &dyn EntityBase) {
        use crate::entity::ai::control::travel_input;
        let base_speed = self
            .controlled_speed
            .load()
            .unwrap_or_else(|| self.get_attribute_value(&Attributes::MOVEMENT_SPEED) as f32);
        let ground = self.entity.on_ground.load(Relaxed);
        let slipperiness = if ground {
            travel_input::modified_friction(
                self.entity
                    .world
                    .load()
                    .get_block(&self.entity.get_block_pos_below_that_affects_my_movement())
                    .slipperiness,
                self.get_attribute_value(&Attributes::FRICTION_MODIFIER) as f32,
            )
        } else {
            1.0
        };
        let player_passenger = caller
            .get_controlling_passenger()
            .is_some_and(|passenger| passenger.get_player().is_some());
        let speed = if !ground && let Some(player) = caller.get_player() {
            player.get_off_ground_speed() as f32
        } else {
            travel_input::air_speed(base_speed, ground, slipperiness, player_passenger)
        };
        let air_drag = travel_input::modified_friction(
            0.91,
            self.get_attribute_value(&Attributes::AIR_DRAG_MODIFIER) as f32,
        );
        let friction = f64::from(slipperiness * air_drag);
        self.apply_movement_input(self.movement_input.load(), f64::from(speed));

        self.apply_climbing_speed();

        self.make_move(caller);

        let mut velo = self.entity.velocity.load();

        let can_powder_snow_climb = if self.entity.was_in_powder_snow.load(Relaxed) {
            crate::block::blocks::powder_snow::can_entity_walk_on_powder_snow(caller)
        } else {
            false
        };

        if (self.entity.horizontal_collision.load(SeqCst) || self.jumping.load(SeqCst))
            && (self.climbing.load(Relaxed) || can_powder_snow_climb)
        {
            velo.y = 0.2;
        }

        let levitation = self.get_effect(&StatusEffect::LEVITATION);

        if let Some(lev) = levitation {
            velo.y += (0.05 * f64::from(lev.amplifier + 1) - velo.y) * 0.2;
        } else {
            velo.y -= self.get_effective_gravity(caller);

            // TODO: If world is not loaded: replace effective gravity with:

            // if below world's bottom y then -0.1, else 0.0
        }

        // If entity has no drag: store velo and return

        velo.x *= friction;

        velo.z *= friction;

        velo.y *= if caller.omnidirectional_air_mover() {
            f64::from(caller.get_air_drag())
        } else {
            caller
                .get_y_velocity_drag()
                .unwrap_or_else(|| f64::from(caller.get_air_drag()))
        };

        self.entity.velocity.store(velo);
    }

    fn travel_in_fluid(&self, caller: &dyn EntityBase, water: bool) {
        if let Some(speed) = self.controlled_speed.load() {
            self.travel_in_controlled_fluid(caller, water, speed);
            return;
        }
        let movement_input = self.movement_input.load();

        let falling = self.entity.velocity.load().y <= 0.0;
        let gravity = self.get_effective_gravity(caller);
        let effective_speed = self.get_attribute_value(&Attributes::MOVEMENT_SPEED);

        if water {
            let mut friction = if self.entity.sprinting.load(Relaxed) {
                0.9
            } else {
                f64::from(self.water_movement_speed_multiplier)
            };

            let mut speed = 0.02;

            // Apply water movement efficiency attribute
            let mut water_movement_efficiency =
                self.get_attribute_value(&Attributes::WATER_MOVEMENT_EFFICIENCY);

            if water_movement_efficiency > 0.0 {
                if !self.entity.on_ground.load(SeqCst) {
                    water_movement_efficiency *= 0.5;
                }

                friction += (0.546_000_06 - friction) * water_movement_efficiency;
                speed += (effective_speed - speed) * water_movement_efficiency;
            }

            if self.has_effect(&StatusEffect::DOLPHINS_GRACE) {
                friction = 0.96;
            }

            self.apply_movement_input(movement_input, speed);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();
            if self.entity.horizontal_collision.load(SeqCst) && self.climbing.load(Relaxed) {
                velo.y = 0.2;
            }

            velo = velo.multiply(friction, 0.8, friction);

            self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            self.entity.velocity.store(velo);
        } else {
            self.apply_movement_input(movement_input, 0.02);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();

            if self.entity.lava_height.load() <= self.get_swim_height() {
                velo.x *= 0.5;
                velo.z *= 0.5;
                velo.y *= 0.8;

                self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            } else {
                velo = velo * 0.5;
            }

            if gravity != 0.0 {
                velo.y -= gravity / 4.0; // Negative gravity = buoyancy
            }

            self.entity.velocity.store(velo);
        }

        let mut velo = self.entity.velocity.load();

        if self.entity.horizontal_collision.load(SeqCst)
            && !self
                .entity
                .world
                .load()
                .check_fluid_collision(self.entity.bounding_box.load().shift(velo))
        {
            velo.y = 0.3;

            self.entity.velocity.store(velo);
        }
    }

    fn travel_in_controlled_fluid(&self, caller: &dyn EntityBase, water: bool, speed: f32) {
        use crate::entity::ai::control::fluid_travel;
        let old_y = self.entity.pos.load().y;
        let falling = self.entity.velocity.load().y <= 0.0;
        let gravity = self.get_effective_gravity(caller);
        let sprint = self.entity.sprinting.load(Relaxed);
        let [acceleration, friction] = if water {
            fluid_travel::water_parameters(
                sprint,
                self.water_movement_speed_multiplier,
                self.get_attribute_value(&Attributes::WATER_MOVEMENT_EFFICIENCY),
                self.entity.on_ground.load(Relaxed),
                speed,
                self.has_effect(&StatusEffect::DOLPHINS_GRACE),
            )
        } else {
            [0.02_f32, 0.0]
        };
        self.apply_movement_input(self.movement_input.load(), f64::from(acceleration));
        self.make_move(caller);
        let velocity = self.entity.velocity.load();
        let collision = self.entity.horizontal_collision.load(Relaxed);
        let mut velocity = fluid_travel::after_move(
            [velocity.x, velocity.y, velocity.z],
            water,
            friction,
            collision && self.climbing.load(Relaxed),
            self.entity.lava_height.load() <= self.get_swim_height(),
            gravity,
            falling,
            sprint,
        );
        if collision {
            let query = fluid_travel::escape_query(velocity, self.entity.pos.load().y, old_y);
            let bounds = self
                .entity
                .bounding_box
                .load()
                .shift(Vector3::new(query[0], query[1], query[2]));
            let world = self.entity.world.load();
            // Solid-entity and world-border collision categories have a separate parity gate.
            if world.get_block_collisions(bounds, caller).0.is_empty()
                && !world.contains_any_liquid(bounds)
            {
                velocity = fluid_travel::escape_velocity(velocity);
            }
        }
        self.entity
            .velocity
            .store(Vector3::new(velocity[0], velocity[1], velocity[2]));
    }

    fn apply_fluid_moving_speed(&self, dy: &mut f64, gravity: f64, falling: bool) {
        if gravity != 0.0 && !self.entity.sprinting.load(Relaxed) {
            if falling && (*dy - 0.005).abs() >= 0.003 && (*dy - gravity / 16.0).abs() < 0.003 {
                *dy = -0.003;
            } else {
                *dy -= gravity / 16.0;
            }
        }
    }

    fn make_move(&self, caller: &dyn EntityBase) {
        self.entity.move_entity(caller, self.entity.velocity.load());

        self.check_climbing(caller);
    }

    pub(crate) fn check_climbing(&self, caller: &dyn EntityBase) {
        let pos = self.entity.block_pos.load();
        let world = self.entity.world.load();
        let state = world.get_block_state(&pos);
        let climbing = super::climbing::can_climb(
            state.id,
            || world.get_block_state(&pos.down()).id,
            caller.is_spectator(),
            self.entity.is_fall_flying(),
        );
        self.climbing.store(climbing, Relaxed);
        if climbing {
            self.climbing_pos.store(Some(pos));
        } else if self.entity.on_ground.load(SeqCst) {
            self.climbing_pos.store(None);
        }
    }

    fn apply_climbing_speed(&self) {
        if self.climbing.load(Relaxed) {
            self.fall_distance.store(0.0);
            let hold_ladder = self.entity.entity_type == &EntityType::PLAYER
                && self.entity.is_sneaking()
                && self
                    .entity
                    .world
                    .load()
                    .get_block(&self.entity.block_pos.load())
                    != &Block::SCAFFOLDING;
            self.entity.velocity.store(super::climbing::limit_velocity(
                self.entity.velocity.load(),
                hold_ladder,
            ));
        }
    }

    pub fn get_swim_height(&self) -> f64 {
        let eye_height = self.entity.get_eye_height();

        if self.entity.entity_type == &EntityType::BREEZE {
            eye_height
        } else if eye_height < 0.4 {
            0.0
        } else {
            0.4
        }
    }

    fn jump(&self) {
        let jump = self.get_jump_velocity(1.0);

        if jump <= 1.0e-5 {
            return;
        }

        let mut velo = self.entity.velocity.load();

        velo.y = jump.max(velo.y);

        if self.entity.sprinting.load(Relaxed) {
            let yaw = f64::from(self.entity.yaw.load()).to_radians();

            velo.x -= yaw.sin() * 0.2;
            velo.z += yaw.cos() * 0.2;
        }

        self.entity.velocity.store(velo);

        self.entity.velocity_dirty.store(true, SeqCst);
    }

    fn get_jump_velocity(&self, mut strength: f64) -> f64 {
        strength *= self.get_attribute_value(&Attributes::JUMP_STRENGTH);
        strength *= f64::from(self.entity.get_jump_velocity_multiplier());
        if let Some(effect) = self.get_effect(&StatusEffect::JUMP_BOOST) {
            strength += 0.1 * f64::from(effect.amplifier + 1);
        }
        strength
    }

    pub fn fall(
        &self,
        caller: &dyn EntityBase,
        height_difference: f64,
        ground: bool,
        dont_damage: bool,
    ) {
        if caller.get_player().is_some()
            && ground
            && self.fall_distance.load() > 0.0
            && self.extra_particles_on_fall.swap(false, Relaxed)
        {
            let world = self.entity.world.load();
            let pos = self.entity.get_pos_with_y_offset(0.2).0;
            world.spawn_block_particles(
                world.get_block_state_id(&pos),
                pos.to_f64().add_raw(0.5, 1.0, 0.5),
                (50.0 * self.fall_distance.load()).clamp(0.0, 200.0) as i32,
                Vector3::new(0.3, 0.3, 0.3),
            );
        }
        if !self.entity.is_in_water() {
            self.entity.update_fluid_state(caller);
        }
        let previous_distance = self.fall_distance.load();
        if ground && previous_distance > 0.0 {
            let world = self.entity.world.load();
            let on_pos = self.entity.get_pos_with_y_offset(0.2).0;
            let on_state = world.get_block_state(&on_pos);
            self.on_changed_block(caller, on_pos);
            let power = (self.fall_distance.load() + 1.0e-6
                - self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE))
            .floor()
            .max(0.0);
            if power > 0.0 && !on_state.is_air() {
                let mut position = self.entity.pos.load();
                let entity_pos = self.entity.block_pos.load();
                if on_pos.0.x != entity_pos.0.x || on_pos.0.z != entity_pos.0.z {
                    let x_diff = position.x - f64::from(on_pos.0.x) - 0.5;
                    let z_diff = position.z - f64::from(on_pos.0.z) - 0.5;
                    let max_diff = x_diff.abs().max(z_diff.abs());
                    position.x = f64::from(on_pos.0.x) + 0.5 + x_diff / max_diff * 0.5;
                    position.z = f64::from(on_pos.0.z) + 0.5 + z_diff / max_diff * 0.5;
                }
                let scale = (f64::from(0.2_f32) + power / 15.0).min(2.5);
                world.spawn_block_particles(
                    on_state.id,
                    position,
                    (150.0 * scale) as i32,
                    Vector3::new(0.0, 0.0, 0.0),
                );
            }
        }
        let distance = super::fall_distance::accumulate(
            previous_distance,
            height_difference,
            self.entity.is_in_water(),
        );
        self.fall_distance.store(distance);
        if ground {
            if distance > 0.0 {
                let world = self.entity.world.load();
                let position = self.entity.get_pos_with_y_offset(0.2).0;
                let (block, state) = world.get_block_and_state(&position);
                if !dont_damage {
                    if let Some(pumpkin_block) = world.block_registry.get_pumpkin_block(block.id) {
                        pumpkin_block.on_landed_upon(OnLandedUponArgs {
                            world: &world,
                            position: &position,
                            fall_distance: distance,
                            entity: caller,
                        });
                    } else {
                        caller.cause_fall_damage(caller, distance, 1.0, DamageType::FALL);
                    }
                }
                let support = self
                    .entity
                    .get_supporting_block_pos()
                    .map_or(state.id, |pos| world.get_block_state_id(&pos));
                world.emit_game_event_with_context(
                    "hit_ground",
                    self.entity.pos.load(),
                    Some(self.entity.entity_id),
                    Some(support),
                );
            }
            // Damage/combat callbacks observe the accumulated distance until landing finishes.
            self.fall_distance.store(0.0);
            self.climbing_pos.store(None);
        }
    }

    pub(crate) fn reset_impulse_context(&self) {
        self.impulse_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset();
    }
    pub(crate) fn try_reset_impulse_context(&self) {
        self.impulse_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_reset();
    }

    pub fn handle_fall_damage(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f64,
        damage_per_distance: f32,
    ) {
        self.handle_fall_damage_with_type(
            caller,
            fall_distance,
            damage_per_distance,
            DamageType::FALL,
        );
    }

    pub fn handle_fall_damage_with_type(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f64,
        damage_per_distance: f32,
        damage_type: DamageType,
    ) {
        caller.cause_fall_damage(caller, fall_distance, damage_per_distance, damage_type);
    }

    pub fn apply_fall_damage_with_type(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f64,
        damage_per_distance: f32,
        damage_type: DamageType,
    ) -> bool {
        let may_fly = caller.get_player().is_some_and(|player| {
            player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .allow_flying
        });
        if may_fly || self.is_immune_to_fall_damage() {
            return false;
        }

        // Vanilla parity: the fall_damage gamerule only affects players.
        if caller.get_player().is_some()
            && !self
                .entity
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .fall_damage
        {
            return false;
        }

        let damage = calculate_fall_damage(
            f64::from(fall_distance),
            self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE),
            f64::from(damage_per_distance),
            self.get_attribute_value(&Attributes::FALL_DAMAGE_MULTIPLIER),
        );
        if damage > 0.0 {
            self.reset_impulse_context();
            if !self.entity.is_silent() {
                let player = caller.get_player().is_some();
                let hostile = super::is_monster_type(self.entity.entity_type);
                let category = if player {
                    SoundCategory::Players
                } else if hostile {
                    SoundCategory::Hostile
                } else {
                    SoundCategory::Neutral
                };
                let sound = match (player, hostile, damage > 4.0) {
                    (true, _, true) => Sound::EntityPlayerBigFall,
                    (true, _, false) => Sound::EntityPlayerSmallFall,
                    (_, true, true) => Sound::EntityHostileBigFall,
                    (_, true, false) => Sound::EntityHostileSmallFall,
                    (_, _, true) => Sound::EntityGenericBigFall,
                    _ => Sound::EntityGenericSmallFall,
                };
                let world = self.entity.world.load();
                let pos = self.entity.pos.load();
                world.play_sound(sound, category, &pos);
                let below = BlockPos::floored(pos.x, pos.y - f64::from(0.2_f32), pos.z);
                let (block, state) = world.get_block_and_state(&below);
                if !state.is_air() {
                    let sound = pumpkin_data::sound_type::sound_type_for_block(block.id);
                    world.play_sound_fine(
                        sound.fall_sound,
                        category,
                        &pos,
                        sound.volume * 0.5,
                        sound.pitch * 0.75,
                    );
                }
            }
            self.damage(caller, damage, damage_type);
            return true;
        }
        false
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    pub fn get_death_message(
        dyn_self: &dyn EntityBase,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> TextComponent {
        let kill_credit = dyn_self.get_living_entity().and_then(Self::get_kill_credit);
        let kill_credit_name = kill_credit.as_ref().map(|c| c.get_display_name());

        if let Some(living) = dyn_self.get_living_entity() {
            let tracker = living
                .combat_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            return tracker.get_death_message(dyn_self.get_display_name(), kill_credit_name);
        }

        if let Some(cause) = cause
            && source.is_some()
        {
            TextComponent::translate_cross(
                format!("death.attack.{}", damage_type.message_id),
                format!("death.attack.{}", damage_type.message_id),
                [dyn_self.get_display_name(), cause.get_display_name()],
            )
        } else if let Some(killer) = cause
            .or(source)
            .map(|c| c.get_display_name())
            .or(kill_credit_name)
        {
            TextComponent::translate_cross(
                format!("death.attack.{}.player", damage_type.message_id),
                format!("death.attack.{}.player", damage_type.message_id),
                [dyn_self.get_display_name(), killer],
            )
        } else {
            TextComponent::translate_cross(
                format!("death.attack.{}", damage_type.message_id),
                format!("death.attack.{}", damage_type.message_id),
                [dyn_self.get_display_name()],
            )
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn on_death(
        &self,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
        // Vanilla parity: `makeSound(deathSound)` (and `playSecondaryHurtSound`) inside
        // `hurtServer`'s death branch only fires `if (tookFullDamage)` -- a killing blow
        // absorbed via the invulnerability-window difference rule plays no death sound
        // (LivingEntity.java:1252-1257). Everything else in `die()` still runs regardless.
        took_full_damage: bool,
    ) {
        let world = self.entity.world.load();
        let Some(dyn_self) = world.get_entity_by_id(self.entity.entity_id) else {
            return;
        };
        if self
            .dead
            .compare_exchange(false, true, Relaxed, Relaxed)
            .is_ok()
        {
            self.entity.set_fall_flying(false);
            self.movement_input.store(Vector3::default());
            self.jumping.store(false, Relaxed);

            let kill_credit = self.get_kill_credit();
            let killer = cause.or(source).or(kill_credit.as_deref());

            self.update_death_stats(&*dyn_self, killer);

            if took_full_damage {
                world.play_sound_fine(
                    self.death_sound(&*dyn_self),
                    SoundCategory::Players,
                    &self.entity.pos.load(),
                    1.0,
                    self.get_pitch(),
                );
            }
            world.send_entity_status(&self.entity, EntityStatus::Death, Some(ActorEventID::Death));
            let looting_level;
            let tool = if let Some(cause_ent) = cause {
                if let Some(player) = cause_ent
                    .cast_any()
                    .downcast_ref::<crate::entity::player::Player>()
                {
                    let hand_stack = player
                        .inventory()
                        .get_stack_in_hand(pumpkin_util::Hand::Right);
                    looting_level = hand_stack
                        .get_enchantment_level(&Enchantment::LOOTING)
                        .max(0) as u32;
                    (!hand_stack.is_empty()).then(|| hand_stack.clone())
                } else {
                    looting_level = 0;
                    None
                }
            } else {
                looting_level = 0;
                None
            };

            let is_raining = world.is_raining();
            let is_thundering = world.is_thundering();

            let has_player_kill =
                killer.is_some_and(|c| c.get_entity().entity_type == &EntityType::PLAYER) || {
                    let tracker = self
                        .combat_tracker
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    tracker.has_player_attacker()
                };

            let params = LootContextParameters {
                killed_by_player: Some(has_player_kill),
                this_entity: Some(self.entity.entity_type),
                killer_entity: killer.map(|c| c.get_entity().entity_type),
                direct_killer_entity: source.map(|s| s.get_entity().entity_type),
                position: Some(self.entity.pos.load()),
                world_time: world.level_info.load().day_time as u64,
                damage_type: Some(damage_type),
                tool,
                is_raining: Some(is_raining),
                is_thundering: Some(is_thundering),
                is_on_fire: Some(
                    self.entity
                        .fire_ticks
                        .load(std::sync::atomic::Ordering::Relaxed)
                        > 0,
                ),
                // Facts the loot predicates ask about. Without these the conditions they
                // gate can never pass, so they are supplied wherever the entity exposes
                // them and left `None` otherwise -- which fails the condition closed.
                this_is_baby: Some(
                    dyn_self
                        .get_mob()
                        .and_then(crate::entity::mob::Mob::as_ageable)
                        .is_some_and(crate::entity::ageable::AgeableMob::is_baby),
                ),
                this_vehicle: self
                    .entity
                    .vehicle
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                    .map(|vehicle| vehicle.get_entity().entity_type),
                ..Default::default()
            };

            // LivingEntity.die emits before death loot (including experience).
            world.emit_game_event_with_source(
                "entity_die",
                self.entity.pos.load(),
                Some(self.entity.entity_id),
            );
            // Drop loot
            self.drop_loot(&params);

            // Award experience
            if !self.experience_consumed.load(Relaxed)
                && params.killed_by_player.unwrap_or(false)
                && dyn_self.should_drop_experience()
                && world.level_info.load().game_rules.mob_drops
            {
                let amount = dyn_self.get_experience_reward(killer);
                if amount > 0 {
                    ExperienceOrbEntity::spawn(&world, self.entity.pos.load(), amount);
                }
            }
            self.entity.pose.store(EntityPose::Dying);

            self.drop_equipment(looting_level);

            // Broadcast death message if it's a player and the gamerule is enabled
            self.broadcast_death_message(&*dyn_self, damage_type, source, cause);

            // Trigger on_mob_death for active status effects
            let active_effects_vec: Vec<_> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects
                    .values()
                    .map(|e| (e.effect_type, e.amplifier))
                    .collect()
            };
            for (effect_type, amplifier) in active_effects_vec {
                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect_type) {
                    mob_effect.on_mob_death(self, amplifier, &damage_type);
                }
            }

            self.reset_effects_and_attributes();
        }
    }

    fn drop_equipment(&self, looting_level: u32) {
        let world = self.entity.world.load();
        let block_pos = self.entity.block_pos.load();

        let drop_chances = self
            .equipment_drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let slots_to_drop: Vec<EquipmentSlot> = {
            let mut slots: Vec<_> = self.equipment_slots.values().cloned().collect();
            slots.push(EquipmentSlot::MAIN_HAND);
            slots
        };

        for slot in &slots_to_drop {
            let mut chance = drop_chances
                .get(slot)
                .copied()
                .unwrap_or(DEFAULT_EQUIPMENT_DROP_CHANCE);
            // Vanilla approximation: EnchantmentHelper.processEquipmentDropChance
            // adds lootingLevel * 0.01 to the per-slot equipment drop chance.
            chance += looting_level as f32 * 0.01;
            chance = chance.min(1.0);
            if rand::random::<f32>() >= chance {
                continue;
            }
            let mut item = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .equipment
                .remove(slot)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            if item.is_empty() {
                continue;
            }
            // Vanilla approximation: Mob.dropCustomDeathLoot applies random
            // damage to dropped equipment using two chained random calls:
            // setDamageValue(maxDamage - random.nextInt(1 + random.nextInt(max(maxDamage - 3, 1))))
            if let Some(max_damage) = item.get_max_damage() {
                let mut rng = rand::rng();
                let inner = rng.random_range(0..(max_damage - 3).max(1));
                let outer = rng.random_range(0..=inner);
                item.set_damage((max_damage - outer).max(0));
            }
            world.drop_stack(&block_pos, item);
        }
    }

    fn broadcast_death_message(
        &self,
        dyn_self: &dyn EntityBase,
        damage_type: DamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) {
        let world = self.entity.world.load();
        let show_death_messages = { world.level_info.load().game_rules.show_death_messages };
        if self.entity.entity_type == &EntityType::PLAYER {
            let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
            let mut final_death_message = death_message;
            if let Some(player) = dyn_self.get_player() {
                if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                    && let Some(server) = world.server.upgrade()
                {
                    let mut event =
                        crate::plugin::api::events::entity::entity_death::PlayerDeathEvent::new(
                            player_arc,
                            final_death_message.clone(),
                            0,
                        );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        return;
                    }
                    final_death_message = event.death_message;
                }

                player.handle_killed(&final_death_message);
            }

            if show_death_messages && let Some(server) = world.server.upgrade() {
                for player in server.get_all_players() {
                    player.send_system_message(&final_death_message);
                }
            }
        } else if self.entity.custom_name.load().is_some() {
            let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
            tracing::info!(
                "Named entity {} died: {}",
                dyn_self.get_display_name().to_pretty_console(),
                death_message.to_pretty_console()
            );
        }
    }

    fn update_death_stats(&self, dyn_self: &dyn EntityBase, cause: Option<&dyn EntityBase>) {
        if let Some(victim_player) = dyn_self.get_player() {
            victim_player.increment_custom_stat(CustomStatistic::Deaths, 1);
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceDeath as i32,
                0,
            );
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceRest as i32,
                0,
            );
            if let Some(killer_entity) = cause.map(EntityBase::get_entity) {
                victim_player.increment_stat(
                    StatisticCategory::KilledBy,
                    killer_entity.entity_type.id as i32,
                    1,
                );
            }
        }

        if let Some(killer_player) = cause.and_then(|c| c.get_player()) {
            killer_player.increment_stat(
                StatisticCategory::Killed,
                self.entity.entity_type.id as i32,
                1,
            );
            if dyn_self.get_player().is_some() {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::PlayerKills as i32,
                    1,
                );
            } else {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::MobKills as i32,
                    1,
                );

                let resource_name = self.entity.entity_type.resource_name;
                let criterion_key = format!("minecraft:{resource_name}");
                killer_player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::PlayerKilledEntity {
                        entity_type_resource: criterion_key,
                    },
                );

                if resource_name == "skeleton" {
                    let distance_sq = killer_player
                        .position()
                        .squared_distance_to_vec(&self.entity.pos.load());
                    if distance_sq >= 2500.0 {
                        killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::SniperDuel);
                    }
                }

                if resource_name == "phantom" {
                    killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::TwoBirdsOneArrow);
                }

                let held_item = killer_player.inventory().held_item();
                let is_crossbow = held_item.item.registry_key == "crossbow";
                if is_crossbow {
                    killer_player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::Arbalistic,
                    );
                }
            }
        }
    }

    fn drop_loot(&self, params: &LootContextParameters) {
        let resource_name = self.get_entity().entity_type.resource_name;
        let key = format!("minecraft:entities/{resource_name}");
        if let Some(loot_table) = pumpkin_data::loot_table::get_loot_table(&key) {
            let seed: i64 = rand::random();
            let pos = self.entity.block_pos.load();
            for stack in crate::world::loot::generate_loot_with_context(loot_table, seed, params) {
                self.entity.world.load().drop_stack(&pos, stack);
            }
        }
    }

    fn tick_effects(&self) {
        let mut effects_to_remove = Vec::new();
        let mut effects_to_apply = Vec::new();
        let mut effects_to_refresh = Vec::new();

        {
            let Ok(mut effects) = self.active_effects.try_lock() else {
                return;
            };
            let entity_age = self.entity.tick_count.load(Relaxed);
            for effect in effects.values_mut() {
                if !effect.has_remaining_duration() {
                    effects_to_remove.push(effect.effect_type);
                    continue;
                }

                let tick_duration = if effect.duration == -1 {
                    entity_age
                } else {
                    effect.duration
                };

                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect.effect_type)
                    && mob_effect.should_apply_effect_tick(tick_duration, effect.amplifier)
                {
                    effects_to_apply.push((mob_effect, effect.amplifier));
                }

                let (alive, downgraded) = effect.tick_duration();
                if downgraded {
                    effects_to_refresh.push(effect.effect.clone());
                }
                if !alive {
                    effects_to_remove.push(effect.effect_type);
                }
            }
        }

        for effect in effects_to_refresh {
            self.on_effect_updated(effect);
        }

        // Call the central removal function for each expired effect
        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }

        for (mob_effect, amplifier) in effects_to_apply {
            mob_effect.apply_effect_tick(self, amplifier);
        }
    }

    /// Tries to use a totem of undying from the entity's hands. If successful, applies the totem effects and returns true.
    #[allow(dead_code)]
    async fn try_use_death_protector(&self, caller: &dyn EntityBase) -> bool {
        for hand in Hand::all() {
            let mut stack = self.get_stack_in_hand(caller, hand);

            // Clear the stack and use the totem of undying
            if stack.get_data_component::<DeathProtectionImpl>().is_some() {
                let mut resurrect_event =
                    crate::plugin::api::events::entity::entity_resurrect::EntityResurrectEvent::new(
                        self.entity.entity_id,
                    );
                if let Some(server) = self.entity.world.load().server.upgrade() {
                    server
                        .plugin_manager
                        .fire(&server, &mut resurrect_event)
                        .await;
                }
                if resurrect_event.cancelled {
                    return false;
                }

                stack.clear();
                let slot = match hand {
                    Hand::Right => EquipmentSlot::MAIN_HAND,
                    Hand::Left => EquipmentSlot::OFF_HAND,
                };
                if let Some(player) = caller.get_player() {
                    player
                        .inventory()
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot, stack);
                } else {
                    self.entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot, stack);
                }
                self.set_health(1.0);
                self.entity.world.load().send_entity_status(
                    &self.entity,
                    EntityStatus::ProtectedFromDeath,
                    Some(ActorEventID::InstantDeath),
                );

                // Set Absorption, Regeneration, and Fire Resistance effects
                self.add_effect(Effect {
                    effect_type: &StatusEffect::ABSORPTION,
                    duration: 100,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::REGENERATION,
                    duration: 900,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::FIRE_RESISTANCE,
                    duration: 800,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });

                return true;
            }
        }

        false
    }

    #[allow(dead_code)]
    fn damage_armor_items(&self, caller: &dyn EntityBase, damage_amount: f32) {
        // Formula: armor loses floor(incoming_damage / 4) durability, minimum 1.
        let armor_damage = (damage_amount / 4.0).floor().max(1.0) as i32;
        let mut equipment_updates = Vec::new();

        // TODO: Falling anvil/stalactite should only damage the helmet slot.
        // TODO: Implement DAMAGE_RESISTANT component checks (e.g. netherite vs fire).

        let armor_slots: Vec<(usize, ItemStack, EquipmentSlot)> = {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.equipment_slots
                .iter()
                .filter(|(_, slot)| slot.is_armor_slot())
                .filter_map(|(index, slot)| {
                    equipment_lock
                        .equipment
                        .get(slot)
                        .cloned()
                        .map(|stack| (*index, stack, slot.clone()))
                })
                .collect()
        };

        for (slot_index, mut stack, slot) in armor_slots {
            if stack.is_empty() {
                continue;
            }

            let takes_damage = stack
                .get_data_component::<EquippableImpl>()
                .is_none_or(|equippable| equippable.damage_on_hurt);

            if takes_damage {
                let item_id = stack.item.id;
                let slot_result = stack.damage_item(armor_damage);
                if slot_result != pumpkin_data::item_stack::DamageResult::Untouched {
                    if slot_result == pumpkin_data::item_stack::DamageResult::Broken {
                        if let Some(player) = caller.get_player() {
                            player.increment_stat(
                                pumpkin_data::statistic::StatisticCategory::Broken,
                                item_id as i32,
                                1,
                            );
                        }
                        let world = self.entity.world.load();
                        world.send_entity_status(
                            &self.entity,
                            super::equipment_break_status(&slot),
                            None,
                        );
                    }
                    equipment_updates.push((slot.clone(), stack.clone()));
                    if let Some(player) = caller.get_player() {
                        player.enqueue_slot_set_packet(&CSetPlayerInventory::new(
                            (slot_index as i32).into(),
                            &ItemStackSerializer::from(stack),
                        ));
                    }
                }
            }
        }

        if !equipment_updates.is_empty() {
            self.send_equipment_changes(&equipment_updates);
        }
    }

    pub fn held_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.held_item();
        }
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(&EquipmentSlot::MAIN_HAND)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    pub fn get_stack_in_hand(&self, caller: &dyn EntityBase, hand: Hand) -> ItemStack {
        match hand {
            Hand::Left => self.off_hand_item(caller),
            Hand::Right => self.held_item(caller),
        }
    }

    /// getOffHandStack in source
    pub fn off_hand_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.off_hand_item();
        }
        let Some(slot) = self.equipment_slots.get(&PlayerInventory::OFF_HAND_SLOT) else {
            return ItemStack::EMPTY.clone();
        };
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    pub fn can_take_damage(&self) -> bool {
        !self.entity.invulnerable.load(Ordering::Relaxed) && self.is_part_of_game()
    }

    pub fn is_part_of_game(&self) -> bool {
        !self.is_spectator() && self.entity.is_alive()
    }

    pub fn reset_state(&self) {
        self.reset_impulse_context();
        self.extra_particles_on_fall.store(false, Relaxed);
        self.entity.reset_state();

        // Restore to maximum health for this entity type
        let max_health = self.get_max_health();
        self.set_health(max_health);
        // Clear any absorption
        self.absorption.store(0.0);
        // Send health metadata
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, max_health);

        self.reset_effects_and_attributes();

        // Give a short grace period of invulnerability after respawn
        self.hurt_cooldown.store(20, Relaxed);
        self.last_damage_taken.store(0f32);

        self.entity.portal_cooldown.store(0, Relaxed);
        *self
            .entity
            .portal_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;

        // Clear fall/fire state
        self.fall_distance.store(0.0);
        self.death_time.store(0, Relaxed);
        self.entity.extinguish();
        self.entity.fire_ticks.store(0, Relaxed);

        // Clear velocity and movement input to remove persisted momentum
        self.entity.velocity.store(Vector3::default());
        self.entity.velocity_dirty.store(true, SeqCst);
        self.movement_input.store(Vector3::default());
        self.jumping.store(false, Relaxed);

        // If this LivingEntity corresponds to a Player, reset their hunger manager
        let world = self.entity.world.load();
        if let Some(player) = world.get_player_by_id(self.entity.entity_id) {
            player.hunger_manager.restart();
        }

        self.experience_consumed.store(false, Relaxed);
        self.last_damage_source_entity_id.store(-1, Relaxed);
        self.last_damage_source_time.store(0, Relaxed);
        self.dead.store(false, Relaxed);
    }

    pub fn is_player(&self) -> bool {
        let world = self.entity.world.load();
        world.get_player_by_id(self.entity.entity_id).is_some()
    }

    pub fn get_movement(&self) -> Vector3<f64> {
        self.entity.movement.load()
    }

    fn death_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.death_sound()
        {
            return audio;
        }

        Self::death_sound_for_entity(self.entity.entity_type)
    }

    fn hurt_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.hurt_sound()
        {
            return audio;
        }

        Self::hurt_sound_for_entity(self.entity.entity_type)
    }
}

impl LivingEntity {
    pub fn write_living_nbt(&self, nbt: &mut NbtCompound) {
        self.impulse_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);
        let attributes = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let saved = Attributes::ALL
            .iter()
            .filter_map(|a| {
                attributes
                    .get(&a.id)
                    .map(|instance| NbtTag::Compound(instance.write_nbt(a.name)))
            })
            .collect();
        nbt.put("attributes", NbtTag::List(saved));
        drop(attributes);
        nbt.put("Health", NbtTag::Float(self.health.load()));
        // Persist current absorption amount
        nbt.put("AbsorptionAmount", NbtTag::Float(self.absorption.load()));
        nbt.put_short("HurtTime", self.hurt_cooldown.load(Relaxed).max(0) as i16);
        nbt.put_short("DeathTime", i16::from(self.death_time.load(Relaxed)));
        nbt.put_bool("FallFlying", self.entity.is_fall_flying());
        {
            let effects_vec: Vec<EffectInstance> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects.values().cloned().collect()
            };
            if !effects_vec.is_empty() {
                // Iterate effects and create Box<[NbtTag]>
                let mut effects_list = Vec::with_capacity(effects_vec.len());
                for effect in effects_vec {
                    let mut effect_nbt = pumpkin_nbt::compound::NbtCompound::new();
                    effect.write_nbt(&mut effect_nbt);
                    effects_list.push(NbtTag::Compound(effect_nbt));
                }
                nbt.put("active_effects", NbtTag::List(effects_list));
            }
        }
        let mut equipment_nbt = NbtCompound::new();
        for (slot, stack) in &self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .equipment
        {
            if !stack.is_empty() {
                let mut data = NbtCompound::new();
                stack.write_item_stack(&mut data);
                equipment_nbt.put_compound(slot.to_name(), data);
            }
        }
        nbt.put_compound("equipment", equipment_nbt);
        let mut drops = NbtCompound::new();
        for (slot, chance) in self
            .equipment_drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
        {
            drops.put_float(slot.to_name(), *chance);
        }
        nbt.put_compound("drop_chances", drops);
        // todo more...
    }

    pub fn read_living_nbt_non_mut(&self, nbt: &NbtCompound) {
        self.impulse_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .read_nbt(nbt);
        // Restore base values before clamping health; only permanent modifiers
        // are stored. Age/effects/equipment recreate their transient modifiers.
        if let Some(saved) = nbt.get_list("attributes") {
            for tag in saved {
                let Some(data) = tag.extract_compound() else {
                    continue;
                };
                let Some(name) = data.get_string("id") else {
                    continue;
                };
                if let Some(attribute) = Attributes::ALL.iter().find(|a| a.name == name) {
                    self.update_attribute(attribute, |instance| instance.read_nbt(data));
                }
            }
        }
        if let Some(saved) = nbt.get_compound("equipment") {
            let mut equipment = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (name, value) in &saved.child_tags {
                if let Some(slot) = EquipmentSlot::get_from_name(name)
                    && let Some(data) = value.extract_compound()
                {
                    // ItemStack's count defaults to one in Java's persistent codec.
                    let mut data = data.clone();
                    if data.get_int("count").is_none() {
                        data.put_int("count", 1);
                    }
                    if let Some(stack) = ItemStack::read_item_stack(&data) {
                        equipment.put(slot, stack);
                    }
                }
            }
        }
        if let Some(saved) = nbt.get_compound("drop_chances") {
            let mut drops = self
                .equipment_drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (name, value) in &saved.child_tags {
                if let Some(slot) = EquipmentSlot::get_from_name(name)
                    && let NbtTag::Float(chance) = value
                {
                    drops.insert(slot.clone(), *chance);
                }
            }
        }
        self.set_health(
            nbt.get_float("Health")
                .unwrap_or_else(|| self.get_max_health()),
        );

        // Clamp any persisted absorption to the entity's configured max
        let raw_abs = nbt.get_float("AbsorptionAmount").unwrap_or(0.0);
        let max_abs = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
        let clamped_abs = raw_abs.max(0.0).min(max_abs);
        self.absorption.store(clamped_abs);

        if let Some(hurt_time) = nbt.get_short("HurtTime") {
            self.hurt_cooldown.store(i32::from(hurt_time), Relaxed);
        }
        if let Some(death_time) = nbt.get_short("DeathTime") {
            self.death_time.store(death_time as u8, Relaxed);
        }
        self.entity.set_fall_flying(
            self.health.load() > 0.0 && nbt.get_bool("FallFlying").unwrap_or(false),
        );
        {
            let mut loaded = FxHashMap::default();
            if let Some(tags) = nbt.get_list("active_effects") {
                for tag in tags {
                    let parsed = match tag {
                        NbtTag::Compound(nbt) => EffectInstance::create_from_nbt(&mut nbt.clone()),
                        _ => None,
                    };
                    let Some(mut effect) = parsed else {
                        loaded.clear();
                        break;
                    };
                    effect.blend = true;
                    loaded.insert(effect.effect_type, effect);
                }
            }
            *self
                .active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = loaded;
        }
        // todo more...
    }

    /// Calculates damage after armor reduction, mirroring vanilla `LivingEntity.getDamageAfterArmorAbsorb`.
    pub fn get_damage_after_armor_absorb(
        &self,
        damage: f32,
        damage_type: &DamageType,
        caller: &dyn EntityBase,
        attacker: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ARMOR) {
            return damage;
        }

        // Vanilla parity: armor durability is damaged with the pre-reduction `damage`
        // before the reduced amount is computed (LivingEntity.java:1906-1911). The base
        // `hurtArmor` is a no-op (LivingEntity.java:1886) -- only `Player` overrides it
        // (Player.java:738, all 4 armor slots via `doHurtEquipment`); regular `Mob`s do not
        // override it either, so this hook is a genuine no-op for them, matching vanilla.
        caller.hurt_armor(*damage_type, damage);

        let mut armor = 0.0f32;
        let mut toughness = 0.0f32;
        {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ] {
                if let Some(stack) = equipment_lock.equipment.get(&slot)
                    && !stack.is_empty()
                    && let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>()
                {
                    for modifier in modifiers.attribute_modifiers.iter() {
                        if modifier.r#type == &Attributes::ARMOR {
                            armor += modifier.amount as f32;
                        } else if modifier.r#type == &Attributes::ARMOR_TOUGHNESS {
                            toughness += modifier.amount as f32;
                        }
                    }
                }
            }
        }

        let breach_level = attacker
            .and_then(|att| {
                let player = att.get_player()?;
                let hand_stack = player
                    .inventory()
                    .get_stack_in_hand(pumpkin_util::Hand::Right);
                let level = hand_stack.get_enchantment_level(&Enchantment::BREACH);
                (level > 0).then_some(level as u32)
            })
            .unwrap_or(0);

        CombatRules::get_damage_after_absorb(damage, armor, toughness, breach_level)
    }

    /// Calculates damage after magic/resistance/enchantment reduction, mirroring vanilla `LivingEntity.getDamageAfterMagicAbsorb`.
    pub fn get_damage_after_magic_absorb(
        &self,
        mut damage: f32,
        damage_type: &DamageType,
        caller: &dyn EntityBase,
        cause: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_EFFECTS) {
            return damage;
        }

        // 1. Resistance Effect (evaluated before enchantments)
        if !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_RESISTANCE)
            && let Some(effect) = self.get_effect(&StatusEffect::RESISTANCE)
        {
            let absorb_value = (effect.amplifier + 1) * 5;
            let absorb = 25 - absorb_value;
            let v = damage * absorb as f32;
            let old_damage = damage;
            damage = (v / 25.0).max(0.0);
            let damage_resisted = old_damage - damage;
            if damage_resisted > 0.0 {
                if let Some(victim_player) = caller.get_player() {
                    victim_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageResisted as i32,
                        (damage_resisted * 10.0).round() as i32,
                    );
                } else if let Some(attacker_player) = cause.and_then(|c| c.get_player()) {
                    attacker_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageDealtResisted as i32,
                        (damage_resisted * 10.0).round() as i32,
                    );
                }
            }
        }

        if damage <= 0.0 {
            return 0.0;
        }

        // 2. Enchantment Protection
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ENCHANTMENTS) {
            return damage;
        }

        let is_fire_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FIRE);
        let mut epf = 0.0f32;
        {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ] {
                if let Some(stack) = equipment_lock.equipment.get(&slot)
                    && !stack.is_empty()
                    && let Some(enchantments) = stack.get_data_component::<EnchantmentsImpl>()
                {
                    for (enchantment, level) in enchantments.enchantment.iter() {
                        let enc = *enchantment;
                        let lvl = *level as f32;
                        if enc == &Enchantment::PROTECTION {
                            if !damage_type
                                .has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
                                && damage_type != &DamageType::STARVE
                                && damage_type != &DamageType::GENERIC_KILL
                                && damage_type != &DamageType::OUT_OF_WORLD
                            {
                                epf += lvl;
                            }
                        } else if enc == &Enchantment::FIRE_PROTECTION {
                            if is_fire_damage {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::BLAST_PROTECTION {
                            if damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_EXPLOSION) {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::PROJECTILE_PROTECTION {
                            if damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_PROJECTILE) {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::FEATHER_FALLING
                            && damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FALL)
                        {
                            epf += lvl * 3.0;
                        }
                    }
                }
            }
        }

        if epf > 0.0 {
            damage = CombatRules::get_damage_after_magic_absorb(damage, epf);
        }

        damage
    }

    /// Vanilla parity: the `invulnerableTime`/`lastHurt` rule from `LivingEntity.hurtServer`
    /// (LivingEntity.java:1216-1231):
    /// ```java
    /// if (this.invulnerableTime > 10.0F && !source.is(DamageTypeTags.BYPASSES_COOLDOWN)) {
    ///     if (damage <= this.lastHurt) return false;
    ///     this.actuallyHurt(level, source, damage - this.lastHurt);
    ///     this.lastHurt = damage;
    ///     tookFullDamage = false;
    /// } else {
    ///     this.lastHurt = damage;
    ///     this.invulnerableTime = 20;
    ///     this.actuallyHurt(level, source, damage);
    /// }
    /// ```
    /// `amount` is the raw incoming damage (after blocking/freeze scaling, before armor and
    /// magic absorb -- those run per-hit on whichever amount this returns, not on `amount`
    /// itself, since the armor formula is not linear in the damage it's given). `last_hurt`
    /// is the entity's stored `lastHurt`. `cooldown_active` is
    /// `invulnerableTime > 10 && !BYPASSES_COOLDOWN`, computed by the caller.
    ///
    /// Returns `None` when the hit must be rejected outright (a smaller-or-equal hit landing
    /// inside the invulnerability window -- vanilla's early `return false`). Otherwise
    /// returns `Some((raw_hit_amount, took_full_damage))`: the raw amount to run through
    /// armor/magic absorb next, and whether this was a full hit rather than the
    /// invulnerability-window difference.
    fn resolve_hurt_cooldown(
        amount: f32,
        last_hurt: f32,
        cooldown_active: bool,
    ) -> Option<(f32, bool)> {
        if cooldown_active {
            if amount <= last_hurt {
                None
            } else {
                Some((amount - last_hurt, false))
            }
        } else {
            Some((amount, true))
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        let mut amount = amount;

        // Check invulnerability before applying damage
        if self.entity.is_invulnerable_to(&damage_type) {
            return false;
        }

        if self.health.load() <= 0.0 || self.dead.load(Relaxed) {
            return false; // Dying or dead
        }

        // Vanilla parity: `LivingEntity.hurtServer` clamps negative damage to zero rather
        // than rejecting the hit outright (LivingEntity.java:1194-1196).
        if amount < 0.0 {
            amount = 0.0;
        }

        // Vanilla parity: `Player.hurtServer` scales incoming damage by world difficulty
        // before anything else runs (Player.java:694-708); only players are scaled, mobs
        // never are. `DamageSource.scalesWithDifficulty` (DamageSource.java:92-98) keys off
        // the damage type's `scaling()`: `WHEN_CAUSED_BY_LIVING_NON_PLAYER` only scales when
        // the credited attacker (vanilla's `DamageSource.getEntity()`, i.e. `cause` here) is
        // a non-player `LivingEntity`.
        if caller.get_player().is_some() {
            let scales = match damage_type.scaling {
                DamageScaling::Never => false,
                DamageScaling::WhenCausedByLivingNonPlayer => cause
                    .is_some_and(|c| c.get_living_entity().is_some() && c.get_player().is_none()),
                DamageScaling::Always => true,
            };
            if scales {
                match self.entity.world.load().level_info.load().difficulty {
                    pumpkin_util::Difficulty::Peaceful => amount = 0.0,
                    pumpkin_util::Difficulty::Easy => amount = (amount / 2.0 + 1.0).min(amount),
                    pumpkin_util::Difficulty::Normal => {}
                    pumpkin_util::Difficulty::Hard => amount = amount * 3.0 / 2.0,
                }
            }
            // Vanilla parity: `Player.hurtServer` returns false for zero damage
            // unconditionally, not only when scaling produced it (Player.java:708).
            if amount == 0.0 {
                return false;
            }
        }

        let mut damage_event =
            crate::plugin::api::events::entity::entity_damage::EntityDamageEvent::new(
                self.entity.entity_id,
                damage_type,
                amount,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut damage_event);
        }
        if damage_event.cancelled {
            return false;
        }
        amount = damage_event.damage;

        if let Some(damager) = source.or(cause) {
            let mut by_entity_event =
                crate::plugin::api::events::entity::entity_damage_by_entity::EntityDamageByEntityEvent {
                    entity_id: self.entity.entity_id,
                    damager_id: damager.get_entity().entity_id,
                    damage: amount,
                    cause: format!("{damage_type:?}"),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_entity_event);
            }
            if by_entity_event.cancelled {
                return false;
            }
            amount = by_entity_event.damage;
        } else if position.is_some()
            || matches!(
                damage_type,
                DamageType::CACTUS
                    | DamageType::SWEET_BERRY_BUSH
                    | DamageType::CAMPFIRE
                    | DamageType::HOT_FLOOR
                    | DamageType::STALAGMITE
            )
        {
            let damager_pos = position.map(|p| {
                BlockPos(Vector3::new(
                    p.x.floor() as i32,
                    p.y.floor() as i32,
                    p.z.floor() as i32,
                ))
            });
            let mut by_block_event =
                crate::plugin::api::events::entity::entity_damage_by_block::EntityDamageByBlockEvent {
                    entity_id: self.entity.entity_id,
                    damager_pos,
                    damage: amount,
                    cause: format!("{damage_type:?}"),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_block_event);
            }
            if by_block_event.cancelled {
                return false;
            }
            amount = by_block_event.damage;
        }

        let world = self.entity.world.load();
        let is_fire_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FIRE);

        // Fire damage can be prevented by either game rules or fire resistance
        if is_fire_damage {
            // Check game rule for fire damage (only for players)
            if self.entity.entity_type == &EntityType::PLAYER
                && !world.level_info.load().game_rules.fire_damage
            {
                return false;
            }

            // Vanilla parity: fire-resistance immunity is unconditional here -- it does not
            // check BYPASSES_EFFECTS (LivingEntity.java:1185).
            if self.has_effect(&StatusEffect::FIRE_RESISTANCE) {
                return false;
            }
        }

        let (damage_blocked, blocking_component) =
            self.apply_item_blocking(caller, amount, damage_type, position, source);
        amount -= damage_blocked;
        let blocked = damage_blocked > 0.0;

        // Vanilla parity: entities in FREEZE_HURTS_EXTRA_TYPES take 5x freezing damage,
        // applied to the post-blocking damage (LivingEntity.java:1203-1205).
        if damage_type == DamageType::FREEZE
            && self
                .entity
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_FREEZE_HURTS_EXTRA_TYPES)
        {
            amount *= 5.0;
        }

        // These damage types bypass the hurt cooldown and death protection
        // (DamageTypeTags.BYPASSES_COOLDOWN, LivingEntity.java:1217). No
        // MINECRAFT_BYPASSES_COOLDOWN tag constant is generated because vanilla's own
        // "bypasses_cooldown" tag data currently has no members beyond generic_kill and
        // out_of_world, so those two are enumerated directly instead of a tag lookup.
        let bypasses_cooldown_protection =
            damage_type == DamageType::GENERIC_KILL || damage_type == DamageType::OUT_OF_WORLD;

        // Vanilla parity: `LivingEntity.hurtServer` invulnerableTime/lastHurt rule
        // (LivingEntity.java:1216-1231). `hurt_cooldown` mirrors `invulnerableTime` and
        // `last_damage_taken` mirrors `lastHurt`; both track the RAW damage (after
        // blocking/freeze scaling but before armor/magic absorb). Armor and magic absorb
        // are computed per-hit inside `actuallyHurt`, not once up front -- so a second hit
        // landing inside the invulnerability window is reduced by armor as the DIFFERENCE
        // `amount - lastHurt`, not as `absorb(amount) - absorb(lastHurt)`. Applying absorb
        // once to the full amount and diffing afterwards (the previous implementation)
        // gives a different, wrong number because the armor formula is not linear in
        // `damage` (CombatRules.java:20 divides by `damage / toughness`).
        let last_hurt = self.last_damage_taken.load();
        let cooldown_active =
            self.hurt_cooldown.load(Relaxed) > 10 && !bypasses_cooldown_protection;
        let Some((raw_hit_amount, took_full_damage)) =
            Self::resolve_hurt_cooldown(amount, last_hurt, cooldown_active)
        else {
            return false;
        };
        self.last_damage_taken.store(amount);
        if took_full_damage {
            self.hurt_cooldown.store(20, Relaxed);
        }

        // Vanilla parity: `actuallyHurt` (LivingEntity.java:1960-1979) -- armor absorb,
        // then magic absorb, then absorption hearts, then health, applied to whichever
        // raw amount was selected above (full hit or invulnerability-window difference).
        let damage_after_armor = self.get_damage_after_armor_absorb(
            raw_hit_amount,
            &damage_type,
            caller,
            // Vanilla parity: `CombatRules.getDamageAfterAbsorb`'s weapon-item (Breach
            // enchantment) lookup reads `DamageSource.getWeaponItem()`, which resolves via
            // the DIRECT entity, not the credited cause (DamageSource.java:67-69) --
            // `source` here is Pumpkin's direct-entity equivalent.
            source,
        );
        let effective_amount = self.get_damage_after_magic_absorb(
            damage_after_armor,
            &damage_type,
            caller,
            // Vanilla parity: the Resistance-absorbed-damage stat attributes to
            // `DamageSource.getEntity()` (the credited cause), not the direct entity
            // (LivingEntity.java:1932-1934).
            cause,
        );

        let current_abs = self.absorption.load();
        let dmg_to_health = (effective_amount - current_abs).max(0.0);
        let absorbed_damage = effective_amount - dmg_to_health;

        if absorbed_damage > 0.0 {
            let new_abs = (current_abs - absorbed_damage).max(0.0);
            self.set_absorption(new_abs);

            if let Some(player) = caller.get_player() {
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealtAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }
        }

        // Vanilla parity: `actuallyHurt` only touches health/combat-tracker when the
        // post-absorption damage is non-zero (LivingEntity.java:1972-1977).
        if dmg_to_health != 0.0 {
            let max_h = self.get_max_health();
            let new_health = (self.health.load() - dmg_to_health).clamp(0.0, max_h);

            if let Some(player) = caller.get_player() {
                if damage_type.exhaustion > 0.0 {
                    player.add_exhaustion(damage_type.exhaustion);
                }
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageTaken as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            self.set_health(new_health);

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealt as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            let current_tick = world.level_info.load().day_time;
            let fall_location = FallLocation::get_current_fall_location(self, &world);
            let fall_distance = self.fall_distance.load();
            let is_alive = self.health.load() > 0.0 && !self.dead.load(Relaxed);
            {
                let mut tracker = self
                    .combat_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                tracker.record_damage(
                    current_tick,
                    is_alive,
                    fall_distance,
                    fall_location,
                    damage_type,
                    dmg_to_health,
                    source,
                    cause,
                );
            }
            // LivingEntity.actuallyHurt only emits after a nonzero health loss.
            world.emit_game_event_with_source(
                "entity_damage",
                self.entity.pos.load(),
                Some(self.entity.entity_id),
            );
        }

        // Vanilla parity: `resolveMobResponsibleForDamage`/`resolvePlayerResponsibleForDamage`
        // (LivingEntity.java:1233-1234, defined at 1354-1376) run unconditionally after
        // `actuallyHurt`, regardless of `tookFullDamage`, and are independent of each other
        // -- a player attacker sets BOTH `lastHurtByMob` and `lastHurtByPlayer`. Both key off
        // `DamageSource.getEntity()` (the credited `cause`), not the direct entity.
        //
        // Known gap: `resolvePlayerResponsibleForDamage`'s tamed-wolf-owner-credit branch
        // (LivingEntity.java:1366-1372, crediting a wolf's owner when the wolf lands the
        // hit) is not ported -- `LivingEntity` here has no visibility into `Wolf`'s
        // tame/owner state. Only the direct-player branch is implemented.
        if let Some(attacker) = cause {
            let attacker_id = attacker.get_entity().entity_id;
            self.last_attacker_id.store(attacker_id, Relaxed);
            self.last_attacked_time
                .store(self.entity.tick_count.load(Relaxed), Relaxed);

            let current_tick = world.level_info.load().day_time;
            if attacker.get_living_entity().is_some()
                && !damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_ANGER)
                && !(damage_type == DamageType::WIND_CHARGE
                    && self
                        .entity
                        .entity_type
                        .has_tag(&tag::EntityType::MINECRAFT_NO_ANGER_FROM_WIND_CHARGE))
            {
                self.last_hurt_by_mob_id.store(attacker_id, Relaxed);
                self.last_hurt_by_mob_time.store(current_tick, Relaxed);
            }

            if attacker.get_player().is_some() {
                self.last_hurt_by_player_id.store(attacker_id, Relaxed);
                self.last_hurt_by_player_time.store(current_tick, Relaxed);
            }
        }

        // Vanilla parity: broadcast/markHurt/knockback only happen when the hit was not
        // reduced to the invulnerability-window difference (`tookFullDamage`,
        // LivingEntity.java:1235-1250).
        if took_full_damage {
            let Some(server) = world.server.upgrade() else {
                return false;
            };
            let config = &server.advanced_config.pvp;

            if config.hurt_animation && !blocked {
                let entity_id = self.entity.entity_id;
                let hurt_yaw = source.map_or(0.0, |source| {
                    let src = source.get_entity().pos.load();
                    let tgt = self.entity.pos.load();
                    (src.z - tgt.z).atan2(src.x - tgt.x).to_degrees() as f32
                        - self.entity.yaw.load()
                });
                let hurt_event = SActorEvent {
                    target_runtime_id: VarULong(entity_id as u64),
                    event_id: ActorEventID::Hurt,
                    data: VarInt(0),
                    fire_at_position: None,
                };
                let hurt_animation = CHurtAnimation::new(entity_id.into(), hurt_yaw);
                world.send_to_tracking_players_and_self_editioned(
                    &self.entity,
                    &hurt_animation,
                    &hurt_event,
                );
            }

            if blocked {
                if let Some(sound) = blocking_component
                    .as_ref()
                    .and_then(|component| component.block_sound.as_ref())
                {
                    world.play_sound_event_fine(
                        sound,
                        SoundCategory::Players,
                        &self.entity.pos.load(),
                        1.0,
                        0.8 + rand::random::<f32>() * 0.4,
                    );
                }
            } else {
                world.broadcast_damage_event(
                    &self.entity,
                    i32::from(damage_type.id),
                    source.map(|e| e.get_entity().entity_id),
                    cause.map(|e| e.get_entity().entity_id),
                    position,
                );
            }

            // Vanilla parity: `dealDefaultKnockback` runs whenever NO_KNOCKBACK isn't set,
            // independent of whether the hit was fatal (LivingEntity.java:1247-1248).
            if !damage_type.has_tag(&tag::DamageType::MINECRAFT_NO_KNOCKBACK)
                && let Some(source) = source
            {
                let source_pos = source.get_entity().pos.load();
                let target_pos = self.entity.pos.load();
                let dx = source_pos.x - target_pos.x;
                let dz = source_pos.z - target_pos.z;
                let resistance = self.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE);
                self.entity.apply_knockback(
                    knockback_after_resistance(f64::from(0.4_f32), resistance),
                    dx,
                    dz,
                );
            }
        }

        // Vanilla parity: death-or-hurt-sound decision runs after health has been applied
        // above, using the entity's real post-hit state (LivingEntity.java:1252-1264).
        let is_dead = self.health.load() <= 0.0 || self.dead.load(Relaxed);

        if is_dead {
            let mut death_event =
                crate::plugin::api::events::entity::entity_death::EntityDeathEvent::new(
                    self.entity.entity_id,
                    0,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut death_event);
            }
            self.on_death(damage_type, source, cause, took_full_damage);
        } else if took_full_damage {
            world.play_sound_fine(
                self.hurt_sound(caller),
                SoundCategory::Players,
                &self.entity.pos.load(),
                1.0,
                self.get_pitch(),
            );
        }

        if damage_blocked > 0.0
            && damage_blocked < 3.4028235E37
            && let Some(player) = caller.get_player()
        {
            player.increment_stat(
                StatisticCategory::Custom,
                CustomStatistic::DamageBlockedByShield as i32,
                (damage_blocked * 10.0).round() as i32,
            );
        }
        let success = !blocked || amount > 0.0;
        if success {
            // LivingEntity.hurtServer records this AFTER die(). A catalyst handling
            // ENTITY_DIE reads the preceding successful damage source, if still fresh.
            self.last_damage_source_entity_id.store(
                cause.map_or(-1, |entity| entity.get_entity().entity_id),
                Relaxed,
            );
            self.last_damage_source_time.store(
                world
                    .level_time
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .world_age,
                Relaxed,
            );
        }
        success
    }

    pub fn damage(&self, caller: &dyn EntityBase, amount: f32, damage_type: DamageType) -> bool {
        self.damage_with_context(caller, amount, damage_type, None, None, None)
    }
}

impl EntityBase for LivingEntity {
    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, position, source, cause)
    }

    fn tick_in_void(&self, dyn_self: &dyn EntityBase) {
        dyn_self.damage(dyn_self, 4.0, DamageType::OUT_OF_WORLD);
    }

    fn get_default_gravity(&self) -> f64 {
        self.get_attribute_value(&Attributes::GRAVITY)
    }

    #[allow(clippy::too_many_lines)]
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        self.entity.tick(caller, server);
        if caller.get_player().is_some() && self.health.load() > 0.0 && !self.entity.is_in_wall() {
            let damage = self.entity.world.load().worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .damage_at(self.entity.pos.load(), self.entity.bounding_box.load());
            if let Some(damage) = damage {
                caller.damage(caller, damage, DamageType::OUTSIDE_BORDER);
            }
        }
        let stingers = self.stinger_count.load(Relaxed);
        if stingers > 0 {
            if self.remove_stinger_time.load(Relaxed) <= 0 {
                self.remove_stinger_time
                    .store(20 * (30 - stingers), Relaxed);
            }
            if self.remove_stinger_time.fetch_sub(1, Relaxed) - 1 <= 0 {
                self.set_stinger_count(stingers - 1);
            }
        }

        // LivingEntity.aiStep advances the swing every tick, for every living entity,
        // whether or not it is currently swinging -- that is what ends a swing once it
        // has run its duration.
        self.update_swing_time(caller);

        // Only tick movement if the entity is alive. This prevents a dead "corpse"
        // from continuing to be simulated (accumulating fall_distance/velocity).
        // We allow movement during death animation (20 ticks) so knockback is applied.
        let is_alive = !self.dead.load(Relaxed) && self.health.load() > 0.0;
        let in_death_animation = self.health.load() <= 0.0 && self.death_time.load(Relaxed) < 20;
        let is_player = self.entity.entity_type == &EntityType::PLAYER;
        if (is_alive || in_death_animation) && !is_player {
            self.tick_movement(caller);
            // Vanilla-like order: freeze logic runs after movement/collisions.
            self.entity.tick_frozen(caller);
        } else if is_alive {
            // Client-authoritative players skip `travel`, so decay pushed velocity like
            // vanilla to prevent it accumulating and launching the player.
            self.apply_travel_friction();

            let suffocating = self.entity.tick_block_collisions(caller);
            if suffocating {
                caller.damage(caller, 1.0, DamageType::IN_WALL);
            }

            // Players push other entities like any living entity.
            self.push_entities(caller);

            self.entity.tick_frozen(caller);
        }

        self.impulse_context.lock().unwrap_or_else(std::sync::PoisonError::into_inner).tick();

        // Coalesce velocity sends to once per tick.
        if self.entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            self.entity.send_velocity();
        }

        // TODO
        let player = caller.get_player();
        let is_player = player.is_some();

        if !is_player {
            self.entity.send_pos_rot();
        }

        let current_block_pos = self.entity.block_pos.load();
        if is_alive && self.last_block_pos.load() != Some(current_block_pos) {
            self.last_block_pos.store(Some(current_block_pos));
            self.on_changed_block(caller, current_block_pos);
        }

        self.tick_effects();

        if let Some(player) = caller.get_player() {
            let remaining_use_ticks = self.item_use_time.load(Ordering::Relaxed);
            if remaining_use_ticks > 0 {
                let item_in_use = self
                    .item_in_use
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if let Some(item) = item_in_use.as_ref() {
                    server
                        .item_registry
                        .on_use_tick(item, player, remaining_use_ticks);
                }
            }
        }

        // Current active item
        if self.item_use_time.load(Ordering::Relaxed) > 0
            && self.item_use_time.fetch_sub(1, Ordering::Relaxed) <= 1
        {
            let item_in_use = self
                .item_in_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(item) = item_in_use.as_ref() {
                // Consume item
                if let Some(food) = item.get_data_component::<FoodImpl>()
                    && let Some(player) = caller.get_player()
                {
                    player
                        .hunger_manager
                        .eat(player, food.nutrition as u8, food.saturation);
                    self.entity.world.load().play_bedrock_level_sound(
                        "burp",
                        &self.entity.pos.load(),
                        -1,
                    );
                }

                // Vanilla: `Consumable.onConsume` always plays the component's configured
                // sound (`entity.generic.eat`/`entity.generic.drink`/etc.) once, regardless
                // of whether the item is food (Consumable.java:79, emitParticlesAndSounds
                // called with the finishing particle count). The consuming player already
                // predicts this sound locally, so only broadcast it to everyone else,
                // matching `Player.playSound` excluding `this` (Player.java:399).
                if let Some(consumable) = item.get_data_component::<ConsumableImpl>()
                    && let Some(player) = caller.get_player()
                {
                    player.world().play_sound_event_expect(
                        player,
                        &consumable.sound_event,
                        SoundCategory::Players,
                        &self.entity.pos.load(),
                    );
                }

                self.apply_consumable_effects(caller, item);

                // Handle potion consumption
                if item
                    .get_data_component::<pumpkin_data::data_component_impl::PotionContentsImpl>()
                    .is_some()
                {
                    let effects = crate::item::potion::PotionContents::read_potion_effects(item);
                    crate::item::potion::PotionContents::apply_effects_to(
                        self,
                        effects,
                        1.0,
                        crate::item::potion::PotionApplicationSource::Normal,
                    );
                }

                // Vanilla: `ItemStack.applyAfterUseComponentSideEffects` converts the spent
                // stack into the item's `use_remainder` (bowls for stews, glass bottles for
                // potions/honey bottles, buckets for milk) once the stack is fully consumed
                // (ItemStack.java:396-410, UseRemainder.java:15-36). In creative mode the
                // stack is never shrunk (`ItemStack.consume`, ItemStack.java:1082-1086), so
                // it is never converted either.
                let remainder_item = item
                    .get_data_component::<UseRemainderImpl>()
                    .and_then(|remainder| Item::from_registry_key(&remainder.item));

                if let Some(player) = caller.get_player() {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::ConsumeItem {
                            item_id: format!("minecraft:{}", item.item.registry_key),
                        },
                    );

                    // Prefer modifying the exact stack that matches the consumed item:
                    // 1) selected hotbar (held_item)
                    // 2) off-hand
                    // 3) fallback to active_hand if the above didn't match
                    let mut handled = false;

                    // Check main hand (hotbar selected)
                    let mut held = player.inventory.held_item();
                    if held.are_items_and_components_equal(item) {
                        if let Some(remainder_item) = remainder_item {
                            if player.gamemode.load() != GameMode::Creative {
                                held.decrement(1);
                                if held.is_empty() {
                                    held = ItemStack::new(1, remainder_item);
                                }
                            }
                        } else {
                            held.decrement_unless_creative(player.gamemode.load(), 1);
                        }
                        player.inventory.set_held_item(held);
                        handled = true;
                    }

                    if !handled {
                        // Check off-hand
                        let mut off_hand = player.inventory.off_hand_item();
                        if off_hand.are_items_and_components_equal(item) {
                            if let Some(remainder_item) = remainder_item {
                                if player.gamemode.load() != GameMode::Creative {
                                    off_hand.decrement(1);
                                    if off_hand.is_empty() {
                                        off_hand = ItemStack::new(1, remainder_item);
                                    }
                                }
                            } else {
                                off_hand.decrement_unless_creative(player.gamemode.load(), 1);
                            }
                            player.inventory.set_stack_in_hand(Hand::Left, off_hand);
                            handled = true;
                        }
                    }

                    if !handled {
                        // Use stored active_hand (as a fallback)
                        let active_hand = *self
                            .active_hand
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let hand_to_modify = active_hand.unwrap_or(Hand::Right);
                        let mut item_stack = self.get_stack_in_hand(caller, hand_to_modify);

                        if let Some(remainder_item) = remainder_item {
                            if player.gamemode.load() != GameMode::Creative {
                                item_stack.decrement(1);
                                if item_stack.is_empty() {
                                    item_stack = ItemStack::new(1, remainder_item);
                                }
                            }
                        } else {
                            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        }
                        player
                            .inventory
                            .set_stack_in_hand(hand_to_modify, item_stack);
                    }

                    if let Some(cooldown) = item.get_use_cooldown() {
                        let group = cooldown
                            .cooldown_group
                            .clone()
                            .unwrap_or_else(|| item.item.registry_key.to_string());
                        player.start_cooldown(group, (cooldown.seconds * 20.0) as i32);
                    }
                }

                self.clear_active_hand();
            }
        }

        if self.hurt_cooldown.load(Relaxed) > 0 {
            self.hurt_cooldown.fetch_sub(1, Relaxed);
        }
        if self.health.load() <= 0.0 {
            let time = self
                .death_time
                .fetch_update(Relaxed, Relaxed, |time| Some(time.saturating_add(1)))
                .unwrap_or_else(|time| time)
                .saturating_add(1);
            if self.entity.entity_type == &EntityType::PLAYER {
                // Bedrock keeps a dead remote player actor in its death pose.
                // Remove Java players after the animation so respawn can
                // recreate a live, interactable actor with the same identity.
                if time == 10 {
                    self.entity
                        .world
                        .load()
                        .despawn_dead_java_player_for_bedrock(&self.entity);
                }
                // Players remain part of the world until their client requests a
                // respawn. Removing one here breaks reconnecting while dead.
                return;
            }
            // Only send death particles once (on the exact tick death_time reaches 20)
            // and then remove the entity, preventing entity_event spam.
            if time == 20 && !self.entity.removed.swap(true, Ordering::Relaxed) {
                self.entity.world.load().send_entity_status(
                    &self.entity,
                    EntityStatus::Death,
                    Some(ActorEventID::Death),
                );
                self.entity.remove();
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(self)
    }

    fn is_pushable(&self) -> bool {
        self.health.load() > 0.0 && !self.dead.load(Relaxed)
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub const SPEED_MODIFIER_SPRINTING_ID: &str = "minecraft:sprinting";
pub const SPEED_MODIFIER_SPRINTING_AMOUNT: f64 = 0.300_000_011_920_928_96;

impl LivingEntity {
    pub fn set_sprinting(&self, is_sprinting: bool) {
        self.entity.set_sprinting(is_sprinting);
        self.update_attribute(&Attributes::MOVEMENT_SPEED, |speed| {
            speed.remove_modifier(SPEED_MODIFIER_SPRINTING_ID);
            if is_sprinting {
                speed.add_or_replace_modifier(Modifier {
                    id: SPEED_MODIFIER_SPRINTING_ID.to_string(),
                    amount: SPEED_MODIFIER_SPRINTING_AMOUNT,
                    operation: ModifierOperation::MultiplyTotal,
                });
            }
        });
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MOVEMENT_SPEED],
        );
    }

    #[must_use]
    pub fn get_block_speed_factor(&self) -> f32 {
        let efficiency = self.get_attribute_value(&Attributes::MOVEMENT_EFFICIENCY) as f32;
        let super_factor = self.entity.get_block_speed_factor();
        super_factor + efficiency * (1.0 - super_factor)
    }

    /// Applies data-driven `apply_effects` consume effects after an item completes use.
    /// Vanilla: `Consumable.onConsume` invokes every configured effect server-side.
    fn apply_consumable_effects(&self, caller: &dyn EntityBase, item: &ItemStack) {
        let Some(consumable) = item.get_data_component::<ConsumableImpl>() else {
            return;
        };

        for consume_effect in consumable.effects.iter() {
            match consume_effect {
                ConsumeEffect::ApplyEffects((effects, probability)) => {
                    if !consume_effect_probability_applies(*probability, rand::random()) {
                        continue;
                    }

                    for effect in effects.iter() {
                        let Some(effect_type) =
                            StatusEffect::from_minecraft_name(&effect.effect_id)
                        else {
                            continue;
                        };
                        let Ok(amplifier) = u8::try_from(effect.amplifier) else {
                            continue;
                        };

                        self.add_effect(Effect {
                            effect_type,
                            duration: effect.duration,
                            amplifier,
                            ambient: effect.ambient,
                            show_particles: effect.show_particles,
                            show_icon: effect.show_icon,
                            blend: false,
                        });
                    }
                }
                ConsumeEffect::ClearAllEffects => {
                    self.reset_effects_and_attributes();
                }
                ConsumeEffect::RemoveEffects(idset) => {
                    if let pumpkin_data::data_component_impl::IDSet::IDs(ids) = idset {
                        for effect_type in ids.iter() {
                            self.remove_effect(effect_type);
                        }
                    }
                }
                ConsumeEffect::TeleportRandomly(diameter) => {
                    // Java Edition dismounts the consumer before random teleport attempts.
                    let vehicle = caller
                        .get_entity()
                        .vehicle
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if let Some(vehicle) = vehicle {
                        vehicle
                            .get_entity()
                            .remove_passenger_sync(caller.get_entity().entity_id);
                        if caller.get_entity().has_vehicle() {
                            continue;
                        }
                    }

                    let center = self.entity.pos.load();
                    let Some(pos) = self.find_random_teleport_target(*diameter) else {
                        continue;
                    };
                    let (yaw, pitch) = (self.entity.yaw.load(), self.entity.pitch.load());
                    let world = self.entity.world.load_full();
                    caller.teleport(pos, Some(yaw), Some(pitch), world.clone());

                    let destination = self.entity.pos.load();
                    if destination != center {
                        self.fall_distance.store(0.0);
                        self.reset_impulse_context();
                        // Vanilla broadcasts entity event 46 (teleport particles) on success.
                        world.send_entity_status(&self.entity, EntityStatus::Teleport, None);
                        world.emit_game_event("teleport", center);
                        world.play_sound(
                            Sound::ItemChorusFruitTeleport,
                            SoundCategory::Players,
                            &destination,
                        );
                    }
                }
                ConsumeEffect::PlaySound(sound) => {
                    // Vanilla broadcasts to everyone at the consumer's block position,
                    // including the consumer itself (`level.playSound(null, ...)`,
                    // PlaySoundConsumeEffect.java:29), unlike the implicit finishing sound
                    // which excludes the consumer.
                    self.entity.world.load().play_sound_event(
                        sound,
                        SoundCategory::Players,
                        &self.entity.pos.load(),
                    );
                }
            }
        }
    }

    fn find_random_teleport_target(&self, diameter: f32) -> Option<Vector3<f64>> {
        let center = self.entity.pos.load();
        let world = self.entity.world.load();
        let bottom_y = world.get_bottom_y();
        let top_y = world.get_top_y();
        let dimensions = self.entity.entity_dimension.load();
        let mut rng = rand::rng();

        'attempts: for _ in 0..Self::RANDOM_TELEPORT_ATTEMPTS {
            let target_x = random_teleport_coordinate(center.x, diameter, rng.random());
            let target_z = random_teleport_coordinate(center.z, diameter, rng.random());
            let sampled_y = random_teleport_coordinate(center.y, diameter, rng.random())
                .clamp(f64::from(bottom_y + 1), f64::from(top_y));
            let mut block_y = sampled_y.floor() as i32;
            let block_x = target_x.floor() as i32;
            let block_z = target_z.floor() as i32;

            loop {
                if block_y <= bottom_y {
                    continue 'attempts;
                }

                let below = BlockPos::new(block_x, block_y - 1, block_z);
                let Some(below_state) = world.get_block_state_if_loaded(&below) else {
                    continue 'attempts;
                };
                if below_state.is_solid() {
                    break;
                }
                block_y -= 1;
            }

            let target = Vector3::new(target_x, f64::from(block_y), target_z);
            let bounding_box = BoundingBox::new_from_pos(target.x, target.y, target.z, &dimensions);

            for block_pos in
                BlockPos::iterate(bounding_box.min_block_pos(), bounding_box.max_block_pos())
            {
                if world.get_block_state_if_loaded(&block_pos).is_none()
                    || world.get_fluid(&block_pos).id != Fluid::EMPTY.id
                {
                    continue 'attempts;
                }
            }

            if world.is_space_empty(bounding_box) {
                return Some(target);
            }
        }

        None
    }
}

fn random_teleport_coordinate(center: f64, diameter: f32, random: f64) -> f64 {
    center + (random - 0.5) * f64::from(diameter)
}

fn attributes_by_id(id: u8) -> Option<&'static Attributes> {
    Attributes::ALL.iter().find(|attr| attr.id == id)
}

fn push_unique_attribute(touched: &mut Vec<Attributes>, attr: &Attributes) {
    if !touched.iter().any(|existing| existing.id == attr.id) {
        touched.push(attr.clone());
    }
}

const fn attribute_modifier_slot_matches(
    modifier_slot: &AttributeModifierSlot,
    equipment_slot: &EquipmentSlot,
) -> bool {
    match modifier_slot {
        AttributeModifierSlot::Any => true,
        AttributeModifierSlot::MainHand => matches!(equipment_slot, EquipmentSlot::MainHand(_)),
        AttributeModifierSlot::OffHand => matches!(equipment_slot, EquipmentSlot::OffHand(_)),
        AttributeModifierSlot::Hand => {
            matches!(
                equipment_slot,
                EquipmentSlot::MainHand(_) | EquipmentSlot::OffHand(_)
            )
        }
        AttributeModifierSlot::Feet => matches!(equipment_slot, EquipmentSlot::Feet(_)),
        AttributeModifierSlot::Legs => matches!(equipment_slot, EquipmentSlot::Legs(_)),
        AttributeModifierSlot::Chest => matches!(equipment_slot, EquipmentSlot::Chest(_)),
        AttributeModifierSlot::Head => matches!(equipment_slot, EquipmentSlot::Head(_)),
        AttributeModifierSlot::Armor => matches!(
            equipment_slot,
            EquipmentSlot::Feet(_)
                | EquipmentSlot::Legs(_)
                | EquipmentSlot::Chest(_)
                | EquipmentSlot::Head(_)
        ),
        AttributeModifierSlot::Body => matches!(equipment_slot, EquipmentSlot::Body(_)),
        AttributeModifierSlot::Saddle => matches!(equipment_slot, EquipmentSlot::Saddle(_)),
    }
}

/// Mirrors vanilla's strict `random < probability` consume-effect gate.
const fn consume_effect_probability_applies(probability: f32, random: f32) -> bool {
    random < probability
}

/// LivingEntity.calculateFallDamage in Mojang Java 26.2.
pub(super) fn calculate_fall_damage(
    distance: f64,
    safe: f64,
    block_multiplier: f64,
    attribute_multiplier: f64,
) -> f32 {
    ((distance + 1.0E-6 - safe) * block_multiplier * attribute_multiplier).floor() as f32
}

#[cfg(test)]
mod consumable_effect_tests {
    use super::{consume_effect_probability_applies, random_teleport_coordinate};

    #[test]
    fn consumable_effect_probability_matches_vanilla_strict_threshold() {
        assert!(!consume_effect_probability_applies(0.0, 0.0));
        assert!(consume_effect_probability_applies(1.0, 0.999));
        assert!(consume_effect_probability_applies(0.5, 0.499));
        assert!(!consume_effect_probability_applies(0.5, 0.5));
    }

    #[test]
    fn random_teleport_coordinate_uses_full_diameter() {
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.0), 2.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.5), 10.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 1.0), 18.0);
    }
}
/// Returns `true` if `damage_type` is in `#minecraft:bypasses_armor` (1.21.11).
/// These sources bypass armor entirely (fall, drown, freeze, etc.).
pub(crate) const fn bypasses_armor_durability(damage_type: &DamageType) -> bool {
    // Bitmask lookup: O(1) with two instructions (shift + AND), no array scan.
    // DamageType IDs can exceed 31; use u64 for sufficient range.
    // TODO: Make data-driven once the data pack system can handle it without performance regressions.
    // Compile-time assertions: ensure all bypassing types fit in u64 bitmask.
    const _: () = assert!(
        DamageType::FALL.id < 64
            && DamageType::FLY_INTO_WALL.id < 64
            && DamageType::ON_FIRE.id < 64
            && DamageType::IN_WALL.id < 64
            && DamageType::CRAMMING.id < 64
            && DamageType::DROWN.id < 64
            && DamageType::GENERIC.id < 64
            && DamageType::WITHER.id < 64
            && DamageType::DRAGON_BREATH.id < 64
            && DamageType::STARVE.id < 64
            && DamageType::ENDER_PEARL.id < 64
            && DamageType::FREEZE.id < 64
            && DamageType::STALAGMITE.id < 64
            && DamageType::MAGIC.id < 64
            && DamageType::INDIRECT_MAGIC.id < 64
            && DamageType::OUT_OF_WORLD.id < 64
            && DamageType::GENERIC_KILL.id < 64
            && DamageType::SONIC_BOOM.id < 64
            && DamageType::OUTSIDE_BORDER.id < 64,
        "One or more bypass DamageType IDs exceed u64 bitmask width (>= 64)"
    );
    const BYPASS_MASK: u64 = (1u64 << DamageType::FALL.id)
        | (1u64 << DamageType::FLY_INTO_WALL.id)
        | (1u64 << DamageType::ON_FIRE.id)
        | (1u64 << DamageType::IN_WALL.id)
        | (1u64 << DamageType::CRAMMING.id)
        | (1u64 << DamageType::DROWN.id)
        | (1u64 << DamageType::GENERIC.id)
        | (1u64 << DamageType::WITHER.id)
        | (1u64 << DamageType::DRAGON_BREATH.id)
        | (1u64 << DamageType::STARVE.id)
        | (1u64 << DamageType::ENDER_PEARL.id)
        | (1u64 << DamageType::FREEZE.id)
        | (1u64 << DamageType::STALAGMITE.id)
        | (1u64 << DamageType::MAGIC.id)
        | (1u64 << DamageType::INDIRECT_MAGIC.id)
        | (1u64 << DamageType::OUT_OF_WORLD.id)
        | (1u64 << DamageType::GENERIC_KILL.id)
        | (1u64 << DamageType::SONIC_BOOM.id)
        | (1u64 << DamageType::OUTSIDE_BORDER.id);
    (damage_type.id < 64) && ((BYPASS_MASK >> damage_type.id) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bypasses_armor_durability ─────────────────────────────────────

    /// Every member of `minecraft:bypasses_armor` (1.21.11) must return `true`.
    #[test]
    fn bypasses_armor_durability_returns_true_for_tag_members() {
        // Exact contents of the minecraft:bypasses_armor tag in 1.21.11.
        let bypassing: &[DamageType] = &[
            DamageType::ON_FIRE,
            DamageType::IN_WALL,
            DamageType::CRAMMING,
            DamageType::DROWN,
            DamageType::FLY_INTO_WALL,
            DamageType::GENERIC,
            DamageType::WITHER,
            DamageType::DRAGON_BREATH,
            DamageType::STARVE,
            DamageType::FALL,
            DamageType::ENDER_PEARL,
            DamageType::FREEZE,
            DamageType::STALAGMITE,
            DamageType::MAGIC,
            DamageType::INDIRECT_MAGIC,
            DamageType::OUT_OF_WORLD,
            DamageType::GENERIC_KILL,
            DamageType::SONIC_BOOM,
            DamageType::OUTSIDE_BORDER,
        ];
        for dt in bypassing {
            assert!(
                bypasses_armor_durability(dt),
                "{} should bypass armor durability",
                dt.message_id
            );
        }
    }

    /// Physical/combat damage types must NOT bypass armor durability.
    #[test]
    fn bypasses_armor_durability_returns_false_for_physical_sources() {
        let physical: &[DamageType] = &[
            DamageType::MOB_ATTACK,
            DamageType::PLAYER_ATTACK,
            DamageType::ARROW,
            DamageType::CACTUS,
            DamageType::SWEET_BERRY_BUSH,
            DamageType::LAVA,
            DamageType::EXPLOSION,
            DamageType::PLAYER_EXPLOSION,
            DamageType::LIGHTNING_BOLT,
            DamageType::FIREBALL,
            DamageType::THORNS,
            DamageType::TRIDENT,
        ];
        for dt in physical {
            assert!(
                !bypasses_armor_durability(dt),
                "{} should NOT bypass armor durability",
                dt.message_id
            );
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_zombie_family_sounds() {
        let cases = [
            (&EntityType::ZOMBIE, Sound::EntityZombieHurt),
            (&EntityType::DROWNED, Sound::EntityDrownedHurt),
            (&EntityType::HUSK, Sound::EntityHuskHurt),
            (
                &EntityType::ZOMBIE_VILLAGER,
                Sound::EntityZombieVillagerHurt,
            ),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_enderman_hurt_sound() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ENDERMAN),
            Sound::EntityEndermanHurt
        );
    }

    #[test]
    fn hurt_sound_for_entity_uses_skeleton_family_sounds() {
        let cases = [
            (&EntityType::SKELETON, Sound::EntitySkeletonHurt),
            (&EntityType::BOGGED, Sound::EntityBoggedHurt),
            (&EntityType::PARCHED, Sound::EntityParchedHurt),
            (
                &EntityType::WITHER_SKELETON,
                Sound::EntityWitherSkeletonHurt,
            ),
            (&EntityType::STRAY, Sound::EntityStrayHurt),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_defaults_to_generic_hurt() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ITEM),
            Sound::EntityGenericHurt
        );
    }

    #[test]
    fn regeneration_particle_metadata_uses_vanilla_argb_color() {
        let effect = Effect {
            effect_type: &StatusEffect::REGENERATION,
            duration: 200,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        };
        let metadata = pumpkin_protocol::java::client::play::Metadata::new(
            tracked_data::living_entity::EFFECT_PARTICLES,
            EffectParticles(vec![EffectParticle::from_effect(&effect)]),
        );
        let mut bytes = Vec::new();

        metadata
            .write(
                &mut bytes,
                &pumpkin_util::version::JavaMinecraftVersion::V_26_2,
            )
            .unwrap();

        assert_eq!(bytes, [10, 17, 1, 28, 0xff, 0xcd, 0x5c, 0xab]);
    }

    // ── damage pipeline: armor formula, protection cap, lastHurt rule ────────────

    /// Pins `CombatRules.getDamageAfterAbsorb` (CombatRules.java:16-32): armor toughness
    /// changes the `damage / toughness` term, which in turn changes `realArmor` before it's
    /// divided by `ARMOR_PROTECTION_DIVIDER` (25.0).
    #[test]
    fn armor_formula_toughness_matches_vanilla_combat_rules() {
        // toughness = 2.0 + 8.0/4.0 = 4.0
        // realArmor = clamp(20.0 - 20.0/4.0, 4.0, 20.0) = clamp(15.0, 4.0, 20.0) = 15.0
        // fraction = 15.0/25.0 = 0.6 -> damage * 0.4
        let result = CombatRules::get_damage_after_absorb(20.0, 20.0, 8.0, 0);
        assert!((result - 8.0).abs() < 1e-3, "expected 8.0, got {result}");
    }

    /// Pins the `MIN_ARMOR_RATIO` floor (CombatRules.java:13,20): against enough damage,
    /// `realArmor` bottoms out at `totalArmor * 0.2` instead of going negative.
    #[test]
    fn armor_formula_floors_at_min_armor_ratio_against_heavy_damage() {
        // toughness = 2.0; realArmor = clamp(20.0 - 1000.0/2.0, 4.0, 20.0) = 4.0 (floor)
        // fraction = 4.0/25.0 = 0.16 -> damage * 0.84
        let result = CombatRules::get_damage_after_absorb(1000.0, 20.0, 0.0, 0);
        assert!(
            (result - 840.0).abs() < 1e-1,
            "expected 840.0, got {result}"
        );
    }

    /// Pins the Breach enchantment's armor-effectiveness reduction
    /// (CombatRules.java:22-27 in vanilla via `EnchantmentHelper.modifyArmorEffectiveness`;
    /// mirrored here as `min(level * 0.15, 1.0)` off the armor fraction).
    #[test]
    fn armor_formula_breach_reduces_armor_fraction() {
        // Pre-breach: toughness = 2.0; realArmor = clamp(20.0 - 10.0/2.0, 4.0, 20.0) = 15.0
        // fraction = 0.6; breach 4 -> reduction = min(4*0.15, 1.0) = 0.6
        // modified fraction = clamp(0.6 * (1.0 - 0.6), 0.0, 1.0) = 0.24 -> damage * 0.76
        let result = CombatRules::get_damage_after_absorb(10.0, 20.0, 0.0, 4);
        assert!((result - 7.6).abs() < 1e-3, "expected 7.6, got {result}");
    }

    /// Pins the enchantment-protection cap (CombatRules.java:34-37): total magic armor
    /// above 20 (e.g. several protection enchantments stacked across 4 armor pieces) is
    /// clamped to 20 before being divided by `ARMOR_PROTECTION_DIVIDER` (25.0), not applied
    /// in full.
    #[test]
    fn magic_absorb_protection_cap_clamps_total_at_twenty() {
        // realArmor = clamp(25.0, 0.0, 20.0) = 20.0 -> fraction = 20.0/25.0 = 0.8
        let capped = CombatRules::get_damage_after_magic_absorb(20.0, 25.0);
        assert!(
            (capped - 4.0).abs() < 1e-3,
            "expected 4.0 (cap applied), got {capped}"
        );

        // Uncapped equivalent would give a different (wrong) answer if the cap were
        // missing: 20.0 * (1.0 - 25.0/25.0) = 0.0. The capped result must differ from this.
        assert!((capped - 0.0).abs() > 1e-3);
    }

    /// Pins the lower clamp of `getDamageAfterMagicAbsorb` (CombatRules.java:35): negative
    /// magic armor does not amplify damage past the original amount.
    #[test]
    fn magic_absorb_clamps_negative_armor_to_zero() {
        let result = CombatRules::get_damage_after_magic_absorb(20.0, -5.0);
        assert!((result - 20.0).abs() < 1e-3, "expected 20.0, got {result}");
    }

    /// Pins `LivingEntity.hurtServer`'s `lastHurt` rule (LivingEntity.java:1216-1231) via
    /// the real `LivingEntity::resolve_hurt_cooldown` helper that `damage_with_context`
    /// calls. Outside the invulnerability window (`cooldown_active == false`), every hit is
    /// a full hit regardless of any previous `lastHurt`.
    #[test]
    fn resolve_hurt_cooldown_outside_window_is_always_a_full_hit() {
        assert_eq!(
            LivingEntity::resolve_hurt_cooldown(10.0, 999.0, false),
            Some((10.0, true))
        );
    }

    /// A new hit that is smaller than or equal to `lastHurt` while still inside the
    /// invulnerability window is rejected outright (`damage <= this.lastHurt` ->
    /// `return false`, LivingEntity.java:1218-1220) -- it must not silently apply zero
    /// damage or clamp, it must reject the hit entirely.
    #[test]
    fn resolve_hurt_cooldown_rejects_smaller_or_equal_hits_inside_window() {
        assert_eq!(LivingEntity::resolve_hurt_cooldown(5.0, 5.0, true), None);
        assert_eq!(LivingEntity::resolve_hurt_cooldown(3.0, 5.0, true), None);
    }

    /// The critical multi-hit-exchange case: a LARGER hit landing inside the invulnerability
    /// window applies only the DIFFERENCE `amount - lastHurt`, not the full new amount and
    /// not zero (LivingEntity.java:1222-1224). Getting this wrong changes every multi-hit
    /// exchange's numbers.
    #[test]
    fn resolve_hurt_cooldown_applies_only_the_difference_for_a_larger_hit() {
        assert_eq!(
            LivingEntity::resolve_hurt_cooldown(8.0, 5.0, true),
            Some((3.0, false))
        );
    }
}

#[cfg(test)]
mod swing_tests {
    use super::should_restart_swing;

    /// The default swing is 6 ticks, so its halfway point is 3.
    const DEFAULT_DURATION: i32 = 6;

    #[test]
    fn an_idle_entity_always_starts_a_swing() {
        assert!(should_restart_swing(false, 0, DEFAULT_DURATION));
        assert!(should_restart_swing(false, 4, DEFAULT_DURATION));
    }

    #[test]
    fn a_swing_in_its_first_half_is_not_restarted() {
        // This is the case that was missing: a mob attacking every tick kept resetting
        // the animation to frame zero, so the arm never visibly moved.
        for swing_time in 0..DEFAULT_DURATION / 2 {
            assert!(
                !should_restart_swing(true, swing_time, DEFAULT_DURATION),
                "tick {swing_time} is in the first half and must not restart"
            );
        }
    }

    #[test]
    fn a_swing_past_halfway_may_restart() {
        for swing_time in DEFAULT_DURATION / 2..DEFAULT_DURATION {
            assert!(
                should_restart_swing(true, swing_time, DEFAULT_DURATION),
                "tick {swing_time} is past halfway and may restart"
            );
        }
    }

    #[test]
    fn a_swing_started_this_tick_may_restart() {
        // swingTime is -1 for the tick a swing begins, before updateSwingTime runs.
        assert!(should_restart_swing(true, -1, DEFAULT_DURATION));
    }

    #[test]
    fn a_haste_shortened_swing_moves_its_halfway_point() {
        // Haste I: 6 - (1 + 0) = 5 ticks, halving to 2 rather than 3.
        let hasted = 5;
        assert!(!should_restart_swing(true, 1, hasted));
        assert!(should_restart_swing(true, 2, hasted));
    }
}
