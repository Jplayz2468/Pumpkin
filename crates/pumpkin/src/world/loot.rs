use pumpkin_data::BlockState;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::loot_table::{LootBonusFormula, LootCondition, LootEntry, LootTable};
use pumpkin_util::random::{RandomImpl, xoroshiro128::Xoroshiro};

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
    rng: &mut Xoroshiro,
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
                pumpkin_util::loot_table::EntityTarget::DirectKiller => {
                    params.direct_killer_entity
                }
            };
            actual.is_some_and(|actual| entity_type_matches(actual, entity_type))
        }
        LootCondition::ThisCubeSizeIs(size) => params.this_cube_size == Some(size),
        LootCondition::ThisIsRaidCaptain(expected) => {
            params.this_is_raid_captain == Some(expected)
        }
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
                    properties.iter().all(|(key, value)| {
                        actual
                            .iter()
                            .any(|(k, v)| k == key && v == value)
                    })
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
    rng: &mut Xoroshiro,
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
            let extra = if max_bonus > 0 {
                rng.next_bounded_i32(max_bonus + 1)
            } else {
                0
            };
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
    let mut rng = Xoroshiro::from_seed(seed as u64);
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
            &mut rng,
        ) {
            continue;
        }

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
                    &mut rng,
                )
            })
            .collect();

        if eligible_entries.is_empty() && pool.empty_weight == 0 {
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
            let entry_weight: i32 = eligible_entries.iter().map(|e| e.weight).sum();
            let total_weight = entry_weight + pool.empty_weight;
            if total_weight == 0 {
                continue;
            }

            let mut pick = rng.next_bounded_i32(total_weight);

            pick -= pool.empty_weight;
            if pick < 0 {
                continue;
            }

            for entry in &eligible_entries {
                pick -= entry.weight;
                if pick < 0 {
                    let count_range = entry.max_count - entry.min_count;
                    let base_count = entry.min_count
                        + if count_range > 0 {
                            rng.next_bounded_i32(count_range + 1)
                        } else {
                            0
                        };

                    let mut final_count = base_count;
                    if let Some(bonus) = entry.bonus_formula {
                        final_count =
                            apply_bonus_formula(final_count, bonus, fortune_level, &mut rng);
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
    let mut items_to_place = generate_loot_with_context(table, seed, context);

    if items_to_place.is_empty() {
        return;
    }

    let inv_size = inventory.size();
    let mut rng = Xoroshiro::from_seed(seed as u64);

    let mut available_slots: Vec<usize> = (0..inv_size)
        .filter(|&slot| inventory.get_stack(slot).is_empty())
        .collect();

    for i in (1..available_slots.len()).rev() {
        let j = rng.next_bounded_i32((i + 1) as i32) as usize;
        available_slots.swap(i, j);
    }

    shuffle_and_split_items(&mut items_to_place, available_slots.len(), &mut rng);

    for item in items_to_place {
        let Some(slot) = available_slots.pop() else {
            tracing::warn!("Tried to over-fill a container");
            return;
        };
        inventory.set_stack(slot, item);
    }
}

fn shuffle_and_split_items(
    result: &mut Vec<ItemStack>,
    available_slots: usize,
    rng: &mut Xoroshiro,
) {
    let mut splittable: Vec<ItemStack> = Vec::new();
    let mut i = 0;
    while i < result.len() {
        if result[i].is_empty() {
            result.swap_remove(i);
        } else if result[i].item_count > 1 {
            splittable.push(result.swap_remove(i));
        } else {
            i += 1;
        }
    }

    while available_slots > result.len() + splittable.len() && !splittable.is_empty() {
        let idx = rng.next_bounded_i32(splittable.len() as i32) as usize;
        let mut stack = splittable.swap_remove(idx);

        let count = stack.item_count as i32;
        let split_off = 1 + rng.next_bounded_i32(count / 2);
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
        assert!(!check(LootCondition::Unsupported, &LootContextParameters::default()));
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
        assert!(check(LootCondition::ThisVehicleIs("minecraft:chicken"), &jockey));

        // An ordinary adult zombie on foot satisfies neither.
        let plain = LootContextParameters {
            this_is_baby: Some(false),
            this_vehicle: None,
            ..Default::default()
        };
        assert!(!check(LootCondition::ThisIsBaby(true), &plain));
        assert!(!check(LootCondition::ThisVehicleIs("minecraft:chicken"), &plain));

        // A baby on a different mount is still not a chicken jockey.
        let on_a_pig = LootContextParameters {
            this_is_baby: Some(true),
            this_vehicle: Some(&EntityType::PIG),
            ..Default::default()
        };
        assert!(!check(LootCondition::ThisVehicleIs("minecraft:chicken"), &on_a_pig));
    }

    /// An unknown fact fails the condition rather than passing it -- guessing "true" is
    /// what produced the guaranteed wrong drop in the first place.
    #[test]
    fn absent_facts_fail_closed() {
        let empty = LootContextParameters::default();
        assert!(!check(LootCondition::ThisIsBaby(true), &empty));
        assert!(!check(LootCondition::ThisCubeSizeIs(1), &empty));
        assert!(!check(LootCondition::ThisIsRaidCaptain(true), &empty));
        assert!(!check(LootCondition::ThisVehicleIs("minecraft:chicken"), &empty));
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
