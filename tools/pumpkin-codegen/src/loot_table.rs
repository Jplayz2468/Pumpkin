use std::{fs, path::Path};

use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use pumpkin_util::loot_table::{EntityTarget, LootBonusFormula, LootCondition};
use quote::{format_ident, quote};
use serde::Deserialize;
use syn::LitStr;

/// `rolls` can be a bare float or an object with `type/min/max`.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum RollsStruct {
    Constant(f32),
    Provider {
        #[allow(dead_code)]
        #[serde(rename = "type")]
        provider_type: String,
        #[allow(dead_code)]
        #[serde(default)]
        min: f32,
        #[serde(default)]
        max: f32,
    },
}

impl RollsStruct {
    fn min(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { min, .. } => min.round() as i32,
        }
    }
    fn max(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { max, .. } => max.round() as i32,
        }
    }
}

/// A `set_count` count provider (uniform or constant).
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum CountStruct {
    Constant(f32),
    Provider {
        #[serde(rename = "type")]
        #[allow(dead_code)]
        provider_type: String,
        #[serde(default)]
        min: f32,
        #[serde(default)]
        max: f32,
    },
}

impl CountStruct {
    fn min(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { min, .. } => min.round() as i32,
        }
    }
    fn max(&self) -> i32 {
        match self {
            Self::Constant(v) => v.round() as i32,
            Self::Provider { max, .. } => max.round() as i32,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct PredicateStruct {
    #[serde(default)]
    items: Option<serde_json::Value>,
    #[serde(default)]
    predicates: Option<serde_json::Value>,
    /// Everything else, kept raw. `entity_properties` and `damage_source_properties`
    /// carry arbitrarily shaped predicates that cannot be modelled field by field.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum EnchantedChanceStruct {
    Constant(f32),
    Linear {
        #[serde(rename = "type")]
        chance_type: String,
        base: f32,
        #[serde(default)]
        per_level_above_first: f32,
    },
}

#[derive(Deserialize, Clone, Debug)]
struct ConditionStruct {
    #[serde(default)]
    condition: String,
    #[allow(dead_code)]
    #[serde(default)]
    enchantment: Option<String>,
    #[serde(default)]
    chance: Option<f32>,
    #[serde(default)]
    unenchanted_chance: Option<f32>,
    #[serde(default)]
    enchanted_chance: Option<EnchantedChanceStruct>,
    #[serde(default)]
    chances: Option<Vec<f32>>,
    #[serde(default)]
    predicate: Option<PredicateStruct>,
    #[serde(default)]
    term: Option<Box<ConditionStruct>>,
    #[serde(default)]
    terms: Option<Vec<ConditionStruct>>,
    /// `entity_properties` names its subject: "this", "killer" or "direct_killer".
    #[serde(default)]
    entity: Option<String>,
    /// `block_state_property` names the block it asserts about.
    #[serde(default)]
    block: Option<String>,
    /// `block_state_property` property values, all serialized as strings by vanilla.
    #[serde(default)]
    properties: Option<serde_json::Map<String, serde_json::Value>>,
}

fn parse_condition(cond: &ConditionStruct) -> LootCondition {
    match cond.condition.as_str() {
        "minecraft:survives_explosion" => LootCondition::SurvivesExplosion,
        "minecraft:killed_by_player" => LootCondition::KilledByPlayer,
        "minecraft:random_chance" => {
            let chance = cond
                .chance
                .or_else(|| cond.chances.as_ref().and_then(|c| c.first().copied()))
                .unwrap_or(0.0);
            LootCondition::RandomChance { chance }
        }
        "minecraft:random_chance_with_enchanted_bonus" => {
            let unenchanted_chance = cond.unenchanted_chance.unwrap_or(0.0);
            let (enchanted_chance_base, enchanted_chance_per_level_above_first) =
                match &cond.enchanted_chance {
                    Some(EnchantedChanceStruct::Linear {
                        base,
                        per_level_above_first,
                        ..
                    }) => (*base, *per_level_above_first),
                    Some(EnchantedChanceStruct::Constant(c)) => (*c, 0.0),
                    None => (unenchanted_chance, 0.0),
                };
            LootCondition::RandomChanceWithEnchantedBonus {
                unenchanted_chance,
                enchanted_chance_base,
                enchanted_chance_per_level_above_first,
            }
        }
        "minecraft:table_bonus" => {
            let chances = cond.chances.clone().unwrap_or_default();
            if chances.is_empty() {
                LootCondition::None
            } else {
                LootCondition::TableBonus {
                    chances: Box::leak(chances.into_boxed_slice()),
                }
            }
        }
        "minecraft:all_of" => {
            if let Some(terms) = &cond.terms {
                combine_conditions(terms)
            } else {
                LootCondition::None
            }
        }
        "minecraft:match_tool" => {
            if let Some(pred) = &cond.predicate {
                if let Some(items_val) = &pred.items {
                    let is_shears = match items_val {
                        serde_json::Value::String(s) => s.contains("shears"),
                        serde_json::Value::Array(arr) => arr
                            .iter()
                            .any(|v| v.as_str().is_some_and(|s| s.contains("shears"))),
                        _ => false,
                    };
                    if is_shears {
                        return LootCondition::Shears;
                    }
                }
                if let Some(pred_val) = &pred.predicates {
                    let s = pred_val.to_string();
                    if s.contains("silk_touch") {
                        return LootCondition::SilkTouch;
                    }
                }
            }
            LootCondition::None
        }
        "minecraft:any_of" => {
            if let Some(terms) = &cond.terms {
                let has_silk = terms
                    .iter()
                    .any(|t| parse_condition(t) == LootCondition::SilkTouch);
                let has_shears = terms
                    .iter()
                    .any(|t| parse_condition(t) == LootCondition::Shears);
                if has_silk && has_shears {
                    return LootCondition::SilkTouchOrShears;
                } else if has_silk {
                    return LootCondition::SilkTouch;
                } else if has_shears {
                    return LootCondition::Shears;
                }
            }
            LootCondition::None
        }
        "minecraft:inverted" => {
            if let Some(term) = &cond.term {
                match parse_condition(term) {
                    LootCondition::SilkTouch => LootCondition::NoSilkTouch,
                    LootCondition::Shears => LootCondition::NoSilkTouchOrShears,
                    LootCondition::SilkTouchOrShears => LootCondition::NoSilkTouchOrShears,
                    _ => LootCondition::None,
                }
            } else {
                LootCondition::None
            }
        }
        "minecraft:block_state_property" => {
            let Some(block) = &cond.block else {
                return LootCondition::Unsupported;
            };
            let mut pairs: Vec<(&'static str, &'static str)> = Vec::new();
            if let Some(properties) = &cond.properties {
                for (key, value) in properties {
                    let Some(value) = value.as_str() else {
                        return LootCondition::Unsupported;
                    };
                    pairs.push((
                        Box::leak(key.clone().into_boxed_str()) as &'static str,
                        Box::leak(value.to_string().into_boxed_str()) as &'static str,
                    ));
                }
            }
            LootCondition::BlockStateProperties {
                block: Box::leak(block.clone().into_boxed_str()),
                properties: Box::leak(pairs.into_boxed_slice()),
            }
        }
        "minecraft:entity_properties" => parse_entity_properties(cond),
        "minecraft:damage_source_properties" => parse_damage_source_properties(cond),
        // Explicitly permissive, not a silent fallthrough. These gate which half of a
        // double plant drops, which needs a block lookup at an offset that the loot
        // evaluator cannot do yet. Suppressing them would stop tall grass and large ferns
        // dropping anything at all, which is worse than the current over-permissiveness.
        // TODO: implement once the loot context can read neighbouring block states.
        "minecraft:location_check" => LootCondition::None,
        // Anything else cannot be represented, so it must never pass. Returning `None`
        // here -- which is what this did before -- made the *pool* unconditional, so a
        // pool gated on something rare dropped every single time.
        _ => LootCondition::Unsupported,
    }
}

/// Parses `minecraft:entity_properties`, which asserts facts about an entity involved in
/// the drop. An empty predicate genuinely matches everything, so it stays `None`.
fn parse_entity_properties(cond: &ConditionStruct) -> LootCondition {
    let Some(predicate) = &cond.predicate else {
        return LootCondition::None;
    };
    if predicate.extra.is_empty() {
        return LootCondition::None;
    }

    let target = match cond.entity.as_deref() {
        Some("killer") => EntityTarget::Killer,
        Some("direct_killer") => EntityTarget::DirectKiller,
        _ => EntityTarget::This,
    };

    let mut parsed: Vec<LootCondition> = Vec::new();
    for (key, value) in &predicate.extra {
        let condition = match key.as_str() {
            "minecraft:flags" => value
                .get("is_baby")
                .and_then(serde_json::Value::as_bool)
                .map(LootCondition::ThisIsBaby),
            "minecraft:vehicle" => value
                .get("minecraft:entity_type")
                .and_then(serde_json::Value::as_str)
                .map(|name| LootCondition::ThisVehicleIs(Box::leak(name.to_string().into_boxed_str()))),
            "minecraft:entity_type" => value.as_str().map(|name| LootCondition::EntityTypeMatches {
                target,
                entity_type: Box::leak(name.to_string().into_boxed_str()),
            }),
            "minecraft:type_specific/cube_mob" => value
                .get("size")
                .and_then(serde_json::Value::as_i64)
                .map(|size| LootCondition::ThisCubeSizeIs(size as i32)),
            "minecraft:type_specific/raider" => value
                .get("is_captain")
                .and_then(serde_json::Value::as_bool)
                .map(LootCondition::ThisIsRaidCaptain),
            _ => None,
        };
        // An unrecognised key means the predicate as a whole is stricter than anything
        // that can be expressed, so the condition must fail rather than be approximated.
        let Some(condition) = condition else {
            return LootCondition::Unsupported;
        };
        parsed.push(condition);
    }

    match parsed.len() {
        0 => LootCondition::None,
        1 => parsed[0],
        _ => LootCondition::AllOf(Box::leak(parsed.into_boxed_slice())),
    }
}

/// Parses `minecraft:damage_source_properties`, currently the damage-type tags and the
/// direct entity's type.
fn parse_damage_source_properties(cond: &ConditionStruct) -> LootCondition {
    let Some(predicate) = &cond.predicate else {
        return LootCondition::None;
    };
    if predicate.extra.is_empty() {
        return LootCondition::None;
    }

    let mut parsed: Vec<LootCondition> = Vec::new();
    for (key, value) in &predicate.extra {
        match key.as_str() {
            "tags" => {
                let Some(entries) = value.as_array() else {
                    return LootCondition::Unsupported;
                };
                for entry in entries {
                    let (Some(id), Some(expected)) = (
                        entry.get("id").and_then(serde_json::Value::as_str),
                        entry.get("expected").and_then(serde_json::Value::as_bool),
                    ) else {
                        return LootCondition::Unsupported;
                    };
                    parsed.push(LootCondition::DamageTypeHasTag {
                        tag: Box::leak(id.to_string().into_boxed_str()),
                        expected,
                    });
                }
            }
            "direct_entity" => {
                let Some(name) = value
                    .get("minecraft:entity_type")
                    .and_then(serde_json::Value::as_str)
                else {
                    return LootCondition::Unsupported;
                };
                parsed.push(LootCondition::EntityTypeMatches {
                    target: EntityTarget::DirectKiller,
                    entity_type: Box::leak(name.to_string().into_boxed_str()),
                });
            }
            _ => return LootCondition::Unsupported,
        }
    }

    match parsed.len() {
        0 => LootCondition::None,
        1 => parsed[0],
        _ => LootCondition::AllOf(Box::leak(parsed.into_boxed_slice())),
    }
}

/// Entry-level conditions, which *select between alternatives* rather than gating a whole
/// pool.
///
/// The two levels degrade in opposite directions. Suppressing an unrepresentable **pool**
/// condition skips a rare drop, which is the safe error. Suppressing an unrepresentable
/// **entry** condition can remove every candidate from a weighted list and leave the pool
/// yielding nothing -- sheep would stop dropping wool, because each colour is an entry
/// gated on the sheep's colour. So an entry whose condition cannot be represented stays
/// selectable, keeping the pool productive at the cost of ignoring the distinction.
fn combine_entry_conditions(conditions: &[ConditionStruct]) -> LootCondition {
    let mut parsed_list: Vec<LootCondition> = Vec::new();
    for c in conditions {
        let parsed = parse_condition(c);
        if parsed == LootCondition::Unsupported || parsed == LootCondition::None {
            continue;
        }
        parsed_list.push(parsed);
    }
    match parsed_list.len() {
        0 => LootCondition::None,
        1 => parsed_list[0],
        _ => LootCondition::AllOf(Box::leak(parsed_list.into_boxed_slice())),
    }
}

fn combine_conditions(conditions: &[ConditionStruct]) -> LootCondition {
    let mut parsed_list: Vec<LootCondition> = Vec::new();
    for c in conditions {
        let parsed = parse_condition(c);
        // `None` means "always true" and adds nothing to an AllOf, so it is dropped.
        // `Unsupported` must be kept: discarding it is precisely what turned a gated pool
        // into an unconditional one.
        if parsed != LootCondition::None {
            parsed_list.push(parsed);
        }
    }
    match parsed_list.len() {
        0 => LootCondition::None,
        1 => parsed_list[0],
        _ => LootCondition::AllOf(Box::leak(parsed_list.into_boxed_slice())),
    }
}

#[derive(Deserialize, Clone, Debug)]
struct BonusParameterStruct {
    #[serde(rename = "bonusMultiplier", default)]
    bonus_multiplier: Option<i32>,
    #[serde(default)]
    extra: Option<i32>,
    #[serde(default)]
    probability: Option<f32>,
}

#[derive(Deserialize, Clone, Debug)]
struct EntryFunctionStruct {
    function: String,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    parameters: Option<BonusParameterStruct>,
    count: Option<CountStruct>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum LootTableValue {
    Reference(String),
    Inline(ChestLootTableJson),
}

#[derive(Deserialize, Clone, Debug)]
struct PoolEntryStruct {
    #[serde(rename = "type")]
    entry_type: String,
    name: Option<String>,
    #[serde(default)]
    value: Option<LootTableValue>,
    #[serde(default = "default_weight")]
    weight: i32,
    #[serde(default)]
    functions: Vec<EntryFunctionStruct>,
    #[serde(default)]
    conditions: Vec<ConditionStruct>,
    #[serde(default)]
    children: Vec<PoolEntryStruct>,
}

fn default_weight() -> i32 {
    1
}

#[derive(Deserialize, Clone, Debug)]
struct PoolStruct {
    #[serde(default)]
    entries: Vec<PoolEntryStruct>,
    #[serde(default = "default_rolls")]
    rolls: RollsStruct,
    #[serde(default)]
    conditions: Vec<ConditionStruct>,
}

fn default_rolls() -> RollsStruct {
    RollsStruct::Constant(1.0)
}

#[derive(Deserialize, Clone, Debug)]
struct ChestLootTableJson {
    #[serde(default)]
    random_sequence: Option<String>,
    #[serde(default)]
    pools: Vec<PoolStruct>,
}

fn path_to_key(relative: &str) -> String {
    format!("minecraft:{relative}")
}

fn path_to_ident(relative: &str) -> String {
    relative.replace('/', "_").to_shouty_snake_case()
}

struct ParsedEntry {
    item: String,
    weight: i32,
    min_count: i32,
    max_count: i32,
    condition: LootCondition,
    bonus_formula: Option<LootBonusFormula>,
}

fn extract_entries(
    entry: &PoolEntryStruct,
    inherited_condition: LootCondition,
    out: &mut Vec<ParsedEntry>,
) {
    extract_entries_with_depth(entry, inherited_condition, out, 0);
}

fn extract_entries_with_depth(
    entry: &PoolEntryStruct,
    inherited_condition: LootCondition,
    out: &mut Vec<ParsedEntry>,
    depth: usize,
) {
    if depth > 5 {
        return;
    }

    let entry_cond = match (
        inherited_condition,
        combine_entry_conditions(&entry.conditions),
    ) {
        (LootCondition::None, cond) | (cond, LootCondition::None) => cond,
        (first, second) if first == second => first,
        (first, second) => {
            LootCondition::AllOf(Box::leak(vec![first, second].into_boxed_slice()))
        }
    };

    match entry.entry_type.as_str() {
        "minecraft:empty" => {
            out.push(ParsedEntry {
                item: String::new(),
                weight: entry.weight,
                min_count: 0,
                max_count: 0,
                condition: entry_cond,
                bonus_formula: None,
            });
        }
        "minecraft:item" => {
            if let Some(name) = &entry.name {
                let (min_count, max_count) = entry
                    .functions
                    .iter()
                    .find(|f| f.function == "minecraft:set_count")
                    .and_then(|f| f.count.as_ref())
                    .map(|c| (c.min(), c.max()))
                    .unwrap_or((1, 1));

                let bonus_formula = entry.functions.iter().find_map(|f| {
                    if f.function == "minecraft:apply_bonus" {
                        match f.formula.as_deref() {
                            Some("minecraft:ore_drops") => Some(LootBonusFormula::OreDrops),
                            Some("minecraft:uniform_bonus_count") => {
                                let mult = f
                                    .parameters
                                    .as_ref()
                                    .and_then(|p| p.bonus_multiplier)
                                    .unwrap_or(1);
                                Some(LootBonusFormula::UniformBonusCount(mult))
                            }
                            Some("minecraft:binomial_with_bonus_count") => {
                                let extra =
                                    f.parameters.as_ref().and_then(|p| p.extra).unwrap_or(0);
                                let prob = f
                                    .parameters
                                    .as_ref()
                                    .and_then(|p| p.probability)
                                    .unwrap_or(0.0);
                                Some(LootBonusFormula::BinomialWithBonusCount {
                                    extra,
                                    probability: prob,
                                })
                            }
                            _ => None,
                        }
                    } else if f.function == "minecraft:enchanted_count_increase" {
                        let mult = f.count.as_ref().map_or(1, |c| c.max());
                        Some(LootBonusFormula::UniformBonusCount(mult))
                    } else {
                        None
                    }
                });

                out.push(ParsedEntry {
                    item: name.clone(),
                    weight: entry.weight,
                    min_count,
                    max_count,
                    condition: entry_cond,
                    bonus_formula,
                });
            }
        }
        "minecraft:tag" => {
            let tag_name_opt = entry.name.as_deref().or_else(|| match &entry.value {
                Some(LootTableValue::Reference(r)) => Some(r.as_str()),
                _ => None,
            });
            if let Some(tag_name) = tag_name_opt {
                let tag_rel = tag_name.strip_prefix("minecraft:").unwrap_or(tag_name);
                let tag_path =
                    Path::new("../../assets/datapacks/26_2/data/minecraft/tags/item")
                        .join(format!("{tag_rel}.json"));
                if let Ok(content) = fs::read_to_string(&tag_path) {
                    #[derive(Deserialize)]
                    struct TagJson {
                        values: Vec<String>,
                    }
                    if let Ok(tag_data) = serde_json::from_str::<TagJson>(&content) {
                        for item_name in tag_data.values {
                            out.push(ParsedEntry {
                                item: item_name,
                                weight: entry.weight,
                                min_count: 1,
                                max_count: 1,
                                condition: entry_cond,
                                bonus_formula: None,
                            });
                        }
                    }
                }
            }
        }
        "minecraft:loot_table" => match &entry.value {
            Some(LootTableValue::Reference(table_name)) => {
                let table_rel = table_name.strip_prefix("minecraft:").unwrap_or(table_name);
                let table_path =
                    Path::new("../../assets/datapacks/26_2/data/minecraft/loot_table")
                        .join(format!("{table_rel}.json"));
                if let Ok(content) = fs::read_to_string(&table_path) {
                    if let Ok(nested_table) =
                        serde_json::from_str::<ChestLootTableJson>(&content)
                    {
                        for pool in &nested_table.pools {
                            let mut pool_cond = entry_cond;
                            for c in &pool.conditions {
                                let parsed = parse_condition(c);
                                if parsed != LootCondition::None {
                                    pool_cond = parsed;
                                }
                            }
                            for child_entry in &pool.entries {
                                extract_entries_with_depth(
                                    child_entry,
                                    pool_cond,
                                    out,
                                    depth + 1,
                                );
                            }
                        }
                    }
                }
            }
            Some(LootTableValue::Inline(nested_table)) => {
                for pool in &nested_table.pools {
                    let mut pool_cond = entry_cond;
                    for c in &pool.conditions {
                        let parsed = parse_condition(c);
                        if parsed != LootCondition::None {
                            pool_cond = parsed;
                        }
                    }
                    for child_entry in &pool.entries {
                        extract_entries_with_depth(child_entry, pool_cond, out, depth + 1);
                    }
                }
            }
            None => {
                if let Some(name) = &entry.name {
                    let table_rel = name.strip_prefix("minecraft:").unwrap_or(name);
                    let table_path =
                        Path::new("../../assets/datapacks/26_2/data/minecraft/loot_table")
                            .join(format!("{table_rel}.json"));
                    if let Ok(content) = fs::read_to_string(&table_path) {
                        if let Ok(nested_table) =
                            serde_json::from_str::<ChestLootTableJson>(&content)
                        {
                            for pool in &nested_table.pools {
                                let mut pool_cond = entry_cond;
                                for c in &pool.conditions {
                                    let parsed = parse_condition(c);
                                    if parsed != LootCondition::None {
                                        pool_cond = parsed;
                                    }
                                }
                                for child_entry in &pool.entries {
                                    extract_entries_with_depth(
                                        child_entry,
                                        pool_cond,
                                        out,
                                        depth + 1,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        },
        "minecraft:alternatives" => {
            let mut saw_silk = false;
            let mut saw_shears = false;

            for child in &entry.children {
                let child_cond = combine_entry_conditions(&child.conditions);

                let effective_cond = if child_cond == LootCondition::SilkTouch {
                    saw_silk = true;
                    LootCondition::SilkTouch
                } else if child_cond == LootCondition::Shears {
                    saw_shears = true;
                    LootCondition::Shears
                } else if child_cond == LootCondition::SilkTouchOrShears {
                    saw_silk = true;
                    saw_shears = true;
                    LootCondition::SilkTouchOrShears
                } else if saw_silk && saw_shears {
                    LootCondition::NoSilkTouchOrShears
                } else if saw_silk {
                    LootCondition::NoSilkTouch
                } else if saw_shears {
                    LootCondition::NoSilkTouchOrShears
                } else {
                    entry_cond
                };

                extract_entries_with_depth(child, effective_cond, out, depth + 1);
            }
        }
        "minecraft:sequence" | "minecraft:group" => {
            for child in &entry.children {
                extract_entries_with_depth(child, entry_cond, out, depth + 1);
            }
        }
        _ => {}
    }
}

fn condition_to_tokens(cond: LootCondition) -> TokenStream {
    match cond {
        LootCondition::None => quote! { LootCondition::None },
        LootCondition::SilkTouch => quote! { LootCondition::SilkTouch },
        LootCondition::NoSilkTouch => quote! { LootCondition::NoSilkTouch },
        LootCondition::Shears => quote! { LootCondition::Shears },
        LootCondition::SilkTouchOrShears => quote! { LootCondition::SilkTouchOrShears },
        LootCondition::NoSilkTouchOrShears => quote! { LootCondition::NoSilkTouchOrShears },
        LootCondition::SurvivesExplosion => quote! { LootCondition::SurvivesExplosion },
        LootCondition::KilledByPlayer => quote! { LootCondition::KilledByPlayer },
        LootCondition::Unsupported => quote! { LootCondition::Unsupported },
        LootCondition::BlockStateProperties { block, properties } => {
            let pairs = properties.iter().map(|(k, v)| quote! { (#k, #v) });
            quote! {
                LootCondition::BlockStateProperties {
                    block: #block,
                    properties: &[#(#pairs),*],
                }
            }
        }
        LootCondition::ThisIsBaby(expected) => {
            quote! { LootCondition::ThisIsBaby(#expected) }
        }
        LootCondition::ThisVehicleIs(name) => {
            quote! { LootCondition::ThisVehicleIs(#name) }
        }
        LootCondition::EntityTypeMatches {
            target,
            entity_type,
        } => {
            let target = match target {
                EntityTarget::This => quote! { EntityTarget::This },
                EntityTarget::Killer => quote! { EntityTarget::Killer },
                EntityTarget::DirectKiller => quote! { EntityTarget::DirectKiller },
            };
            quote! {
                LootCondition::EntityTypeMatches {
                    target: #target,
                    entity_type: #entity_type,
                }
            }
        }
        LootCondition::ThisCubeSizeIs(size) => {
            quote! { LootCondition::ThisCubeSizeIs(#size) }
        }
        LootCondition::ThisIsRaidCaptain(expected) => {
            quote! { LootCondition::ThisIsRaidCaptain(#expected) }
        }
        LootCondition::DamageTypeHasTag { tag, expected } => {
            quote! { LootCondition::DamageTypeHasTag { tag: #tag, expected: #expected } }
        }
        LootCondition::RandomChance { chance } => {
            quote! { LootCondition::RandomChance { chance: #chance } }
        }
        LootCondition::RandomChanceWithEnchantedBonus {
            unenchanted_chance,
            enchanted_chance_base,
            enchanted_chance_per_level_above_first,
        } => {
            quote! {
                LootCondition::RandomChanceWithEnchantedBonus {
                    unenchanted_chance: #unenchanted_chance,
                    enchanted_chance_base: #enchanted_chance_base,
                    enchanted_chance_per_level_above_first: #enchanted_chance_per_level_above_first,
                }
            }
        }
        LootCondition::TableBonus { chances } => {
            let values = chances.iter();
            quote! { LootCondition::TableBonus { chances: &[#(#values),*] } }
        }
        LootCondition::AllOf(list) => {
            let tokens: Vec<TokenStream> = list.iter().copied().map(condition_to_tokens).collect();
            quote! { LootCondition::AllOf(&[#(#tokens),*]) }
        }
    }
}

fn bonus_to_tokens(bonus: Option<LootBonusFormula>) -> TokenStream {
    match bonus {
        None => quote! { None },
        Some(LootBonusFormula::OreDrops) => {
            quote! { Some(LootBonusFormula::OreDrops) }
        }
        Some(LootBonusFormula::UniformBonusCount(mult)) => {
            quote! { Some(LootBonusFormula::UniformBonusCount(#mult)) }
        }
        Some(LootBonusFormula::BinomialWithBonusCount { extra, probability }) => {
            quote! { Some(LootBonusFormula::BinomialWithBonusCount { extra: #extra, probability: #probability }) }
        }
    }
}

/// Emit static entry arrays and pool literals for one table.
/// Returns the list of `LootPool` literals (one per pool).
fn emit_table(
    prefix: &str,
    table: &ChestLootTableJson,
    tokens: &mut TokenStream,
) -> Vec<TokenStream> {
    let mut pool_literals = Vec::new();

    for (pool_idx, pool) in table.pools.iter().enumerate() {
        let min_rolls = pool.rolls.min();
        let max_rolls = pool.rolls.max();

        let pool_cond = combine_conditions(&pool.conditions);

        let mut parsed_entries = Vec::new();

        for entry in &pool.entries {
            extract_entries(entry, LootCondition::None, &mut parsed_entries);
        }

        let entry_literals: Vec<TokenStream> = parsed_entries
            .iter()
            .map(|e| {
                let name_lit = LitStr::new(&e.item, Span::call_site());
                let weight = e.weight;
                let min_count = e.min_count;
                let max_count = e.max_count;
                let cond_tokens = condition_to_tokens(e.condition);
                let bonus_tokens = bonus_to_tokens(e.bonus_formula);

                quote! {
                    LootEntry {
                        item: #name_lit,
                        weight: #weight,
                        min_count: #min_count,
                        max_count: #max_count,
                        condition: #cond_tokens,
                        bonus_formula: #bonus_tokens,
                    }
                }
            })
            .collect();

        // Emit the entries static array.
        let entries_ident = format_ident!("{}_POOL{}_ENTRIES", prefix, pool_idx);
        tokens.extend(quote! {
            static #entries_ident: &[LootEntry] = &[#(#entry_literals),*];
        });

        let pool_cond_tokens = condition_to_tokens(pool_cond);

        pool_literals.push(quote! {
            LootPool {
                entries: #entries_ident,
                min_rolls: #min_rolls,
                max_rolls: #max_rolls,
                condition: #pool_cond_tokens,
            }
        });
    }

    pool_literals
}

/// Recursively collect all `*.json` files under `dir`, returning a vec of
/// `(relative_stem_path, parsed_table)`.
fn collect_json_files(base: &Path, dir: &Path) -> Vec<(String, ChestLootTableJson)> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => panic!("failed to read directory {}: {e}", dir.display()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            result.extend(collect_json_files(base, &path));
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .with_extension("")
                .to_string_lossy()
                .to_string();

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => panic!("failed to read {}: {e}", path.display()),
            };

            let table: ChestLootTableJson = match serde_json::from_str(&content) {
                Ok(t) => t,
                Err(e) => panic!("failed to parse {}: {e}", path.display()),
            };

            result.push((relative, table));
        }
    }

    result
}

/// Read every loot JSON from `../../assets/datapacks/26_2/data/minecraft/loot_table/` (recursively)
/// and emit a `pumpkin-data/src/generated/chest_loot.rs` with static constants
/// and a `get_chest_loot_table(key) -> Option<&'static ChestLootTable>` function.
pub fn build() -> TokenStream {
    let base = Path::new("../../assets/datapacks/26_2/data/minecraft/loot_table");

    // Collect all JSON files recursively, sorted for deterministic output.
    let mut files: Vec<(String, ChestLootTableJson)> = collect_json_files(base, base);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut all_tokens = TokenStream::new();

    // Emit one set of statics per file
    let mut table_idents = Vec::new();
    let mut table_keys = Vec::new();
    let mut short_table_keys = Vec::new();

    for (relative_path, table) in &files {
        let prefix = path_to_ident(relative_path);
        let key = path_to_key(relative_path);
        let table_ident = format_ident!("{}", prefix);

        let pool_tokens = emit_table(&prefix, table, &mut all_tokens);

        let pools_ident = format_ident!("{}_POOLS", prefix);
        let sequence = match &table.random_sequence {
            Some(key) => quote! { Some(#key) },
            None => quote! { None },
        };
        all_tokens.extend(quote! {
    static #pools_ident: &[LootPool] = &[#(#pool_tokens),*];
    pub static #table_ident: LootTable = LootTable { random_sequence: #sequence, pools: #pools_ident };
});

        table_idents.push(table_ident.clone());
        table_keys.push(LitStr::new(&key, Span::call_site()));
        short_table_keys.push(LitStr::new(relative_path, Span::call_site()));
    }

    // Emit get_loot_table and get_chest_loot_table
    all_tokens.extend(quote! {
        #[must_use]
        pub fn get_loot_table(key: &str) -> Option<&'static LootTable> {
            match key {
                #(#table_keys | #short_table_keys => Some(&#table_idents),)*
                _ => None,
            }
        }

        #[must_use]
        pub fn get_chest_loot_table(key: &str) -> Option<&'static LootTable> {
            get_loot_table(key)
        }
    });

    quote! {
        pub use pumpkin_util::loot_table::*;
        #all_tokens
    }
}
