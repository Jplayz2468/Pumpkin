use super::*;
use crate::{block::registry::BlockRegistry, world::World};
use arc_swap::ArcSwap;
use pumpkin_data::{dimension::Dimension, item::Item};
use pumpkin_util::world_seed::Seed;
use pumpkin_world::{level::Level, world_info::LevelData};

fn living(path: &std::path::Path) -> LivingEntity {
    let level = Level::from_root_folder(
        &pumpkin_config::world::LevelConfig::default(),
        path.to_path_buf(),
        262,
        Dimension::OVERWORLD,
    );
    let world = World::load(
        level,
        Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
        Dimension::OVERWORLD,
        Arc::new(BlockRegistry::default()),
        std::sync::Weak::new(),
    );
    LivingEntity::new(Entity::new(
        world,
        Vector3::new(0.0, 64.0, 0.0),
        &EntityType::PLAYER,
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn computed_attributes_match_java_values_and_ranges() {
    let directory = tempfile::tempdir().unwrap();
    let living = living(directory.path());
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("attribute_value_cases.json")).unwrap();
    let slots = cases["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 88);
    for case in slots {
        let group = AttributeModifierSlot::from_name(case["group"].as_str().unwrap()).unwrap();
        let slot = match case["slot"].as_str().unwrap() {
            "mainhand" => EquipmentSlot::MAIN_HAND,
            "offhand" => EquipmentSlot::OFF_HAND,
            "feet" => EquipmentSlot::FEET,
            "legs" => EquipmentSlot::LEGS,
            "chest" => EquipmentSlot::CHEST,
            "head" => EquipmentSlot::HEAD,
            "body" => EquipmentSlot::BODY,
            "saddle" => EquipmentSlot::SADDLE,
            name => panic!("unknown slot: {name}"),
        };
        assert_eq!(
            attribute_modifier_slot_matches(&group, &slot),
            case["matches"].as_bool().unwrap(),
            "{case}"
        );
    }
    let cases = cases["values"].as_array().unwrap();
    assert_eq!(cases.len(), 1600);
    let double = |value: &serde_json::Value| {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    };
    for (index, case) in cases.iter().enumerate() {
        let name = format!("minecraft:{}", case["attribute"].as_str().unwrap());
        let attribute = Attributes::ALL.iter().find(|a| a.name == name).unwrap();
        living.update_attribute(attribute, |instance| {
            *instance = AttributeInstance::new(attribute.default_value);
            instance.set_base_value(double(&case["base"]));
            for modifier in case["modifiers"].as_array().unwrap() {
                instance.add_or_replace_modifier(Modifier {
                    id: modifier["id"].as_str().unwrap().to_owned(),
                    amount: double(&modifier["amount"]),
                    operation: match modifier["operation"].as_u64().unwrap() {
                        0 => ModifierOperation::Add,
                        1 => ModifierOperation::MultiplyBase,
                        2 => ModifierOperation::MultiplyTotal,
                        _ => unreachable!(),
                    },
                });
            }
        });
        let expected = double(&case["value"]).to_bits();
        assert_eq!(
            living.get_attribute_value(attribute).to_bits(),
            expected,
            "case {index}: {name}"
        );
        assert_eq!(
            living.get_attribute_value(attribute).to_bits(),
            expected,
            "cached case {index}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn equipment_changes_preserve_effects_permanent_modifiers_and_slot_filters() {
    let directory = tempfile::tempdir().unwrap();
    let living = living(directory.path());
    living.update_attribute(&Attributes::ATTACK_DAMAGE, |instance| {
        instance.add_permanent_modifier(Modifier {
            id: "example:permanent_damage".into(),
            amount: 2.0,
            operation: ModifierOperation::Add,
        });
    });
    living.add_effect(Effect {
        effect_type: &StatusEffect::STRENGTH,
        duration: 200,
        amplifier: 0,
        ambient: false,
        show_particles: false,
        show_icon: false,
        blend: false,
    });
    let sword = ItemStack::new(1, &Item::DIAMOND_SWORD);
    living.send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, sword.clone())]);
    assert_eq!(living.get_attribute_value(&Attributes::ATTACK_DAMAGE), 12.0);
    assert!(
        (living.get_attribute_value(&Attributes::ATTACK_SPEED) - 1.599_999_904_632_568_4).abs()
            < 1e-12
    );
    living.send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, sword)]);
    assert_eq!(living.get_attribute_value(&Attributes::ATTACK_DAMAGE), 12.0);
    // A helmet cannot contribute its head-only armor modifiers from the main hand.
    living.send_equipment_changes(&[(
        EquipmentSlot::MAIN_HAND,
        ItemStack::new(1, &Item::DIAMOND_HELMET),
    )]);
    assert_eq!(living.get_attribute_value(&Attributes::ATTACK_DAMAGE), 6.0);
    assert_eq!(living.get_attribute_value(&Attributes::ATTACK_SPEED), 4.0);
    assert_eq!(living.get_attribute_value(&Attributes::ARMOR), 0.0);
    living.remove_effect(&StatusEffect::STRENGTH);
    assert_eq!(living.get_attribute_value(&Attributes::ATTACK_DAMAGE), 3.0);
    let attributes = living.attributes.read().unwrap();
    let saved = attributes
        .get(&Attributes::ATTACK_DAMAGE.id)
        .unwrap()
        .write_nbt("minecraft:attack_damage");
    assert_eq!(saved.get_list("modifiers").unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn armor_damage_uses_computed_values_and_floors_armor() {
    let directory = tempfile::tempdir().unwrap();
    let living = living(directory.path());
    living.set_attribute_base(&Attributes::ARMOR, 10.5);
    living.update_attribute(&Attributes::ARMOR, |instance| {
        instance.add_or_replace_modifier(Modifier {
            id: "example:armor".into(),
            amount: 0.5,
            operation: ModifierOperation::MultiplyTotal,
        });
    });
    living.set_attribute_base(&Attributes::ARMOR_TOUGHNESS, 8.0);
    // Armor 15.75 floors to 15; real armor = 15 - 20/(2+8/4) = 10.
    let damage =
        living.get_damage_after_armor_absorb(20.0, &DamageType::PLAYER_ATTACK, &living, None);
    assert_eq!(damage, 12.0);
    assert_eq!(
        living.get_damage_after_armor_absorb(20.0, &DamageType::FALL, &living, None),
        20.0
    );
}
