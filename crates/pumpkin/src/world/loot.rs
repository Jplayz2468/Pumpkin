use pumpkin_data::BlockState;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::loot_table::{
    LootBonusFormula, LootCondition, LootEntry, LootEntryKind, LootFunction, LootFunctionKind,
    LootNumberProvider, LootTable,
};
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
    pub dynamic_drops: std::collections::HashMap<String, Vec<ItemStack>>,
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

fn condition_known(condition: LootCondition) -> bool {
    match condition {
        LootCondition::Unsupported => false,
        LootCondition::AllOf(terms) | LootCondition::AnyOf(terms) => {
            terms.iter().all(|term| condition_known(*term))
        }
        LootCondition::Not(term) => condition_known(*term),
        _ => true,
    }
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
        LootCondition::AnyOf(conditions) => conditions
            .iter()
            .any(|c| check_condition(*c, has_silk_touch, has_shears, fortune_level, params, rng)),
        LootCondition::Not(condition) => {
            condition_known(*condition)
                && !check_condition(
                    *condition,
                    has_silk_touch,
                    has_shears,
                    fortune_level,
                    params,
                    rng,
                )
        }
        LootCondition::ToolItems(items) => params.tool.as_ref().is_some_and(|tool| {
            use pumpkin_data::tag::Taggable;
            items.iter().any(|name| {
                if name.starts_with('#') {
                    tool.item.is_tagged_with(name).unwrap_or(false)
                } else {
                    tool.item
                        .registry_key
                        .strip_prefix("minecraft:")
                        .unwrap_or(tool.item.registry_key)
                        == name.strip_prefix("minecraft:").unwrap_or(name)
                }
            })
        }),
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
                base_count.wrapping_mul(bonus.wrapping_add(1))
            } else {
                base_count
            }
        }
        LootBonusFormula::UniformBonusCount(bonus_multiplier) => {
            let max_bonus = fortune_level.wrapping_mul(bonus_multiplier);
            let extra = rng.next_bounded_i32(max_bonus + 1);
            base_count.wrapping_add(extra)
        }
        LootBonusFormula::BinomialWithBonusCount { extra, probability } => {
            let n = fortune_level + extra;
            let mut bonus_count = 0;
            for _ in 0..n {
                if rng.next_f32() < probability {
                    bonus_count += 1;
                }
            }
            base_count.wrapping_add(bonus_count)
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

fn java_floor(value: f32) -> i32 {
    let integer = value as i32;
    if value < integer as f32 {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}

fn java_round(value: f32) -> i32 {
    // Use a double for the addition so values just below 0.5 do not round up
    // during float addition before Java Math.round's rounding step.
    (f64::from(value) + 0.5).floor() as i32
}

fn number_int(provider: LootNumberProvider, rng: &mut dyn LootRandom) -> i32 {
    match provider {
        LootNumberProvider::Constant(value) => java_round(value),
        LootNumberProvider::Uniform(min, max) => {
            let min = number_int(*min, rng);
            let max = number_int(*max, rng);
            if min >= max {
                min
            } else {
                rng.next_bounded_i32(max.wrapping_sub(min).wrapping_add(1))
                    .wrapping_add(min)
            }
        }
        LootNumberProvider::Binomial(n, p) => {
            let n = number_int(*n, rng);
            let p = number_float(*p, rng);
            (0..n).filter(|_| rng.next_f32() < p).count() as i32
        }
    }
}

fn number_float(provider: LootNumberProvider, rng: &mut dyn LootRandom) -> f32 {
    match provider {
        LootNumberProvider::Constant(value) => value,
        LootNumberProvider::Uniform(min, max) => {
            let min = number_float(*min, rng);
            let max = number_float(*max, rng);
            if min >= max {
                min
            } else {
                rng.next_f32() * (max - min) + min
            }
        }
        LootNumberProvider::Binomial(_, _) => number_int(provider, rng) as f32,
    }
}

fn tool_enchantment(params: &LootContextParameters, name: &str) -> i32 {
    params.tool.as_ref().map_or(0, |tool| {
        pumpkin_data::Enchantment::from_name(name.strip_prefix("minecraft:").unwrap_or(name))
            .map_or(0, |enchantment| tool.get_enchantment_level(enchantment))
    })
}

#[derive(Clone, Copy)]
struct LootFacts {
    silk: bool,
    shears: bool,
    fortune: i32,
}
impl LootFacts {
    fn from_params(params: &LootContextParameters) -> Self {
        Self {
            silk: tool_enchantment(params, "silk_touch") > 0,
            shears: params
                .tool
                .as_ref()
                .is_some_and(|tool| tool.item == &Item::SHEARS),
            fortune: tool_enchantment(params, "fortune").max(tool_enchantment(params, "looting")),
        }
    }
    fn check(
        self,
        condition: LootCondition,
        params: &LootContextParameters,
        rng: &mut dyn LootRandom,
    ) -> bool {
        check_condition(condition, self.silk, self.shears, self.fortune, params, rng)
    }
}

// Keep counts as Java ints until all nested functions have run. Narrowing before
// enclosing functions or splitting used to wrap counts above 255.
struct LootOutput {
    stack: ItemStack,
    count: i32,
}
impl LootOutput {
    fn visible_count(&self) -> i32 {
        if self.stack.item == &Item::AIR {
            0
        } else {
            self.count.max(0)
        }
    }
}
type LootConsumer<'a> = dyn FnMut(LootOutput, &mut dyn LootRandom) + 'a;

fn apply_functions(
    mut output: LootOutput,
    functions: &[LootFunction],
    params: &LootContextParameters,
    facts: LootFacts,
    rng: &mut dyn LootRandom,
) -> LootOutput {
    for function in functions {
        if !facts.check(function.condition, params, rng) {
            continue;
        }
        let current = output.visible_count();
        match function.kind {
            LootFunctionKind::SetCount { count, add } => {
                output.count = number_int(count, rng).wrapping_add(if add { current } else { 0 });
            }
            LootFunctionKind::LimitCount { min, max } => {
                output.count = current;
                let min = min.map(|min| number_int(min, rng));
                let max = max.map(|max| number_int(max, rng));
                if let Some(min) = min {
                    output.count = output.count.max(min);
                }
                if let Some(max) = max {
                    output.count = output.count.min(max);
                }
            }
            LootFunctionKind::ApplyBonus {
                enchantment,
                formula,
            } => {
                if params.tool.is_some() {
                    output.count = apply_bonus_formula(
                        current,
                        formula,
                        tool_enchantment(params, enchantment),
                        rng,
                    );
                }
            }
            LootFunctionKind::EnchantedCountIncrease {
                enchantment,
                count,
                limit,
            } => {
                let level = if params.killer_entity.is_some() {
                    tool_enchantment(params, enchantment)
                } else {
                    0
                };
                if level > 0 {
                    let addition = level as f32 * number_float(count, rng);
                    output.count = current.wrapping_add(java_round(addition));
                    if limit > 0 {
                        output.count = output.count.min(limit);
                    }
                }
            }
            LootFunctionKind::ExplosionDecay => {
                if let Some(radius) = params.explosion_radius {
                    output.count = (0..current)
                        .filter(|_| rng.next_f32() <= 1.0 / radius)
                        .count() as i32;
                }
            }
            LootFunctionKind::SetDamage { damage, add } => {
                use pumpkin_data::data_component_impl::DamageImpl;
                if current > 0
                    && !output.stack.is_unbreakable()
                    && output.stack.get_data_component::<DamageImpl>().is_some()
                    && let Some(max) = output.stack.get_max_damage()
                {
                    let initial = output.stack.get_damage().clamp(0, max);
                    let base = if add {
                        1.0 - initial as f32 / max as f32
                    } else {
                        0.0
                    };
                    let remaining = (number_float(damage, rng) + base).clamp(0.0, 1.0);
                    let value = java_floor((1.0 - remaining) * max as f32).clamp(0, max);
                    // An explicit zero must replace an existing/default damage component.
                    output
                        .stack
                        .set_data_component(DamageImpl { damage: value });
                }
            }
            LootFunctionKind::SetPotion(name) => {
                use pumpkin_data::data_component_impl::PotionContentsImpl;
                if let Some(potion) = pumpkin_data::potion::Potion::from_name(
                    name.strip_prefix("minecraft:").unwrap_or(name),
                ) {
                    let mut contents = (current > 0)
                        .then(|| {
                            output
                                .stack
                                .get_data_component::<PotionContentsImpl>()
                                .cloned()
                        })
                        .flatten()
                        .unwrap_or(PotionContentsImpl {
                            potion_id: None,
                            custom_color: None,
                            custom_effects: Vec::new(),
                            custom_name: None,
                        });
                    contents.potion_id = Some(i32::from(potion.id));
                    output.stack.set_data_component(contents);
                }
            }
            LootFunctionKind::CopyState { properties } => {
                use pumpkin_data::data_component_impl::BlockStateImpl;
                if let Some(state) = params.block_state {
                    let block = pumpkin_data::Block::from_state_id(state.id);
                    let values = block
                        .properties(state.id)
                        .map(|properties| properties.to_props())
                        .unwrap_or_default();
                    let mut stored = (current > 0)
                        .then(|| {
                            output
                                .stack
                                .get_data_component::<BlockStateImpl>()
                                .map(|state| state.properties.to_vec())
                        })
                        .flatten()
                        .unwrap_or_default();
                    for (name, identity) in properties {
                        if pumpkin_data::loot_table::block_property_identity(block.name, name)
                            != Some(*identity)
                        {
                            continue;
                        }
                        if let Some((_, value)) = values.iter().find(|(key, _)| key == name) {
                            if let Some((_, old)) = stored.iter_mut().find(|(key, _)| key == name) {
                                *old = (*value).into();
                            } else {
                                stored.push(((*name).into(), (*value).into()));
                            }
                        }
                    }
                    output.stack.set_data_component(BlockStateImpl {
                        properties: stored.into(),
                    });
                }
            }
            LootFunctionKind::Unsupported(_) => {}
        }
    }
    output
}

struct ExpandedEntry<'a> {
    entry: &'a LootEntry,
    item: Option<&'static str>,
    weight: i32,
}

fn expand_entry<'a>(
    entry: &'a LootEntry,
    params: &LootContextParameters,
    facts: LootFacts,
    rng: &mut dyn LootRandom,
    out: &mut Vec<ExpandedEntry<'a>>,
) -> bool {
    if !facts.check(entry.condition, params, rng) {
        return false;
    }
    match entry.kind {
        LootEntryKind::Alternatives(children) => children
            .iter()
            .any(|child| expand_entry(child, params, facts, rng, out)),
        LootEntryKind::Sequence(children) => children
            .iter()
            .all(|child| expand_entry(child, params, facts, rng, out)),
        LootEntryKind::Group(children) => {
            // Java optimizes a one-child group to that child's expansion result.
            if children.len() == 1 {
                return expand_entry(&children[0], params, facts, rng, out);
            }
            for child in children {
                expand_entry(child, params, facts, rng, out);
            }
            true
        }
        LootEntryKind::Unsupported(_) => false,
        kind => {
            let weight =
                java_floor(entry.weight as f32 + entry.quality as f32 * params.luck).max(0);
            if weight > 0 {
                if let LootEntryKind::Tag {
                    items,
                    expand: true,
                } = kind
                {
                    for item in items {
                        out.push(ExpandedEntry {
                            entry,
                            item: Some(item),
                            weight,
                        });
                    }
                } else {
                    out.push(ExpandedEntry {
                        entry,
                        item: None,
                        weight,
                    });
                }
            }
            // A successful zero-weight or empty tag expansion still stops alternatives.
            true
        }
    }
}

