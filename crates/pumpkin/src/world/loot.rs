use pumpkin_data::BlockState;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::loot_table::{LootBonusFormula, LootCondition, LootEntry, LootTable};
use pumpkin_util::random::legacy_rand::LegacyRand;
#[cfg(test)]
use pumpkin_util::random::xoroshiro128::Xoroshiro;

// Only the operations used by loot, allowing a borrowed level or named source.
trait LootRandom {
    fn next_bounded_i32(&mut self, bound: i32) -> i32;
    fn next_f32(&mut self) -> f32;
    fn next_bool(&mut self) -> bool;
}
impl<T: pumpkin_util::random::RandomImpl> LootRandom for T {
    fn next_bounded_i32(&mut self, bound: i32) -> i32 {
        pumpkin_util::random::RandomImpl::next_bounded_i32(self, bound)
    }
    fn next_f32(&mut self) -> f32 {
        pumpkin_util::random::RandomImpl::next_f32(self)
    }
    fn next_bool(&mut self) -> bool {
        pumpkin_util::random::RandomImpl::next_bool(self)
    }
}

fn with_loot_random<R>(
    world: &super::World,
    table: &LootTable,
    seed: i64,
    action: impl FnOnce(&mut dyn LootRandom) -> R,
) -> R {
    if seed != 0 {
        return action(&mut LegacyRand::from_seed(seed as u64));
    }
    if let Some(key) = table.random_sequence
        && let Some(server) = world.server.upgrade()
    {
        let key = pumpkin_util::identifier::Identifier::parse_static(key);
        let mut sequences = server
            .random_sequences
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let seed = server.level_info.load().world_gen_settings.seed;
        return action(&mut sequences.get_or_create(&key, seed).rng);
    }
    action(
        &mut *world
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}

pub fn generate_loot_in_world(
    world: &super::World,
    table: &LootTable,
    seed: i64,
    params: &LootContextParameters,
) -> Vec<ItemStack> {
    with_loot_random(world, table, seed, |rng| {
        generate_loot_with_rng(table, params, rng)
    })
}

#[derive(Default, Clone)]
pub struct LootContextParameters {
    pub explosion_radius: Option<f32>,
    pub block_state: Option<&'static BlockState>,
    pub killed_by_player: Option<bool>,
    pub luck: f32,
    pub this_entity: Option<&'static EntityType>,
    pub killer_entity: Option<&'static EntityType>,
    pub direct_killer_entity: Option<&'static EntityType>,
    pub position: Option<pumpkin_util::math::vector3::Vector3<f64>>,
    pub world_time: u64,
    pub damage_type: Option<DamageType>,
    pub tool: Option<ItemStack>,
    pub is_raining: Option<bool>,
    pub is_thundering: Option<bool>,
    /// Whether the killed entity was on fire at death time.
    /// Computed from `Entity.fire_ticks > 0`.
    pub is_on_fire: Option<bool>,
    /// Whether the killed entity was a baby. Vanilla `entity_properties` reads this
    /// through `minecraft:flags { is_baby }`.
    pub this_is_baby: Option<bool>,
    /// What the killed entity was riding, if anything -- the other half of the
    /// chicken-jockey check.
    pub this_vehicle: Option<&'static EntityType>,
    /// Slime and magma cube size, for `type_specific/cube_mob { size }`.
    pub this_cube_size: Option<i32>,
    /// Whether the killed entity was a raid captain, for
    /// `type_specific/raider { is_captain }`.
    pub this_is_raid_captain: Option<bool>,
}

/// Matches an entity type against a loot predicate value, which is either a registry
/// name or a `#tag`.
fn entity_type_matches(actual: &'static EntityType, expected: &str) -> bool {
    use pumpkin_data::tag::Taggable;
    if let Some(tag) = expected.strip_prefix('#') {
        return actual.is_tagged_with(tag).unwrap_or(false);
    }
    let expected = expected.strip_prefix("minecraft:").unwrap_or(expected);
    actual.resource_name == expected
}

/// Matches a damage type against a `#tag` named in a `damage_source_properties` predicate.
fn damage_type_has_tag(damage: &DamageType, tag: &str) -> bool {
    use pumpkin_data::tag::Taggable;
    let tag = if tag.starts_with('#') {
        tag.to_string()
    } else {
        format!("#{tag}")
    };
    damage.is_tagged_with(&tag).unwrap_or(false)
}

fn check_condition(
    cond: LootCondition,
    has_silk_touch: bool,
    has_shears: bool,
    fortune_level: i32,
    params: &LootContextParameters,
    rng: &mut dyn LootRandom,
) -> bool {
    match cond {
        LootCondition::None => true,
        LootCondition::SilkTouch => has_silk_touch,
        LootCondition::NoSilkTouch => !has_silk_touch,
        LootCondition::Shears => has_shears,
        LootCondition::SilkTouchOrShears => has_silk_touch || has_shears,
        LootCondition::NoSilkTouchOrShears => !has_silk_touch && !has_shears,
        LootCondition::KilledByPlayer => params.killed_by_player.unwrap_or(false),
        LootCondition::SurvivesExplosion => params
            .explosion_radius
            .is_none_or(|radius| rng.next_f32() <= 1.0 / radius),
        LootCondition::RandomChance { chance } => rng.next_f32() < chance,
        LootCondition::RandomChanceWithEnchantedBonus {
            unenchanted_chance,
            enchanted_chance_base,
            enchanted_chance_per_level_above_first,
        } => {
            let chance = if fortune_level > 0 {
                enchanted_chance_base
                    + enchanted_chance_per_level_above_first * (fortune_level - 1) as f32
            } else {
                unenchanted_chance
            };
            rng.next_f32() < chance
        }
        LootCondition::TableBonus { chances } => {
            let index = (fortune_level.max(0) as usize).min(chances.len().saturating_sub(1));
            chances
                .get(index)
                .is_some_and(|chance| rng.next_f32() < *chance)
        }
        LootCondition::AllOf(conditions) => conditions
            .iter()
            .all(|c| check_condition(*c, has_silk_touch, has_shears, fortune_level, params, rng)),

        // An absent fact fails the check rather than passing it: the pools these gate are
        // rare drops, so guessing "true" is what produced guaranteed wrong drops.
        LootCondition::ThisIsBaby(expected) => params.this_is_baby == Some(expected),
        LootCondition::ThisVehicleIs(name) => params
            .this_vehicle
            .is_some_and(|vehicle| entity_type_matches(vehicle, name)),
        LootCondition::EntityTypeMatches {
            target,
            entity_type,
        } => {
            let actual = match target {
                pumpkin_util::loot_table::EntityTarget::This => params.this_entity,
                pumpkin_util::loot_table::EntityTarget::Killer => params.killer_entity,
                pumpkin_util::loot_table::EntityTarget::DirectKiller => params.direct_killer_entity,
            };
            actual.is_some_and(|actual| entity_type_matches(actual, entity_type))
        }
        LootCondition::ThisCubeSizeIs(size) => params.this_cube_size == Some(size),
        LootCondition::ThisIsRaidCaptain(expected) => params.this_is_raid_captain == Some(expected),
        LootCondition::DamageTypeHasTag { tag, expected } => params
            .damage_type
            .as_ref()
            .is_some_and(|damage| damage_type_has_tag(damage, tag) == expected),

        // The broken block must carry every named property value. `block_state` is the
        // state at break time, which is what vanilla's predicate reads.
        LootCondition::BlockStateProperties { block, properties } => {
            params.block_state.is_some_and(|state| {
                let actual_block = pumpkin_data::Block::from_state_id(state.id);
                let expected = block.strip_prefix("minecraft:").unwrap_or(block);
                if actual_block.name != expected {
                    return false;
                }
                actual_block.properties(state.id).is_some_and(|props| {
                    let actual = props.to_props();
                    properties
                        .iter()
                        .all(|(key, value)| actual.iter().any(|(k, v)| k == key && v == value))
                })
            })
        }

        LootCondition::Unsupported => false,
    }
}

fn apply_bonus_formula(
    base_count: i32,
    bonus: LootBonusFormula,
    fortune_level: i32,
    rng: &mut dyn LootRandom,
) -> i32 {
    match bonus {
        LootBonusFormula::OreDrops => {
            if fortune_level > 0 {
                let bonus = (rng.next_bounded_i32(fortune_level + 2) - 1).max(0);
                base_count * (bonus + 1)
            } else {
                base_count
            }
        }
        LootBonusFormula::UniformBonusCount(bonus_multiplier) => {
            let max_bonus = fortune_level * bonus_multiplier;
            let extra = rng.next_bounded_i32(max_bonus + 1);
            base_count + extra
        }
        LootBonusFormula::BinomialWithBonusCount { extra, probability } => {
            let n = fortune_level + extra;
            let mut bonus_count = 0;
            for _ in 0..n {
                if rng.next_f32() < probability {
                    bonus_count += 1;
                }
            }
            base_count + bonus_count
        }
    }
}

#[must_use]
pub fn generate_loot(table: &LootTable, seed: i64) -> Vec<ItemStack> {
    generate_loot_with_context(table, seed, &LootContextParameters::default())
}

#[must_use]
pub fn generate_loot_with_context(
    table: &LootTable,
    seed: i64,
    params: &LootContextParameters,
) -> Vec<ItemStack> {
    generate_loot_with_rng(table, params, &mut LegacyRand::from_seed(seed as u64))
}

fn generate_loot_with_rng(
    table: &LootTable,
    params: &LootContextParameters,
    rng: &mut dyn LootRandom,
) -> Vec<ItemStack> {
    let mut items_to_place: Vec<ItemStack> = Vec::new();

    let has_silk_touch = params.tool.as_ref().is_some_and(|tool| {
        pumpkin_data::Enchantment::from_name("silk_touch")
            .is_some_and(|e| tool.get_enchantment_level(e) > 0)
    });

    let has_shears = params.tool.as_ref().is_some_and(|tool| {
        let name = tool
            .item
            .registry_key
            .strip_prefix("minecraft:")
            .unwrap_or(tool.item.registry_key);
        name == "shears"
    });

    let fortune_level = params.tool.as_ref().map_or(0, |tool| {
        let fortune = pumpkin_data::Enchantment::from_name("fortune")
            .map_or(0, |e| tool.get_enchantment_level(e));
        let looting = pumpkin_data::Enchantment::from_name("looting")
            .map_or(0, |e| tool.get_enchantment_level(e));
        fortune.max(looting)
    });

    for pool in table.pools {
        if !check_condition(
            pool.condition,
            has_silk_touch,
            has_shears,
            fortune_level,
            params,
            rng,
        ) {
            continue;
        }

        let range = pool.max_rolls - pool.min_rolls;
        let rolls = pool.min_rolls
            + if range > 0 {
                rng.next_bounded_i32(range + 1)
            } else {
                0
            };

        for _ in 0..rolls {
            let eligible_entries: Vec<&LootEntry> = pool
                .entries
                .iter()
                .filter(|e| {
                    check_condition(
                        e.condition,
                        has_silk_touch,
                        has_shears,
                        fortune_level,
                        params,
                        rng,
                    ) && e.weight > 0
                })
                .collect();

            if eligible_entries.is_empty() {
                continue;
            }

            let total_weight: i32 = eligible_entries.iter().map(|e| e.weight).sum();
            if total_weight == 0 {
                continue;
            }

            // Java skips the weighted draw when there is only one expanded entry.
            let mut pick = if eligible_entries.len() == 1 {
                0
            } else {
                rng.next_bounded_i32(total_weight)
            };

            for entry in &eligible_entries {
                pick -= entry.weight;
                if pick < 0 {
                    if entry.item.is_empty() {
                        break;
                    }
                    let count_range = entry.max_count - entry.min_count;
                    let base_count = entry.min_count
                        + if count_range > 0 {
                            rng.next_bounded_i32(count_range + 1)
                        } else {
                            0
                        };

                    let mut final_count = base_count;
                    if let Some(bonus) = entry.bonus_formula {
                        final_count = apply_bonus_formula(final_count, bonus, fortune_level, rng);
                    }

                    if final_count > 0 {
                        let item_key = entry.item.strip_prefix("minecraft:").unwrap_or(entry.item);

                        if let Some(item) = Item::from_registry_key(item_key) {
                            items_to_place.push(ItemStack::new(final_count as u8, item));
                        }
                    }
                    break;
                }
            }
        }
    }

    items_to_place
}

pub use generate_loot as generate_chest_loot;

pub fn fill_chest_inventory(
    inventory: &std::sync::Arc<dyn pumpkin_inventory::Inventory>,
    table: &LootTable,
    seed: i64,
) {
    fill_inventory_with_context(
        inventory.as_ref(),
        table,
        seed,
        &LootContextParameters::default(),
    );
}

pub fn fill_inventory_with_context(
    inventory: &dyn pumpkin_inventory::Inventory,
    table: &LootTable,
    seed: i64,
    context: &LootContextParameters,
) {
    let placements = container_placements(
        inventory,
        table,
        context,
        &mut LegacyRand::from_seed(seed as u64),
    );
    for (slot, item) in placements {
        inventory.set_stack(slot, item);
    }
}

pub fn fill_inventory_in_world(
    world: &super::World,
    inventory: &dyn pumpkin_inventory::Inventory,
    table: &LootTable,
    seed: i64,
    context: &LootContextParameters,
) {
    let placements = with_loot_random(world, table, seed, |rng| {
        container_placements(inventory, table, context, rng)
    });
    // Inventory notifications can run block logic, so release the random-source lock first.
    for (slot, item) in placements {
        inventory.set_stack(slot, item);
    }
}

fn container_placements(
    inventory: &dyn pumpkin_inventory::Inventory,
    table: &LootTable,
    context: &LootContextParameters,
    rng: &mut dyn LootRandom,
) -> Vec<(usize, ItemStack)> {
    let mut items_to_place = generate_loot_with_rng(table, context, rng);
    let mut available_slots: Vec<usize> = (0..inventory.size())
        .filter(|&slot| inventory.get_stack(slot).is_empty())
        .collect();
    for i in (1..available_slots.len()).rev() {
        let j = rng.next_bounded_i32((i + 1) as i32) as usize;
        available_slots.swap(i, j);
    }
    shuffle_and_split_items(&mut items_to_place, available_slots.len(), rng);
    let mut placements = Vec::new();
    for item in items_to_place {
        let Some(slot) = available_slots.pop() else {
            tracing::warn!("Tried to over-fill a container");
            break;
        };
        placements.push((slot, item));
    }
    placements
}

fn shuffle_and_split_items(
    result: &mut Vec<ItemStack>,
    available_slots: usize,
    rng: &mut dyn LootRandom,
) {
    let mut splittable: Vec<ItemStack> = Vec::new();
    let mut i = 0;
    while i < result.len() {
        if result[i].is_empty() {
            result.remove(i);
        } else if result[i].item_count > 1 {
            splittable.push(result.remove(i));
        } else {
            i += 1;
        }
    }

    while available_slots > result.len() + splittable.len() && !splittable.is_empty() {
        let idx = if splittable.len() > 1 {
            rng.next_bounded_i32(splittable.len() as i32) as usize
        } else {
            0
        };
        let mut stack = splittable.remove(idx);

        let count = stack.item_count as i32;
        let split_off = if count / 2 > 1 {
            1 + rng.next_bounded_i32(count / 2)
        } else {
            1
        };
        stack.item_count = (count - split_off) as u8;
        let mut copy = stack.clone();
        copy.item_count = split_off as u8;

        if stack.item_count > 1 && rng.next_bool() {
            splittable.push(stack);
        } else {
            result.push(stack);
        }
        if copy.item_count > 1 && rng.next_bool() {
            splittable.push(copy);
        } else {
            result.push(copy);
        }
    }

    result.extend(splittable);

    let n = result.len();
    for i in (1..n).rev() {
        let j = rng.next_bounded_i32((i + 1) as i32) as usize;
        result.swap(i, j);
    }
}

#[cfg(test)]
mod condition_tests {
    use super::*;
    use pumpkin_util::loot_table::EntityTarget;

    fn rng() -> Xoroshiro {
        Xoroshiro::from_seed(42)
    }

    fn check(cond: LootCondition, params: &LootContextParameters) -> bool {
        check_condition(cond, false, false, 0, params, &mut rng())
    }

    /// The bug this guards: an unrepresentable condition used to be discarded, which left
    /// the pool unconditional. A zombie therefore dropped the chicken-jockey music disc on
    /// every player kill. Unsupported must never pass.
    #[test]
    fn unsupported_never_passes() {
        assert!(!check(
            LootCondition::Unsupported,
            &LootContextParameters::default()
        ));
    }

    /// The exact condition from the zombie's music-disc pool: baby *and* riding a chicken.
    #[test]
    fn chicken_jockey_disc_requires_baby_riding_a_chicken() {
        let jockey = LootContextParameters {
            this_is_baby: Some(true),
            this_vehicle: Some(&EntityType::CHICKEN),
            ..Default::default()
        };
        assert!(check(LootCondition::ThisIsBaby(true), &jockey));
        assert!(check(
            LootCondition::ThisVehicleIs("minecraft:chicken"),
            &jockey
        ));

        // An ordinary adult zombie on foot satisfies neither.
        let plain = LootContextParameters {
            this_is_baby: Some(false),
            this_vehicle: None,
            ..Default::default()
        };
        assert!(!check(LootCondition::ThisIsBaby(true), &plain));
        assert!(!check(
            LootCondition::ThisVehicleIs("minecraft:chicken"),
            &plain
        ));

        // A baby on a different mount is still not a chicken jockey.
        let on_a_pig = LootContextParameters {
            this_is_baby: Some(true),
            this_vehicle: Some(&EntityType::PIG),
            ..Default::default()
        };
        assert!(!check(
            LootCondition::ThisVehicleIs("minecraft:chicken"),
            &on_a_pig
        ));
    }

    /// An unknown fact fails the condition rather than passing it -- guessing "true" is
    /// what produced the guaranteed wrong drop in the first place.
    #[test]
    fn absent_facts_fail_closed() {
        let empty = LootContextParameters::default();
        assert!(!check(LootCondition::ThisIsBaby(true), &empty));
        assert!(!check(LootCondition::ThisCubeSizeIs(1), &empty));
        assert!(!check(LootCondition::ThisIsRaidCaptain(true), &empty));
        assert!(!check(
            LootCondition::ThisVehicleIs("minecraft:chicken"),
            &empty
        ));
    }

    /// Slime balls are gated on cube size 1; larger slimes must not drop them directly.
    #[test]
    fn slime_ball_only_from_size_one() {
        for (size, expected) in [(1, true), (2, false), (4, false)] {
            let params = LootContextParameters {
                this_cube_size: Some(size),
                ..Default::default()
            };
            assert_eq!(
                check(LootCondition::ThisCubeSizeIs(1), &params),
                expected,
                "size {size}"
            );
        }
    }

    #[test]
    fn entity_type_matches_by_name_and_target() {
        let params = LootContextParameters {
            this_entity: Some(&EntityType::ZOMBIE),
            killer_entity: Some(&EntityType::SKELETON),
            ..Default::default()
        };
        assert!(check(
            LootCondition::EntityTypeMatches {
                target: EntityTarget::Killer,
                entity_type: "minecraft:skeleton",
            },
            &params
        ));
        // Right name, wrong subject.
        assert!(!check(
            LootCondition::EntityTypeMatches {
                target: EntityTarget::This,
                entity_type: "minecraft:skeleton",
            },
            &params
        ));
    }
}

#[cfg(test)]
mod random_tests {
    use super::*;
    use pumpkin_inventory::{Inventory, SimpleInventory};
    use pumpkin_util::loot_table::LootPool;
    use serde_json::{Value, json};

    fn table(mode: i64) -> LootTable {
        let mut entries = vec![LootEntry {
            item: "minecraft:diamond",
            weight: 3,
            min_count: 1,
            max_count: 9,
            condition: LootCondition::RandomChance {
                chance: if mode == 0 { 1.0 } else { 0.65 },
            },
            bonus_formula: None,
        }];
        if mode > 0 {
            entries.push(LootEntry {
                item: "minecraft:coal",
                weight: 1,
                min_count: 2,
                max_count: 2,
                condition: LootCondition::None,
                bonus_formula: None,
            });
        }
        if mode == 1 {
            entries.push(LootEntry {
                item: "minecraft:stick",
                weight: 0,
                min_count: 1,
                max_count: 1,
                condition: LootCondition::RandomChance { chance: 0.2 },
                bonus_formula: None,
            });
        }
        if mode == 2 {
            entries.push(LootEntry {
                item: "",
                weight: 2,
                min_count: 0,
                max_count: 0,
                condition: LootCondition::RandomChance { chance: 0.5 },
                bonus_formula: None,
            });
        }
        LootTable {
            random_sequence: None,
            pools: Box::leak(
                vec![LootPool {
                    entries: Box::leak(entries.into_boxed_slice()),
                    min_rolls: if mode == 0 { 1 } else { 2 },
                    max_rolls: if mode == 0 { 1 } else { 5 },
                    condition: if mode == 3 {
                        LootCondition::RandomChance { chance: 0.25 }
                    } else {
                        LootCondition::None
                    },
                }]
                .into_boxed_slice(),
            ),
        }
    }
    fn stack_value(stack: &ItemStack) -> Value {
        let key = stack.item.registry_key;
        json!([
            if key.contains(':') {
                key.to_owned()
            } else {
                format!("minecraft:{key}")
            },
            stack.item_count
        ])
    }
    fn compare<R: pumpkin_util::random::RandomImpl>(
        case: &Value,
        table: &LootTable,
        mut source: R,
        mut fill_source: R,
    ) {
        let context = LootContextParameters::default();
        let generated = generate_loot_with_rng(table, &context, &mut source);
        assert_eq!(
            json!(generated.iter().map(stack_value).collect::<Vec<_>>()),
            case["generated"],
            "{case}"
        );
        let inventory = SimpleInventory::new(case["size"].as_u64().unwrap() as usize);
        if case["occupied"].as_bool().unwrap() {
            inventory.set_stack(1, ItemStack::new(1, &Item::STONE));
        }
        for (slot, stack) in container_placements(&inventory, table, &context, &mut fill_source) {
            inventory.set_stack(slot, stack);
        }
        assert_eq!(
            json!(
                (0..inventory.size())
                    .map(|i| stack_value(&inventory.get_stack(i)))
                    .collect::<Vec<_>>()
            ),
            case["filled"],
            "{case}"
        );
        assert_eq!(
            fill_source.next_i64(),
            case["next"].as_i64().unwrap(),
            "random consumption: {case}"
        );
    }
    #[test]
    fn matches_java_raw_loot_and_continuous_container_random_stream() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("loot_random_cases.json")).unwrap();
        assert_eq!(cases.len(), 256);
        let tables = [table(0), table(1), table(2), table(3)];
        for case in cases {
            let table = &tables[case["mode"].as_u64().unwrap() as usize];
            let seed = case["seed"].as_i64().unwrap() as u64;
            if case["kind"] == 0 {
                compare(
                    &case,
                    table,
                    LegacyRand::from_seed(seed),
                    LegacyRand::from_seed(seed),
                );
            } else {
                compare(
                    &case,
                    table,
                    Xoroshiro::from_seed(seed),
                    Xoroshiro::from_seed(seed),
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn world_source_advances_and_explicit_seed_leaves_it_untouched() {
        use arc_swap::ArcSwap;
        use pumpkin_data::dimension::Dimension;
        use pumpkin_util::world_seed::Seed;
        use pumpkin_world::{level::Level, world_info::LevelData};
        use std::sync::{Arc, Weak};
        let folder = tempfile::tempdir().unwrap();
        let level = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            folder.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        let world = super::super::World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
            Dimension::OVERWORLD,
            Arc::new(crate::block::registry::BlockRegistry::default()),
            Weak::new(),
        );
        *world.random.lock().unwrap() = LegacyRand::from_seed(262);
        let table = table(2);
        let context = LootContextParameters::default();
        let mut expected = LegacyRand::from_seed(262);
        for _ in 0..3 {
            let actual = generate_loot_in_world(&world, &table, 0, &context);
            let expected = generate_loot_with_rng(&table, &context, &mut expected);
            assert_eq!(
                actual.iter().map(stack_value).collect::<Vec<_>>(),
                expected.iter().map(stack_value).collect::<Vec<_>>()
            );
        }
        let actual = generate_loot_in_world(&world, &table, 900, &context);
        let seeded = generate_loot_with_context(&table, 900, &context);
        assert_eq!(
            actual.iter().map(stack_value).collect::<Vec<_>>(),
            seeded.iter().map(stack_value).collect::<Vec<_>>()
        );
        assert_eq!(
            pumpkin_util::random::RandomImpl::next_i64(&mut *world.random.lock().unwrap()),
            pumpkin_util::random::RandomImpl::next_i64(&mut expected)
        );
    }
}
