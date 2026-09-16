//! Java 26.2 item/component predicates shared by container locks and other callers.
//! Predicates are their codec-decoded NBT form. Unavailable component codecs fail closed.
use pumpkin_data::{
    data_component::DataComponent,
    data_component_impl::{DataComponentImpl, read_data},
    item_stack::ItemStack,
    tag::{RegistryKey, get_tag_values},
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};

fn component(stack: &ItemStack, id: DataComponent) -> Option<&dyn DataComponentImpl> {
    if stack.is_empty() {
        return None;
    }
    if let Some((_, v)) = stack.patch.iter().find(|(kind, _)| *kind == id) {
        return v.as_deref();
    }
    stack
        .item
        .components
        .iter()
        .find(|(kind, _)| *kind == id)
        .map(|(_, v)| *v)
}
fn data(stack: &ItemStack, name: &str) -> Option<NbtTag> {
    component(stack, DataComponent::try_from_name(&canonical(name))?)
        .map(DataComponentImpl::write_data)
}
fn canonical(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    }
}
fn holder(set: &NbtTag, name: &str, registry: &str) -> bool {
    match set {
        NbtTag::String(value) => {
            if let Some(tag) = value.strip_prefix('#') {
                RegistryKey::from_string(registry)
                    .and_then(|registry| get_tag_values(registry, tag))
                    .is_some_and(|names| names.iter().any(|n| canonical(n) == canonical(name)))
            } else {
                canonical(value) == canonical(name)
            }
        }
        NbtTag::List(values) => values.iter().any(|value| {
            value
                .extract_string()
                .is_some_and(|v| canonical(v) == canonical(name))
        }),
        _ => false,
    }
}
fn int(value: &NbtTag) -> Option<i32> {
    match value {
        NbtTag::Byte(v) => Some(i32::from(*v)),
        NbtTag::Short(v) => Some(i32::from(*v)),
        NbtTag::Int(v) => Some(*v),
        _ => None,
    }
}
fn number(value: &NbtTag) -> Option<f64> {
    match value {
        NbtTag::Byte(v) => Some(f64::from(*v)),
        NbtTag::Short(v) => Some(f64::from(*v)),
        NbtTag::Int(v) => Some(f64::from(*v)),
        NbtTag::Long(v) => Some(*v as f64),
        NbtTag::Float(v) => Some(f64::from(*v)),
        NbtTag::Double(v) => Some(*v),
        _ => None,
    }
}
fn range(value: Option<&NbtTag>, number: f64) -> bool {
    value.is_none_or(|value| match value {
        NbtTag::Compound(c) => {
            c.get("min")
                .is_none_or(|v| self::number(v).is_some_and(|v| number >= v))
                && c.get("max")
                    .is_none_or(|v| self::number(v).is_some_and(|v| number <= v))
        }
        value => self::number(value) == Some(number),
    })
}
fn collection<T>(predicate: &NbtTag, values: &[T], test: impl Fn(&NbtTag, &T) -> bool) -> bool {
    let Some(p) = predicate.extract_compound() else {
        return false;
    };
    if !range(p.get("size"), values.len() as f64) {
        return false;
    }
    if let Some(contains) = p.get("contains") {
        let Some(contains) = contains.extract_list() else {
            return false;
        };
        if !contains.iter().all(|p| values.iter().any(|v| test(p, v))) {
            return false;
        }
    }
    if let Some(count) = p.get("count") {
        let Some(count) = count.extract_list() else {
            return false;
        };
        for entry in count {
            let Some(entry) = entry.extract_compound() else {
                return false;
            };
            let Some(p) = entry.get("test") else {
                return false;
            };
            if !range(
                entry.get("count"),
                values.iter().filter(|v| test(p, v)).count() as f64,
            ) {
                return false;
            }
        }
    }
    true
}
/// NbtUtils.compareNbt with list matching enabled. Repeated tests may match one value.
fn partial_nbt(expected: &NbtTag, actual: &NbtTag) -> bool {
    match (expected, actual) {
        (NbtTag::Compound(e), NbtTag::Compound(a)) => e
            .child_tags
            .iter()
            .all(|(k, v)| a.get(k).is_some_and(|a| partial_nbt(v, a))),
        (NbtTag::List(e), NbtTag::List(a)) => {
            if e.is_empty() {
                a.is_empty()
            } else {
                e.iter().all(|e| a.iter().any(|a| partial_nbt(e, a)))
            }
        }
        _ => expected == actual,
    }
}
pub fn matches(predicate: &NbtCompound, stack: &ItemStack) -> bool {
    if predicate
        .get("items")
        .is_some_and(|p| !holder(p, stack.get_item().registry_key, "item"))
    {
        return false;
    }
    if !range(
        predicate.get("count"),
        if stack.is_empty() {
            0.0
        } else {
            f64::from(stack.item_count)
        },
    ) {
        return false;
    }
    if let Some(exact) = predicate.get("components") {
        let Some(exact) = exact.extract_compound() else {
            return false;
        };
        for (name, value) in &exact.child_tags {
            let Some(id) = DataComponent::try_from_name(&canonical(name)) else {
                return false;
            };
            let Some(actual) = component(stack, id) else {
                return false;
            };
            let Some(expected) = read_data(id, value) else {
                return false;
            };
            // A stub read_data must not make arbitrary payloads compare equal.
            if matches!(expected.write_data(), NbtTag::End) || !actual.equal(expected.as_ref()) {
                return false;
            }
        }
    }
    if let Some(partial) = predicate.get("predicates") {
        let Some(partial) = partial.extract_compound() else {
            return false;
        };
        for (name, p) in &partial.child_tags {
            if !partial_component(name, p, stack) {
                return false;
            }
        }
    }
    true
}
fn fields_equal(p: &NbtCompound, value: &NbtCompound, fields: &[(&str, &str)]) -> bool {
    fields
        .iter()
        .all(|(pkey, vkey)| p.get(pkey).is_none_or(|p| value.get(vkey) == Some(p)))
}
fn firework(p: &NbtTag, value: &NbtTag) -> bool {
    let (Some(p), Some(value)) = (p.extract_compound(), value.extract_compound()) else {
        return false;
    };
    p.get_string("shape")
        .is_none_or(|p| p == value.get_string("shape").unwrap_or("small_ball"))
        && ["has_twinkle", "has_trail"].iter().all(|key| {
            p.get_bool(key)
                .is_none_or(|p| p == value.get_bool(key).unwrap_or(false))
        })
}
fn raw_page(value: &NbtTag) -> &NbtTag {
    value
        .extract_compound()
        .and_then(|v| v.get("raw"))
        .unwrap_or(value)
}
fn partial_component(name: &str, p: &NbtTag, stack: &ItemStack) -> bool {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    if name == "custom_data" {
        let parsed;
        let p = if let NbtTag::String(value) = p {
            let mut reader = pumpkin_command::string_reader::StringReader::new(value.as_ref());
            let Ok(value) = pumpkin_command::snbt::SnbtParser::parse_for_commands(&mut reader)
            else {
                return false;
            };
            reader.skip_whitespace();
            if reader.can_read_char() {
                return false;
            }
            parsed = value;
            &parsed
        } else {
            p
        };
        return partial_nbt(
            p,
            &data(stack, name).unwrap_or_else(|| NbtTag::Compound(NbtCompound::new())),
        );
    }
    let Some(value) = data(stack, name) else {
        return false;
    };
    match name {
        "damage" => {
            let Some(p) = p.extract_compound() else {
                return false;
            };
            let Some(damage) = int(&value) else {
                return false;
            };
            let max = data(stack, "max_damage")
                .as_ref()
                .and_then(int)
                .unwrap_or(0);
            range(p.get("damage"), f64::from(damage))
                && range(p.get("durability"), f64::from(max.wrapping_sub(damage)))
        }
        "enchantments" | "stored_enchantments" => {
            let (Some(predicates), Some(values)) = (p.extract_list(), value.extract_compound())
            else {
                return false;
            };
            predicates.iter().all(|p| {
                p.extract_compound().is_some_and(|p| {
                    values.child_tags.iter().any(|(name, level)| {
                        int(level).is_some_and(|level| {
                            level != 0
                                && p.get("enchantments")
                                    .is_none_or(|p| holder(p, name, "enchantment"))
                                && range(p.get("levels"), f64::from(level))
                        })
                    })
                })
            })
        }
        "potion_contents" => value
            .extract_compound()
            .and_then(|v| v.get_string("potion"))
            .is_some_and(|name| holder(p, name, "potion")),
        "villager/variant" => value
            .extract_string()
            .is_some_and(|name| holder(p, name, "villager_type")),
        "trim" => {
            let (Some(p), Some(value)) = (p.extract_compound(), value.extract_compound()) else {
                return false;
            };
            [("material", "trim_material"), ("pattern", "trim_pattern")]
                .iter()
                .all(|(key, registry)| {
                    p.get(key).is_none_or(|p| {
                        value
                            .get_string(key)
                            .is_some_and(|name| holder(p, name, registry))
                    })
                })
        }
        "jukebox_playable" => {
            let Some(p) = p.extract_compound() else {
                return false;
            };
            p.get("song").is_none_or(|p| {
                value
                    .extract_string()
                    .or_else(|| value.extract_compound().and_then(|v| v.get_string("song")))
                    .is_some_and(|name| holder(p, name, "jukebox_song"))
            })
        }
        "container" | "bundle_contents" => {
            let (Some(p), Some(values)) = (p.extract_compound(), value.extract_list()) else {
                return false;
            };
            let Some(p) = p.get("items") else {
                return true;
            };
            let items: Option<Vec<_>> = values
                .iter()
                .map(|v| {
                    let v = v.extract_compound()?;
                    let v = if name == "container" {
                        v.get_compound("item")?
                    } else {
                        v
                    };
                    ItemStack::read_item_stack(v)
                })
                .collect();
            items.is_some_and(|items| {
                collection(p, &items, |p, item| {
                    p.extract_compound().is_some_and(|p| matches(p, item))
                })
            })
        }
        "firework_explosion" => firework(p, &value),
        "fireworks" => {
            let (Some(p), Some(value)) = (p.extract_compound(), value.extract_compound()) else {
                return false;
            };
            range(
                p.get("flight_duration"),
                value.get("flight_duration").and_then(number).unwrap_or(0.0),
            ) && p.get("explosions").is_none_or(|p| {
                collection(p, value.get_list("explosions").unwrap_or(&[]), firework)
            })
        }
        "writable_book_content" | "written_book_content" => {
            let (Some(p), Some(value)) = (p.extract_compound(), value.extract_compound()) else {
                return false;
            };
            if name == "written_book_content"
                && (!fields_equal(p, value, &[("author", "author")])
                    || p.get("title")
                        .is_some_and(|p| value.get("title").is_none_or(|v| p != raw_page(v)))
                    || !range(
                        p.get("generation"),
                        value.get("generation").and_then(number).unwrap_or(0.0),
                    )
                    || p.get_bool("resolved")
                        .is_some_and(|p| p != value.get_bool("resolved").unwrap_or(false)))
            {
                return false;
            }
            p.get("pages").is_none_or(|p| {
                collection(p, value.get_list("pages").unwrap_or(&[]), |p, v| {
                    if name == "written_book_content" {
                        pumpkin_util::text::TextComponent::from_nbt(p)
                            == pumpkin_util::text::TextComponent::from_nbt(raw_page(v))
                    } else {
                        p == raw_page(v)
                    }
                })
            })
        }
        "attribute_modifiers" => {
            let Some(p) = p.extract_compound() else {
                return false;
            };
            let Some(values) = value.extract_list() else {
                return false;
            };
            p.get("modifiers").is_none_or(|p| {
                collection(p, values, |p, v| {
                    let (Some(p), Some(v)) = (p.extract_compound(), v.extract_compound()) else {
                        return false;
                    };
                    p.get("attribute").is_none_or(|p| {
                        v.get_string("type")
                            .is_some_and(|name| holder(p, name, "attribute"))
                    }) && fields_equal(p, v, &[("id", "id"), ("operation", "operation")])
                        && range(
                            p.get("amount"),
                            v.get("amount").and_then(number).unwrap_or(0.0),
                        )
                        && p.get_string("slot")
                            .is_none_or(|p| p == v.get_string("slot").unwrap_or("any"))
                })
            })
        }
        // All other registered component keys denote Java's AnyValue presence predicate.
        _ => {
            p.extract_compound().is_some()
                && DataComponent::try_from_name(&canonical(name)).is_some()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_nbt::deserializer::NbtReadHelperJava;
    use pumpkin_util::serde_json::Value;
    fn bytes(v: &Value) -> Vec<u8> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect()
    }
    #[test]
    fn item_predicates_match_java_codec_and_evaluator() {
        let fixture: Value =
            pumpkin_util::serde_json::from_str(include_str!("item_predicate_cases.json")).unwrap();
        let stacks: Vec<_> = fixture["states"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let item = pumpkin_data::item::Item::from_registry_key(
                    v["item"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("minecraft:")
                        .unwrap(),
                )
                .unwrap();
                let mut stack = ItemStack::new(v["count"].as_u64().unwrap() as u8, item);
                for (id, value) in
                    pumpkin_data::component_patch::decode(&bytes(&v["patch"])).expect("Java patch")
                {
                    if let Some(value) = value {
                        stack.set_data_component_dyn(value);
                    } else {
                        stack.remove_data_component(id);
                    }
                }
                stack
            })
            .collect();
        assert_eq!(stacks.len(), 315);
        assert_eq!(fixture["predicates"].as_array().unwrap().len(), 52);
        for case in fixture["component_hashes"].as_array().unwrap() {
            let tag = NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
                bytes(&case["nbt"]),
            )))
            .unwrap();
            let component = read_data(
                DataComponent::try_from_name(case["component"].as_str().unwrap()).unwrap(),
                &tag,
            )
            .unwrap();
            assert_eq!(component.get_hash(), case["hash"].as_i64().unwrap() as i32);
        }
        for case in fixture["predicates"].as_array().unwrap() {
            let p = NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
                bytes(&case["nbt"]),
            )))
            .unwrap();
            for (index, stack) in stacks.iter().enumerate() {
                assert_eq!(
                    matches(p.extract_compound().unwrap(), stack),
                    case["matches"][index].as_bool().unwrap(),
                    "predicate={} state={}",
                    case["input"],
                    fixture["states"][index]
                );
            }
        }
    }
}