fn emit_item(name: &str, rng: &mut dyn LootRandom, output: &mut LootConsumer<'_>) {
    if let Some(item) = Item::from_registry_key(name.strip_prefix("minecraft:").unwrap_or(name)) {
        output(
            LootOutput {
                stack: ItemStack::new(1, item),
                count: 1,
            },
            rng,
        );
    }
}

fn run_entry(
    candidate: &ExpandedEntry<'_>,
    params: &LootContextParameters,
    facts: LootFacts,
    rng: &mut dyn LootRandom,
    visiting: &mut Vec<*const LootTable>,
    output: &mut LootConsumer<'_>,
) {
    if let Some(item) = candidate.item {
        // Expanded Java tag entries emit the item directly, bypassing tag functions.
        emit_item(item, rng, output);
        return;
    }
    let entry = candidate.entry;
    let mut decorated = |item, rng: &mut dyn LootRandom| {
        output(
            apply_functions(item, entry.functions, params, facts, rng),
            rng,
        )
    };
    match entry.kind {
        LootEntryKind::Item(name) => emit_item(name, rng, &mut decorated),
        LootEntryKind::TableReference(key) => {
            if let Some(table) = pumpkin_data::loot_table::get_loot_table(key) {
                run_table(table, params, facts, rng, visiting, &mut decorated);
            }
        }
        LootEntryKind::InlineTable(table) => {
            run_table(table, params, facts, rng, visiting, &mut decorated)
        }
        LootEntryKind::Tag { items, .. } => {
            for item in items {
                emit_item(item, rng, &mut decorated);
            }
        }
        LootEntryKind::Dynamic(name) => {
            if let Some(items) = params.dynamic_drops.get(name) {
                for stack in items {
                    decorated(
                        LootOutput {
                            count: i32::from(stack.item_count),
                            stack: stack.clone(),
                        },
                        rng,
                    );
                }
            }
        }
        _ => {}
    }
}

