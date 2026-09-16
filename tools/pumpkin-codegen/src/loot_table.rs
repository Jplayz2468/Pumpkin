use std::{fs, path::Path};

use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use pumpkin_util::loot_table::{EntityTarget, LootCondition};
use quote::{format_ident, quote};
use serde::Deserialize;
use syn::LitStr;

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
                    let items: Vec<_> = match items_val {
                        serde_json::Value::String(s) => vec![s.as_str()],
                        serde_json::Value::Array(values) => values
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect(),
                        _ => return LootCondition::Unsupported,
                    };
                    return LootCondition::ToolItems(Box::leak(
                        items
                            .into_iter()
                            .map(|name| Box::leak(name.to_owned().into_boxed_str()) as &'static str)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    ));
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
            let terms: Vec<_> = cond
                .terms
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(parse_condition)
                .collect();
            LootCondition::AnyOf(Box::leak(terms.into_boxed_slice()))
        }
        "minecraft:inverted" => cond
            .term
            .as_ref()
            .map_or(LootCondition::Unsupported, |term| {
                LootCondition::Not(Box::leak(Box::new(parse_condition(term))))
            }),
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
/// the drop. An omitted predicate matches unconditionally; an explicit empty one requires an entity.
fn parse_entity_properties(cond: &ConditionStruct) -> LootCondition {
    let Some(predicate) = &cond.predicate else {
        return LootCondition::None;
    };
    let target = match cond.entity.as_deref() {
        Some("attacker" | "killer") => EntityTarget::Killer,
        Some("direct_attacker" | "direct_killer") => EntityTarget::DirectKiller,
        Some("this") | None => EntityTarget::This,
        _ => return LootCondition::Unsupported,
    };

    if predicate.extra.is_empty() { return LootCondition::EntityPresent(target); }

    let mut parsed: Vec<LootCondition> = Vec::new();
    for (key, value) in &predicate.extra {
        let condition = match key.as_str() {
            "minecraft:flags" => value.as_object().map(|flags| {
                if flags.is_empty() { return LootCondition::EntityPresent(target); }
                let flags: Vec<_> = flags.iter().map(|(key, value)| {
                    match (key.as_str(), value.as_bool()) {
                        ("is_on_fire", Some(expected)) => LootCondition::EntityOnFire { target, expected },
                        ("is_baby", Some(expected)) if target == EntityTarget::This => LootCondition::ThisIsBaby(expected),
                        _ => LootCondition::Unsupported,
                    }
                }).collect();
                LootCondition::AllOf(Box::leak(flags.into_boxed_slice()))
            }),
            "minecraft:equipment" => {
                // Preserve every constraint; the currently supported equipment shape
                // is the main-hand enchantment predicate used by smelting loot.
                let equipment = value.as_object();
                equipment.filter(|v| v.len() == 1).and_then(|v| v.get("mainhand"))
                    .and_then(serde_json::Value::as_object).filter(|v| v.len() == 1)
                    .and_then(|v| v.get("predicates")).and_then(serde_json::Value::as_object)
                    .filter(|v| v.len() == 1).and_then(|v| v.get("minecraft:enchantments"))
                    .and_then(serde_json::Value::as_array).map(|predicates| {
                        let mut conditions = vec![LootCondition::EntityMainhandHasEnchantments(target)];
                        conditions.extend(predicates.iter().map(|predicate| {
                            predicate.as_object().filter(|v| v.len() == 1)
                                .and_then(|v| v.get("enchantments")).and_then(serde_json::Value::as_str)
                                .map(|name| LootCondition::EntityMainhandEnchantment {
                                    target, enchantment: Box::leak(name.to_owned().into_boxed_str()),
                                }).unwrap_or(LootCondition::Unsupported)
                        }));
                        LootCondition::AllOf(Box::leak(conditions.into_boxed_slice()))
                    })
            }
            "minecraft:vehicle" => value
                .get("minecraft:entity_type")
                .and_then(serde_json::Value::as_str)
                .map(|name| {
                    LootCondition::ThisVehicleIs(Box::leak(name.to_string().into_boxed_str()))
                }),
            "minecraft:entity_type" => {
                value.as_str().map(|name| LootCondition::EntityTypeMatches {
                    target,
                    entity_type: Box::leak(name.to_string().into_boxed_str()),
                })
            }
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
struct EntryFunctionStruct {
    function: String,
    #[serde(default)]
    conditions: Vec<ConditionStruct>,
    #[serde(flatten)]
    fields: serde_json::Map<String, serde_json::Value>,
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
    quality: i32,
    #[serde(default)]
    expand: bool,
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
    rolls: serde_json::Value,
    #[serde(default)]
    bonus_rolls: serde_json::Value,
    #[serde(default)]
    functions: Vec<EntryFunctionStruct>,
    #[serde(default)]
    conditions: Vec<ConditionStruct>,
}

fn default_rolls() -> serde_json::Value {
    serde_json::json!(1.0)
}

#[derive(Deserialize, Clone, Debug)]
struct ChestLootTableJson {
    #[serde(default)]
    random_sequence: Option<String>,
    #[serde(default)]
    pools: Vec<PoolStruct>,
    #[serde(default)]
    functions: Vec<EntryFunctionStruct>,
}

fn path_to_key(relative: &str) -> String {
    format!("minecraft:{relative}")
}

fn path_to_ident(relative: &str) -> String {
    relative.replace('/', "_").to_shouty_snake_case()
}

fn entity_target_tokens(target: EntityTarget) -> TokenStream {
    match target {
        EntityTarget::This => quote! { EntityTarget::This },
        EntityTarget::Killer => quote! { EntityTarget::Killer },
        EntityTarget::DirectKiller => quote! { EntityTarget::DirectKiller },
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
        LootCondition::EntityPresent(target) => {
            let target = entity_target_tokens(target);
            quote! { LootCondition::EntityPresent(#target) }
        }
        LootCondition::EntityMainhandHasEnchantments(target) => {
            let target = entity_target_tokens(target);
            quote! { LootCondition::EntityMainhandHasEnchantments(#target) }
        }
        LootCondition::EntityOnFire { target, expected } => {
            let target = entity_target_tokens(target);
            quote! { LootCondition::EntityOnFire { target: #target, expected: #expected } }
        }
        LootCondition::EntityMainhandEnchantment { target, enchantment } => {
            let target = entity_target_tokens(target);
            quote! { LootCondition::EntityMainhandEnchantment { target: #target, enchantment: #enchantment } }
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
        LootCondition::AnyOf(list) => {
            let tokens: Vec<_> = list.iter().copied().map(condition_to_tokens).collect();
            quote! { LootCondition::AnyOf(&[#(#tokens),*]) }
        }
        LootCondition::Not(term) => {
            let term = condition_to_tokens(*term);
            quote! { LootCondition::Not(&#term) }
        }
        LootCondition::ToolItems(items) => quote! { LootCondition::ToolItems(&[#(#items),*]) },
        LootCondition::AllOf(list) => {
            let tokens: Vec<TokenStream> = list.iter().copied().map(condition_to_tokens).collect();
            quote! { LootCondition::AllOf(&[#(#tokens),*]) }
        }
    }
}

fn number_tokens(value: &serde_json::Value) -> TokenStream {
    if value.is_null() {
        return quote! { LootNumberProvider::Constant(0.0) };
    }
    if let Some(n) = value.as_f64() {
        let n = n as f32;
        return quote! { LootNumberProvider::Constant(#n) };
    }
    match value["type"].as_str().unwrap_or("") {
        "minecraft:constant" => number_tokens(&value["value"]),
        "minecraft:uniform" => {
            let min = number_tokens(&value["min"]);
            let max = number_tokens(&value["max"]);
            quote! { LootNumberProvider::Uniform(&#min, &#max) }
        }
        "minecraft:binomial" => {
            let n = number_tokens(&value["n"]);
            let p = number_tokens(&value["p"]);
            quote! { LootNumberProvider::Binomial(&#n, &#p) }
        }
        other => panic!("Unsupported built-in loot number provider {other}: {value}"),
    }
}

type BlockPropertyIds = std::collections::BTreeMap<String, std::collections::BTreeMap<String, u32>>;
fn block_property_ids() -> &'static BlockPropertyIds {
    static IDS: std::sync::OnceLock<BlockPropertyIds> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        serde_json::from_str(
            &fs::read_to_string("../../assets/block_property_ids.json")
                .expect("Java block property identities"),
        )
        .expect("block property identity JSON")
    })
}
fn property_lookup_tokens() -> TokenStream {
    let blocks: Vec<_> = block_property_ids()
        .iter()
        .filter(|(_, properties)| !properties.is_empty())
        .map(|(block, properties)| {
            let block = block.strip_prefix("minecraft:").unwrap_or(block);
            let properties: Vec<_> = properties
                .iter()
                .map(|(name, id)| quote! { #name => Some(#id), })
                .collect();
            quote! { #block => match property { #(#properties)* _ => None }, }
        })
        .collect();
    quote! { pub fn block_property_identity(block: &str, property: &str) -> Option<u32> {
        match block.strip_prefix("minecraft:").unwrap_or(block) { #(#blocks)* _ => None }
    } }
}

fn registry_set_tokens(value: &serde_json::Value) -> TokenStream {
    match value {
        serde_json::Value::Null => quote! { LootRegistrySet::All },
        serde_json::Value::String(name) if name.starts_with('#') => {
            let name = &name[1..]; quote! { LootRegistrySet::Tag(#name) }
        }
        serde_json::Value::String(name) => quote! { LootRegistrySet::Values(&[#name]) },
        serde_json::Value::Array(values) => {
            let names: Vec<_> = values.iter().map(|value| value.as_str().expect("registry identifier")).collect();
            quote! { LootRegistrySet::Values(&[#(#names),*]) }
        }
        _ => panic!("invalid registry set"),
    }
}

fn functions_tokens(functions: &[EntryFunctionStruct]) -> TokenStream {
    let functions: Vec<_> = functions.iter().map(|function| {
        let condition = condition_to_tokens(combine_conditions(&function.conditions));
        let f = &function.fields;
        let field = |key: &str| f.get(key).unwrap_or(&serde_json::Value::Null);
        let kind = match function.function.as_str() {
            "minecraft:set_components" => {
                static PATCHES: std::sync::LazyLock<Vec<serde_json::Value>> = std::sync::LazyLock::new(|| {
                    serde_json::from_str(&fs::read_to_string("../../assets/loot_component_patches.json").expect("canonical component patch export")).expect("component patch JSON")
                });
                let patch = PATCHES.iter().find(|patch| &patch["components"] == field("components"))
                    .expect("New component patch: rerun tools/vanilla/LootPatchTrimOracle.java to export canonical typed NBT");
                let bytes: Vec<u8> = patch["nbt"].as_array().expect("patch bytes").iter().map(|b| u8::try_from(b.as_u64().expect("byte")).expect("byte range")).collect();
                quote! { LootFunctionKind::SetComponents(&[#(#bytes),*]) }
            }
            "minecraft:set_instrument" => {
                let options = registry_set_tokens(field("options"));
                quote! { LootFunctionKind::SetInstrument(#options) }
            }
            "minecraft:set_name" if field("entity").is_null() => {
                let name = if field("name").is_null() { quote! { None } } else {
                    let json = field("name").to_string(); quote! { Some(#json) }
                };
                let item_name = match field("target").as_str().unwrap_or("custom_name") {
                    "custom_name" => false, "item_name" => true, target => panic!("Invalid name target {target}"),
                };
                quote! { LootFunctionKind::SetName { name_json: #name, item_name: #item_name } }
            }
            "minecraft:enchant_randomly" => {
                let options = registry_set_tokens(field("options"));
                let only_compatible = field("only_compatible").as_bool().unwrap_or(true);
                let include_additional_cost = field("include_additional_cost_component").as_bool().unwrap_or(false);
                quote! { LootFunctionKind::EnchantRandomly { options: #options, only_compatible: #only_compatible, include_additional_cost: #include_additional_cost } }
            }
            "minecraft:enchant_with_levels" => {
                let options = registry_set_tokens(field("options"));
                let levels = number_tokens(field("levels"));
                let include_additional_cost = field("include_additional_cost_component").as_bool().unwrap_or(false);
                quote! { LootFunctionKind::EnchantWithLevels { levels: #levels, options: #options, include_additional_cost: #include_additional_cost } }
            }
            "minecraft:set_enchantments" => {
                let enchantments: Vec<_> = field("enchantments").as_object().into_iter().flatten().map(|(name, level)| {
                    let level = number_tokens(level); quote! { (#name, #level) }
                }).collect();
                let add = field("add").as_bool().unwrap_or(false);
                quote! { LootFunctionKind::SetEnchantments { enchantments: &[#(#enchantments),*], add: #add } }
            }
            "minecraft:set_count" => {
                let count = number_tokens(field("count")); let add = field("add").as_bool().unwrap_or(false);
                quote! { LootFunctionKind::SetCount { count: #count, add: #add } }
            }
            "minecraft:limit_count" => {
                let limit = field("limit");
                let (min,max) = if limit.is_number() { (limit,limit) } else { (&limit["min"], &limit["max"]) };
                let option = |v: &serde_json::Value| if v.is_null() { quote! {None} } else { let n=number_tokens(v); quote! {Some(#n)} };
                let min = option(min); let max = option(max);
                quote! { LootFunctionKind::LimitCount { min: #min, max: #max } }
            }
            "minecraft:furnace_smelt" => {
                let use_input_count = field("use_input_count").as_bool().unwrap_or(true);
                quote! { LootFunctionKind::FurnaceSmelt { use_input_count: #use_input_count } }
            }
            "minecraft:set_stew_effect" => {
                let mut seen = std::collections::HashSet::new();
                let effects: Vec<_> = field("effects").as_array().into_iter().flatten().map(|entry| {
                    let effect = entry["type"].as_str().expect("stew effect type");
                    assert!(seen.insert(effect), "duplicate stew effect {effect}");
                    let duration = number_tokens(&entry["duration"]);
                    quote! { (#effect, #duration) }
                }).collect();
                quote! { LootFunctionKind::SetStewEffect(&[#(#effects),*]) }
            }
            "minecraft:set_ominous_bottle_amplifier" => {
                let amplifier = number_tokens(field("amplifier"));
                quote! { LootFunctionKind::SetOminousBottleAmplifier(#amplifier) }
            }
            "minecraft:set_damage" => {
                let damage = number_tokens(field("damage")); let add = field("add").as_bool().unwrap_or(false);
                quote! { LootFunctionKind::SetDamage { damage: #damage, add: #add } }
            }
            "minecraft:set_potion" => {
                let id = field("id").as_str().expect("potion identifier");
                quote! { LootFunctionKind::SetPotion(#id) }
            }
            "minecraft:copy_components" => {
                let source = field("source").as_str().expect("copy_components source");
                let names = |value: &serde_json::Value| value.as_array().map(|values| values.iter().map(|v| v.as_str().expect("component name").to_owned()).collect::<Vec<_>>());
                let include = match names(field("include")) {
                    Some(names) => quote! { Some(&[#(#names),*]) },
                    None => quote! { None },
                };
                let exclude = names(field("exclude")).unwrap_or_default();
                quote! { LootFunctionKind::CopyComponents { source: #source, include: #include, exclude: &[#(#exclude),*] } }
            }
            "minecraft:copy_state" => {
                let block = field("block").as_str().expect("copy_state block");
                let declared = block_property_ids().get(block).expect("copy_state known block");
                let mut properties = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for property in field("properties").as_array().expect("copy_state properties") {
                    let name = property.as_str().expect("copy_state property name");
                    if let Some(id) = declared.get(name) && seen.insert(name) { properties.push(quote! { (#name, #id) }); }
                }
                quote! { LootFunctionKind::CopyState { properties: &[#(#properties),*] } }
            }
            "minecraft:explosion_decay" => quote! { LootFunctionKind::ExplosionDecay },
            "minecraft:apply_bonus" => {
                let enchantment = field("enchantment").as_str().unwrap_or("minecraft:fortune");
                let parameters = field("parameters");
                let formula = match field("formula").as_str().unwrap_or("") {
                    "minecraft:ore_drops" => quote! { LootBonusFormula::OreDrops },
                    "minecraft:uniform_bonus_count" => { let mult=parameters["bonusMultiplier"].as_i64().unwrap_or(1) as i32; quote! { LootBonusFormula::UniformBonusCount(#mult) } }
                    "minecraft:binomial_with_bonus_count" => { let extra=parameters["extra"].as_i64().unwrap_or(0) as i32; let probability=parameters["probability"].as_f64().unwrap_or(0.0) as f32; quote! { LootBonusFormula::BinomialWithBonusCount { extra: #extra, probability: #probability } } }
                    other => panic!("Unsupported loot bonus {other}"),
                };
                quote! { LootFunctionKind::ApplyBonus { enchantment: #enchantment, formula: #formula } }
            }
            "minecraft:enchanted_count_increase" => {
                let enchantment = field("enchantment").as_str().unwrap_or("minecraft:looting");
                let count = number_tokens(field("count")); let limit=field("limit").as_i64().unwrap_or(0) as i32;
                quote! { LootFunctionKind::EnchantedCountIncrease { enchantment: #enchantment, count: #count, limit: #limit } }
            }
            other => quote! { LootFunctionKind::Unsupported(#other) },
        };
        quote! { LootFunction { condition: #condition, kind: #kind } }
    }).collect();
    quote! { &[#(#functions),*] }
}

fn tag_items(name: &str, visiting: &mut Vec<String>, result: &mut Vec<String>) {
    assert!(
        !visiting.iter().any(|id| id == name),
        "Recursive item tag {name}"
    );
    visiting.push(name.to_string());
    let path = Path::new("../../assets/datapacks/26_2/data/minecraft/tags/item").join(format!(
        "{}.json",
        name.strip_prefix("minecraft:").unwrap_or(name)
    ));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("loot item tag"))
            .expect("loot item tag JSON");
    for entry in json["values"].as_array().expect("tag values") {
        let id = entry
            .as_str()
            .or_else(|| entry["id"].as_str())
            .expect("tag identifier");
        if let Some(tag) = id.strip_prefix('#') {
            tag_items(tag, visiting, result);
        } else if !result.iter().any(|item| item == id) {
            result.push(id.to_owned());
        }
    }
    visiting.pop();
}

fn entry_tokens(entry: &PoolEntryStruct) -> TokenStream {
    let condition = condition_to_tokens(combine_conditions(&entry.conditions));
    let functions = functions_tokens(&entry.functions);
    let weight = entry.weight;
    let quality = entry.quality;
    let kind = match entry.entry_type.as_str() {
        "minecraft:item" => {
            let name = entry.name.as_ref().expect("item name");
            quote! { LootEntryKind::Item(#name) }
        }
        "minecraft:empty" => quote! { LootEntryKind::Empty },
        "minecraft:dynamic" => {
            let name = entry.name.as_ref().expect("dynamic name");
            quote! { LootEntryKind::Dynamic(#name) }
        }
        "minecraft:alternatives" | "minecraft:sequence" | "minecraft:group" => {
            let children: Vec<_> = entry.children.iter().map(entry_tokens).collect();
            let kind = match entry.entry_type.as_str() {
                "minecraft:alternatives" => format_ident!("Alternatives"),
                "minecraft:sequence" => format_ident!("Sequence"),
                _ => format_ident!("Group"),
            };
            quote! { LootEntryKind::#kind(&[#(#children),*]) }
        }
        "minecraft:loot_table" => match &entry.value {
            Some(LootTableValue::Inline(table)) => {
                let table = table_tokens(table);
                quote! { LootEntryKind::InlineTable(&#table) }
            }
            value => {
                let name = match value {
                    Some(LootTableValue::Reference(name)) => name,
                    _ => entry.name.as_ref().expect("nested table name"),
                };
                quote! { LootEntryKind::TableReference(#name) }
            }
        },
        "minecraft:tag" => {
            let name = entry.name.as_ref().expect("tag name");
            let expand = entry.expand;
            let mut items = Vec::new();
            tag_items(name, &mut Vec::new(), &mut items);
            quote! { LootEntryKind::Tag { items: &[#(#items),*], expand: #expand } }
        }
        other => quote! { LootEntryKind::Unsupported(#other) },
    };
    quote! { LootEntry { kind: #kind, weight: #weight, quality: #quality, condition: #condition, functions: #functions } }
}

fn table_tokens(table: &ChestLootTableJson) -> TokenStream {
    let pools:Vec<_>=table.pools.iter().map(|pool| {
        let entries:Vec<_>=pool.entries.iter().map(entry_tokens).collect();
        let rolls=number_tokens(&pool.rolls); let bonus=number_tokens(&pool.bonus_rolls);
        let condition=condition_to_tokens(combine_conditions(&pool.conditions)); let functions=functions_tokens(&pool.functions);
        quote! { LootPool { entries: &[#(#entries),*], rolls: #rolls, bonus_rolls: #bonus, condition: #condition, functions: #functions } }
    }).collect();
    let sequence = match &table.random_sequence {
        Some(key) => quote! {Some(#key)},
        None => quote! {None},
    };
    let functions = functions_tokens(&table.functions);
    quote! { LootTable { random_sequence: #sequence, pools: &[#(#pools),*], functions: #functions } }
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

        let value = table_tokens(table);
        all_tokens.extend(quote! { pub static #table_ident: LootTable = #value; });

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

    let property_lookup = property_lookup_tokens();
    quote! {
        pub use pumpkin_util::loot_table::*;
        #all_tokens
        #property_lookup
    }
}

/// The same parser/emitter used for built-in tables compiles the Java oracle inputs.
pub fn build_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_tree_tables.json")
            .expect("loot oracle tables"),
    )
    .expect("loot oracle JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}

pub fn build_component_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_component_tables.json")
            .expect("loot component tables"),
    )
    .expect("loot component JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}


pub fn build_copy_components_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_copy_components_tables.json")
            .expect("loot copy component tables"),
    ).expect("loot copy component JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}


pub fn build_transform_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_transform_tables.json")
            .expect("loot transform tables"),
    ).expect("loot transform JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}

pub fn build_enchantment_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_enchantment_tables.json")
            .expect("loot enchantment tables"),
    ).expect("loot enchantment JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}

pub fn build_name_instrument_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_name_instrument_tables.json")
            .expect("name/instrument fixture tables"),
    ).expect("name/instrument fixture JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}


pub fn build_patch_fixtures() -> TokenStream {
    let tables: Vec<ChestLootTableJson> = serde_json::from_str(
        &fs::read_to_string("../../crates/pumpkin/src/world/loot_patch_tables.json").expect("patch fixture tables")
    ).expect("patch fixture JSON");
    let tables: Vec<_> = tables.iter().map(table_tokens).collect();
    quote! { use pumpkin_util::loot_table::*; pub static TABLES: &[LootTable] = &[#(#tables),*]; }
}