fn run_table(
    table: &LootTable,
    params: &LootContextParameters,
    facts: LootFacts,
    rng: &mut dyn LootRandom,
    visiting: &mut Vec<*const LootTable>,
    output: &mut LootConsumer<'_>,
) {
    let pointer = std::ptr::from_ref(table);
    if visiting.contains(&pointer) {
        tracing::warn!("Detected infinite loop in loot tables");
        return;
    }
    visiting.push(pointer);
    let mut table_output = |item, rng: &mut dyn LootRandom| {
        output(
            apply_functions(item, table.functions, params, facts, rng),
            rng,
        )
    };
    for pool in table.pools {
        if !facts.check(pool.condition, params, rng) {
            continue;
        }
        let rolls = number_int(pool.rolls, rng).wrapping_add(java_floor(
            number_float(pool.bonus_rolls, rng) * params.luck,
        ));
        let mut pool_output = |item, rng: &mut dyn LootRandom| {
            table_output(
                apply_functions(item, pool.functions, params, facts, rng),
                rng,
            )
        };
        for _ in 0..rolls {
            let mut candidates = Vec::new();
            for entry in pool.entries {
                expand_entry(entry, params, facts, rng, &mut candidates);
            }
            let total_weight = candidates.iter().fold(0_i32, |total, candidate| {
                total.wrapping_add(candidate.weight)
            });
            if total_weight == 0 || candidates.is_empty() {
                continue;
            }
            let mut pick = if candidates.len() == 1 {
                0
            } else {
                rng.next_bounded_i32(total_weight)
            };
            for candidate in candidates {
                pick = pick.wrapping_sub(candidate.weight);
                if pick < 0 {
                    run_entry(&candidate, params, facts, rng, visiting, &mut pool_output);
                    break;
                }
            }
        }
    }
    visiting.pop();
}

fn generate_loot_with_rng(
    table: &LootTable,
    params: &LootContextParameters,
    rng: &mut dyn LootRandom,
) -> Vec<ItemStack> {
    let mut items = Vec::new();
    run_table(
        table,
        params,
        LootFacts::from_params(params),
        rng,
        &mut Vec::new(),
        &mut |mut output, _| {
            if output.stack.item == &Item::AIR {
                return;
            }
            let max = i32::from(output.stack.get_max_stack_size()).max(1);
            while output.count > 0 {
                let count = output.count.min(max);
                output.stack.item_count = count as u8;
                items.push(output.stack.clone());
                output.count -= count;
            }
        },
    );
    items
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

    fn item_entry(
        name: &'static str,
        weight: i32,
        condition: LootCondition,
        count: LootNumberProvider,
    ) -> LootEntry {
        LootEntry {
            kind: LootEntryKind::Item(name),
            weight,
            quality: 0,
            condition,
            functions: Box::leak(
                vec![LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::SetCount { count, add: false },
                }]
                .into_boxed_slice(),
            ),
        }
    }
    fn table(mode: i64) -> LootTable {
        use LootNumberProvider::{Constant, Uniform};
        let mut entries = vec![item_entry(
            "minecraft:diamond",
            3,
            LootCondition::RandomChance {
                chance: if mode == 0 { 1.0 } else { 0.65 },
            },
            Uniform(&Constant(1.0), &Constant(9.0)),
        )];
        if mode > 0 {
            entries.push(item_entry(
                "minecraft:coal",
                1,
                LootCondition::None,
                Constant(2.0),
            ));
        }
        if mode == 1 {
            entries.push(item_entry(
                "minecraft:stick",
                0,
                LootCondition::RandomChance { chance: 0.2 },
                Constant(1.0),
            ));
        }
        if mode == 2 {
            entries.push(LootEntry {
                kind: LootEntryKind::Empty,
                weight: 2,
                quality: 0,
                condition: LootCondition::RandomChance { chance: 0.5 },
                functions: &[],
            });
        }
        LootTable {
            random_sequence: None,
            functions: &[],
            pools: Box::leak(
                vec![LootPool {
                    entries: Box::leak(entries.into_boxed_slice()),
                    rolls: if mode == 0 {
                        Constant(1.0)
                    } else {
                        Uniform(&Constant(2.0), &Constant(5.0))
                    },
                    bonus_rolls: Constant(0.0),
                    functions: &[],
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

#[cfg(test)]
mod tree_tests {
    use super::*;
    use pumpkin_util::loot_table::{LootNumberProvider::Constant, LootPool};
    use serde_json::{Value, json};
    mod compiled {
        include!("loot_tree_test_tables.rs");
    }

    fn compare<R: pumpkin_util::random::RandomImpl>(case: &Value, mut rng: R) {
        let mut params = LootContextParameters {
            luck: case["luck"].as_f64().unwrap() as f32,
            ..Default::default()
        };
        let radius = case["radius"].as_f64().unwrap() as f32;
        if radius > 0.0 {
            params.explosion_radius = Some(radius);
        }
        if case["dynamic"].as_bool().unwrap() {
            params.dynamic_drops.insert(
                "minecraft:sherds".to_owned(),
                vec![
                    ItemStack::new(2, &Item::DIAMOND),
                    ItemStack::new(3, &Item::COAL),
                ],
            );
        }
        let mut output = Vec::new();
        run_table(
            &compiled::TABLES[case["table"].as_u64().unwrap() as usize],
            &params,
            LootFacts::from_params(&params),
            &mut rng,
            &mut Vec::new(),
            &mut |item, _| {
                let count = item.visible_count();
                let name = if count == 0 {
                    "minecraft:air"
                } else {
                    item.stack.item.registry_key
                };
                output.push(json!([
                    if name.contains(':') {
                        name.to_owned()
                    } else {
                        format!("minecraft:{name}")
                    },
                    count
                ]));
            },
        );
        assert_eq!(json!(output), case["output"], "{case}");
        assert_eq!(
            rng.next_i64(),
            case["next"].as_i64().unwrap(),
            "random consumption: {case}"
        );
    }

    #[test]
    fn generated_tree_matches_java_codec_and_evaluator() {
        let cases: Vec<Value> = serde_json::from_str(include_str!("loot_tree_cases.json")).unwrap();
        assert_eq!(compiled::TABLES.len(), 12);
        assert_eq!(cases.len(), 576);
        for case in cases {
            let seed = case["seed"].as_i64().unwrap() as u64;
            if case["kind"] == 0 {
                compare(&case, LegacyRand::from_seed(seed));
            } else {
                compare(&case, Xoroshiro::from_seed(seed));
            }
        }
    }

    #[test]
    fn final_stack_splitting_does_not_wrap_large_counts() {
        let items = generate_loot(&compiled::TABLES[11], 262);
        assert_eq!(
            items
                .iter()
                .map(|stack| stack.item_count)
                .collect::<Vec<_>>(),
            vec![64, 64, 64, 64, 44]
        );
        assert!(items.iter().all(|stack| stack.item == &Item::DIAMOND));
    }

    const fn entry(kind: LootEntryKind) -> LootEntry {
        LootEntry {
            kind,
            weight: 1,
            quality: 0,
            condition: LootCondition::None,
            functions: &[],
        }
    }
    const fn pool(entries: &'static [LootEntry]) -> LootPool {
        LootPool {
            entries,
            rolls: Constant(1.0),
            bonus_rolls: Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }
    }
    static RECURSIVE: LootTable = LootTable {
        random_sequence: None,
        functions: &[],
        pools: &[
            pool(&[entry(LootEntryKind::InlineTable(&RECURSIVE))]),
            pool(&[entry(LootEntryKind::Item("minecraft:diamond"))]),
        ],
    };
    static REPEAT: LootTable = LootTable {
        random_sequence: None,
        functions: &[],
        pools: &[
            pool(&[entry(LootEntryKind::InlineTable(&RECURSIVE))]),
            pool(&[entry(LootEntryKind::InlineTable(&RECURSIVE))]),
            pool(&[entry(LootEntryKind::TableReference(
                "minecraft:missing_fixture",
            ))]),
        ],
    };
    #[test]
    fn recursion_guard_is_scoped_to_the_active_table_path() {
        let items = generate_loot(&REPEAT, 262);
        assert_eq!(items.len(), 2);
        assert!(
            items
                .iter()
                .all(|stack| stack.item == &Item::DIAMOND && stack.item_count == 1)
        );
    }

    #[test]
    fn builtin_pot_uses_its_dynamic_branch_and_amethyst_requires_the_tool_tag() {
        use pumpkin_data::block_properties::DecoratedPotLikeProperties;
        let table =
            pumpkin_data::loot_table::get_loot_table("minecraft:blocks/decorated_pot").unwrap();
        let mut properties = DecoratedPotLikeProperties::from_state_id(
            pumpkin_data::Block::DECORATED_POT.default_state.id,
        );
        let mut params = LootContextParameters::default();
        params.dynamic_drops.insert(
            "minecraft:sherds".to_owned(),
            vec![
                ItemStack::new(1, &Item::BRICK),
                ItemStack::new(1, &Item::ANGLER_POTTERY_SHERD),
            ],
        );
        for cracked in [false, true] {
            properties.cracked = cracked;
            params.block_state = Some(
                properties
                    .to_state_id(&pumpkin_data::Block::DECORATED_POT)
                    .to_state(),
            );
            let items = generate_loot_with_context(table, 262, &params);
            if cracked {
                assert_eq!(
                    items.iter().map(|stack| stack.item.id).collect::<Vec<_>>(),
                    vec![Item::BRICK.id, Item::ANGLER_POTTERY_SHERD.id]
                );
            } else {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].item, &Item::DECORATED_POT);
            }
        }
        let condition = LootCondition::ToolItems(&["#minecraft:cluster_max_harvestables"]);
        for (item, expected) in [
            (&Item::DIAMOND_PICKAXE, true),
            (&Item::WOODEN_PICKAXE, true),
            (&Item::SHEARS, false),
        ] {
            params.tool = Some(ItemStack::new(1, item));
            assert_eq!(
                LootFacts::from_params(&params).check(
                    condition,
                    &params,
                    &mut LegacyRand::from_seed(0)
                ),
                expected
            );
        }
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::data_component_impl::{
        BlockStateImpl, DamageImpl, MaxDamageImpl, PotionContentsImpl, UnbreakableImpl,
    };
    use pumpkin_data::potion::Potion;
    use serde_json::{Value, json};
    mod compiled {
        include!("loot_component_test_tables.rs");
    }

    fn compare<R: pumpkin_util::random::RandomImpl>(case: &Value, mut rng: R) {
        let name = case["item"]
            .as_str()
            .unwrap()
            .strip_prefix("minecraft:")
            .unwrap();
        let mut stack = ItemStack::new(1, Item::from_registry_key(name).unwrap());
        for component in [
            DataComponent::MaxDamage,
            DataComponent::Damage,
            DataComponent::Unbreakable,
            DataComponent::PotionContents,
            DataComponent::BlockState,
        ] {
            stack.remove_data_component(component);
        }
        if let Some(max) = case["max"].as_i64() {
            stack.set_data_component(MaxDamageImpl {
                max_damage: max as i32,
            });
        }
        if let Some(damage) = case["initial"].as_i64() {
            stack.set_data_component(DamageImpl {
                damage: damage as i32,
            });
        }
        if case["unbreakable"].as_bool().unwrap() {
            stack.set_data_component(UnbreakableImpl);
        }
        if case["metadata"].as_bool().unwrap() {
            stack.set_data_component(PotionContentsImpl {
                potion_id: Some(i32::from(Potion::WATER.id)),
                custom_color: Some(123456),
                custom_name: Some("retained".to_owned()),
                custom_effects: Vec::new(),
            });
            stack.set_data_component(BlockStateImpl {
                properties: vec![
                    ("retained".into(), "yes".into()),
                    ("honey_level".into(), "2".into()),
                    ("age".into(), "4".into()),
                ]
                .into(),
            });
        }
        let mut params = LootContextParameters::default();
        params
            .dynamic_drops
            .insert("minecraft:input".to_owned(), vec![stack]);
        if let Some(context) = case["context"].as_object() {
            let block = pumpkin_data::Block::from_registry_key(
                context["block"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("minecraft:")
                    .unwrap(),
            )
            .unwrap();
            let properties: Vec<_> = context["properties"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str().unwrap()))
                .collect();
            params.block_state = Some(
                if properties.is_empty() {
                    block.default_state.id
                } else {
                    block.from_properties(&properties).to_state_id(block)
                }
                .to_state(),
            );
        }
        let mut output = Vec::new();
        run_table(
            &compiled::TABLES[case["table"].as_u64().unwrap() as usize],
            &params,
            LootFacts::from_params(&params),
            &mut rng,
            &mut Vec::new(),
            &mut |item, _| {
                let potion = item.stack.get_data_component::<PotionContentsImpl>().map(|potion| json!({
                "id": potion.potion_id.and_then(|id| Potion::from_id(id as u8)).map(|p| format!("minecraft:{}",p.name)),
                "color": potion.custom_color, "name": potion.custom_name, "effects": potion.custom_effects.len(),
            }));
                let state = item
                    .stack
                    .get_data_component::<BlockStateImpl>()
                    .map(|state| {
                        state
                            .properties
                            .iter()
                            .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
                            .collect::<serde_json::Map<_, _>>()
                    });
                let name = item.stack.item.registry_key;
                output.push(json!({"item": if name.contains(':') {name.to_owned()} else {format!("minecraft:{name}")}, "count": item.visible_count(), "damage": item.stack.get_data_component::<DamageImpl>().map(|d| d.damage), "potion": potion, "state": state}));
            },
        );
        let mut expected = case["output"].clone();
        for item in expected.as_array_mut().unwrap() {
            for field in ["damage", "potion", "state"] {
                item.as_object_mut()
                    .unwrap()
                    .entry(field)
                    .or_insert(Value::Null);
            }
            if let Some(potion) = item["potion"].as_object_mut() {
                for field in ["id", "color", "name"] {
                    potion.entry(field).or_insert(Value::Null);
                }
            }
        }
        assert_eq!(json!(output), expected, "{case}");
        assert_eq!(
            rng.next_i64(),
            case["next"].as_i64().unwrap(),
            "random consumption: {case}"
        );
    }

    #[test]
    fn component_functions_match_java_through_the_production_generator() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("loot_component_cases.json")).unwrap();
        assert_eq!(compiled::TABLES.len(), 11);
        assert_eq!(cases.len(), 704);
        for case in cases {
            let seed = case["seed"].as_i64().unwrap() as u64;
            if case["kind"] == 0 {
                compare(&case, LegacyRand::from_seed(seed));
            } else {
                compare(&case, Xoroshiro::from_seed(seed));
            }
        }
    }

    #[test]
    fn all_java_property_names_exist_in_the_generated_block_registry() {
        let blocks: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u32>> =
            serde_json::from_str(include_str!("../../../../assets/block_property_ids.json"))
                .unwrap();
        for (name, ids) in &blocks {
            let block =
                pumpkin_data::Block::from_registry_key(name.strip_prefix("minecraft:").unwrap())
                    .unwrap();
            let properties = block
                .properties(block.default_state.id)
                .map(|properties| properties.to_props())
                .unwrap_or_default();
            assert_eq!(properties.len(), ids.len(), "{name}");
            for (property, _) in properties {
                assert_eq!(
                    pumpkin_data::loot_table::block_property_identity(name, property),
                    ids.get(property).copied(),
                    "{name}:{property}"
                );
            }
        }
        assert_ne!(
            pumpkin_data::loot_table::block_property_identity("wheat", "age"),
            pumpkin_data::loot_table::block_property_identity("sugar_cane", "age")
        );
        assert_eq!(
            pumpkin_data::loot_table::block_property_identity("beehive", "honey_level"),
            pumpkin_data::loot_table::block_property_identity("bee_nest", "honey_level")
        );
    }
}
