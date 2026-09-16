//! Decode persistent item predicates before evaluating container locks.
//! Unknown record fields are discarded, as in the Java codecs.
use pumpkin_data::{
    AttributeModifierSlot, Enchantment,
    attributes::Attributes,
    data_component::DataComponent,
    data_component_impl::{Operation, read_data},
    item::Item,
    potion::Potion,
    registry_reference,
    tag::{RegistryKey, get_tag_values},
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::{resource_location::normalize_identifier, text::TextComponent};

fn compound(value: NbtCompound) -> NbtTag {
    NbtTag::Compound(value)
}
fn string(value: &NbtTag) -> Option<NbtTag> {
    Some(NbtTag::String(value.extract_string()?.into()))
}
fn identifier(value: &NbtTag) -> Option<NbtTag> {
    Some(NbtTag::String(
        normalize_identifier(value.extract_string()?)?.into(),
    ))
}
fn integer(value: &NbtTag) -> Option<i32> {
    Some(match value {
        NbtTag::Byte(v) => i32::from(*v),
        NbtTag::Short(v) => i32::from(*v),
        NbtTag::Int(v) => *v,
        NbtTag::Long(v) => *v as i32,
        NbtTag::Float(v) => *v as i32,
        NbtTag::Double(v) => *v as i32,
        _ => return None,
    })
}
fn double(value: &NbtTag) -> Option<f64> {
    Some(match value {
        NbtTag::Byte(v) => f64::from(*v),
        NbtTag::Short(v) => f64::from(*v),
        NbtTag::Int(v) => f64::from(*v),
        NbtTag::Long(v) => *v as f64,
        NbtTag::Float(v) => f64::from(*v),
        NbtTag::Double(v) => *v,
        _ => return None,
    })
}
fn boolean(value: &NbtTag) -> Option<NbtTag> {
    Some(NbtTag::Byte(i8::from(integer(value)? as i8 != 0)))
}
fn bounds(value: &NbtTag, floating: bool) -> Option<NbtTag> {
    let number = |v: &NbtTag| {
        if floating {
            double(v).map(NbtTag::Double)
        } else {
            integer(v).map(NbtTag::Int)
        }
    };
    let Some(c) = value.extract_compound() else {
        return number(value);
    };
    let min = c.get("min").map(number).transpose_option()?;
    let max = c.get("max").map(number).transpose_option()?;
    if let (Some(min), Some(max)) = (&min, &max) {
        let (a, b) = (double(min)?, double(max)?);
        // Double.compare orders NaN last and -0 before +0.
        let compare = if a.is_nan() {
            if b.is_nan() {
                std::cmp::Ordering::Equal
            } else {
                std::cmp::Ordering::Greater
            }
        } else if b.is_nan() {
            std::cmp::Ordering::Less
        } else {
            a.total_cmp(&b)
        };
        if compare.is_gt() {
            return None;
        }
        if compare.is_eq() {
            return Some(min.clone());
        }
    }
    let mut out = NbtCompound::new();
    if let Some(v) = min {
        out.put("min", v);
    }
    if let Some(v) = max {
        out.put("max", v);
    }
    Some(compound(out))
}
trait TransposeOption<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}
impl<T> TransposeOption<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> {
        match self {
            None => Some(None),
            Some(v) => v.map(Some),
        }
    }
}
fn ints(v: &NbtTag) -> Option<NbtTag> {
    bounds(v, false)
}
fn doubles(v: &NbtTag) -> Option<NbtTag> {
    bounds(v, true)
}
struct Decoder {
    recover: bool,
}
impl Decoder {
    fn optional(
        &self,
        out: &mut NbtCompound,
        source: &NbtCompound,
        key: &str,
        decode: impl FnOnce(&NbtTag) -> Option<NbtTag>,
        omit_empty: bool,
    ) -> Option<()> {
        if let Some(v) = source.get(key) {
            let Some(v) = decode(v) else {
                return self.recover.then_some(());
            };
            if !omit_empty
                || !v
                    .extract_compound()
                    .is_some_and(|c| c.child_tags.is_empty())
            {
                out.put(key, v);
            }
        }
        Some(())
    }
    fn exists(&self, registry: &str, name: &str) -> bool {
        let Some(path) = name.strip_prefix("minecraft:") else {
            return false;
        };
        match registry {
            "item" => Item::from_registry_key(path).is_some(),
            "attribute" => Attributes::ALL.iter().any(|a| a.name == name),
            "enchantment" => Enchantment::from_name(path).is_some(),
            "potion" => Potion::from_name(path).is_some(),
            "villager_type" => [
                "desert", "jungle", "plains", "savanna", "snow", "swamp", "taiga",
            ]
            .contains(&path),
            _ => registry_reference::id(registry, name).is_some(),
        }
    }
    fn holders(&self, value: &NbtTag, registry: &str) -> Option<NbtTag> {
        let entry = |v: &NbtTag| {
            let name = normalize_identifier(v.extract_string()?)?;
            self.exists(registry, &name)
                .then_some(NbtTag::String(name.into()))
        };
        if let Some(v) = value.extract_string() {
            if let Some(tag) = v.strip_prefix('#') {
                let tag = normalize_identifier(tag)?;
                let category = RegistryKey::from_string(registry)?;
                get_tag_values(category, &tag)?;
                return Some(NbtTag::String(format!("#{tag}").into()));
            }
            return entry(value);
        }
        let values = self.list(value, entry)?;
        if values.len() == 1 {
            values.into_iter().next()
        } else {
            Some(NbtTag::List(values))
        }
    }
    fn text(&self, value: &NbtTag) -> Option<NbtTag> {
        Some(
            TextComponent::try_from_nbt(value)?
                .to_nbt_tag_for_version(&pumpkin_util::version::JavaMinecraftVersion::V_26_2),
        )
    }
    fn collection(
        &self,
        value: &NbtTag,
        entry: impl Fn(&NbtTag) -> Option<NbtTag>,
    ) -> Option<NbtTag> {
        let c = value.extract_compound()?;
        let mut out = NbtCompound::new();
        self.optional(&mut out, c, "size", ints, false)?;
        self.optional(
            &mut out,
            c,
            "contains",
            |v| Some(NbtTag::List(self.list(v, &entry)?)),
            false,
        )?;
        self.optional(
            &mut out,
            c,
            "count",
            |v| {
                Some(NbtTag::List(self.list(v, |v| {
                    let c = v.extract_compound()?;
                    let mut out = NbtCompound::new();
                    out.put("test", entry(c.get("test")?)?);
                    out.put("count", ints(c.get("count")?)?);
                    Some(compound(out))
                })?))
            },
            false,
        )?;
        Some(compound(out))
    }
    fn firework(&self, value: &NbtTag) -> Option<NbtTag> {
        let c = value.extract_compound()?;
        let mut out = NbtCompound::new();
        self.optional(
            &mut out,
            c,
            "shape",
            |v| {
                ["small_ball", "large_ball", "star", "creeper", "burst"]
                    .contains(&v.extract_string()?)
                    .then(|| v.clone())
            },
            false,
        )?;
        self.optional(&mut out, c, "has_trail", boolean, false)?;
        self.optional(&mut out, c, "has_twinkle", boolean, false)?;
        Some(compound(out))
    }
    fn modifier(&self, value: &NbtTag) -> Option<NbtTag> {
        let c = value.extract_compound()?;
        let mut out = NbtCompound::new();
        self.optional(
            &mut out,
            c,
            "attribute",
            |v| self.holders(v, "attribute"),
            false,
        )?;
        self.optional(&mut out, c, "id", identifier, false)?;
        self.optional(&mut out, c, "amount", doubles, true)?;
        self.optional(
            &mut out,
            c,
            "operation",
            |v| Operation::from_name(v.extract_string()?).map(|_| v.clone()),
            false,
        )?;
        self.optional(
            &mut out,
            c,
            "slot",
            |v| AttributeModifierSlot::from_name(v.extract_string()?).map(|_| v.clone()),
            false,
        )?;
        Some(compound(out))
    }
    fn enchantment(&self, value: &NbtTag) -> Option<NbtTag> {
        let c = value.extract_compound()?;
        let mut out = NbtCompound::new();
        self.optional(
            &mut out,
            c,
            "enchantments",
            |v| self.holders(v, "enchantment"),
            false,
        )?;
        self.optional(&mut out, c, "levels", ints, true)?;
        Some(compound(out))
    }
    fn custom_data(&self, value: &NbtTag) -> Option<NbtTag> {
        if value.extract_compound().is_some() {
            return Some(value.clone());
        }
        let mut reader = pumpkin_command::string_reader::StringReader::new(value.extract_string()?);
        let result = pumpkin_command::snbt::SnbtParser::parse_for_commands(&mut reader).ok()?;
        reader.skip_whitespace();
        (!reader.can_read_char() && result.extract_compound().is_some()).then_some(result)
    }
    fn partial(&self, name: &str, value: &NbtTag) -> Option<NbtTag> {
        match name {
            "custom_data" => return self.custom_data(value),
            "enchantments" | "stored_enchantments" => {
                return Some(NbtTag::List(self.list(value, |v| self.enchantment(v))?));
            }
            "potion_contents" => return self.holders(value, "potion"),
            "villager/variant" => return self.holders(value, "villager_type"),
            "firework_explosion" => return self.firework(value),
            _ => {}
        }
        let c = value.extract_compound()?;
        let mut out = NbtCompound::new();
        match name {
            "damage" => {
                for key in ["damage", "durability"] {
                    self.optional(&mut out, c, key, ints, true)?;
                }
            }
            "container" | "bundle_contents" => self.optional(
                &mut out,
                c,
                "items",
                |v| self.collection(v, |v| self.decode_tag(v)),
                false,
            )?,
            "attribute_modifiers" => self.optional(
                &mut out,
                c,
                "modifiers",
                |v| self.collection(v, |v| self.modifier(v)),
                false,
            )?,
            "fireworks" => {
                self.optional(&mut out, c, "flight_duration", ints, true)?;
                self.optional(
                    &mut out,
                    c,
                    "explosions",
                    |v| self.collection(v, |v| self.firework(v)),
                    false,
                )?;
            }
            "writable_book_content" => {
                self.optional(&mut out, c, "pages", |v| self.collection(v, string), false)?
            }
            "written_book_content" => {
                self.optional(
                    &mut out,
                    c,
                    "pages",
                    |v| self.collection(v, |v| self.text(v)),
                    false,
                )?;
                for key in ["author", "title"] {
                    self.optional(&mut out, c, key, string, false)?;
                }
                self.optional(&mut out, c, "generation", ints, true)?;
                self.optional(&mut out, c, "resolved", boolean, false)?;
            }
            "trim" => {
                for (key, registry) in [("material", "trim_material"), ("pattern", "trim_pattern")]
                {
                    self.optional(&mut out, c, key, |v| self.holders(v, registry), false)?;
                }
            }
            "jukebox_playable" => self.optional(
                &mut out,
                c,
                "song",
                |v| self.holders(v, "jukebox_song"),
                false,
            )?,
            _ => {} // AnyValue's unit map codec ignores all fields.
        }
        Some(compound(out))
    }
    fn exact(&self, id: DataComponent, value: &NbtTag) -> Option<NbtTag> {
        if id == DataComponent::CreativeSlotLock {
            return None;
        }
        let owned;
        let value = if matches!(
            id,
            DataComponent::Damage | DataComponent::MaxDamage | DataComponent::MaxStackSize
        ) {
            let number = integer(value)?;
            let valid = match id {
                DataComponent::Damage => number >= 0,
                DataComponent::MaxDamage => number > 0,
                _ => (1..=99).contains(&number),
            };
            if !valid {
                return None;
            }
            owned = NbtTag::Int(number);
            &owned
        } else {
            value
        };
        if id == DataComponent::Lock {
            return self.decode_tag(value);
        }
        let decoded = read_data(id, value)?;
        match decoded.write_data() {
            NbtTag::End => Some(value.clone()), // Retain codecs not yet implemented by the data layer.
            value => Some(value),
        }
    }
    fn decode_tag(&self, value: &NbtTag) -> Option<NbtTag> {
        self.decode(value.extract_compound()?).map(compound)
    }
    fn decode(&self, source: &NbtCompound) -> Option<NbtCompound> {
        let mut out = NbtCompound::new();
        self.optional(
            &mut out,
            source,
            "items",
            |v| self.holders(v, "item"),
            false,
        )?;
        self.optional(&mut out, source, "count", ints, true)?;
        for key in ["components", "predicates"] {
            self.optional(
                &mut out,
                source,
                key,
                |v| {
                    let mut out = NbtCompound::new();
                    for (name, value) in &v.extract_compound()?.child_tags {
                        let decoded = (|| {
                            let name = normalize_identifier(name)?;
                            let id = DataComponent::try_from_name(&name)?;
                            let value = if key == "components" {
                                self.exact(id, value)?
                            } else {
                                self.partial(name.strip_prefix("minecraft:")?, value)?
                            };
                            Some((name, value))
                        })();
                        let Some((name, value)) = decoded else {
                            if self.recover {
                                continue;
                            }
                            return None;
                        };
                        if out.get(&name).is_some() {
                            return None;
                        }
                        out.put(&name, value);
                    }
                    Some(compound(out))
                },
                true,
            )?;
        }
        Some(out)
    }

    fn list(
        &self,
        value: &NbtTag,
        decode: impl Fn(&NbtTag) -> Option<NbtTag>,
    ) -> Option<Vec<NbtTag>> {
        let mut out = Vec::new();
        for value in value.extract_list()? {
            match decode(value) {
                Some(value) => out.push(value),
                None if self.recover => {}
                None => return None,
            }
        }
        Some(out)
    }
}

/// Strict codec decoding for components and validation.
pub fn decode(source: &NbtCompound) -> Option<NbtCompound> {
    Decoder { recover: false }.decode(source)
}
/// ValueInput accepts the partial value attached to a Java codec error.
pub fn load(source: &NbtCompound) -> Option<NbtCompound> {
    Decoder { recover: true }.decode(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::entities::container_lock::ContainerLock;
    use pumpkin_data::{
        data_component_impl::{CustomNameImpl, LockImpl},
        item_stack::ItemStack,
    };
    use pumpkin_nbt::deserializer::NbtReadHelperJava;
    fn nbt(value: &serde_json::Value) -> NbtTag {
        let bytes: Vec<_> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
            bytes,
        )))
        .unwrap()
    }
    #[test]
    fn container_lock_decode_fallback_and_evaluation_match_java() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("lock_codec_cases.json")).unwrap();
        let cases = cases.as_array().unwrap();
        assert_eq!(cases.len(), 286);
        for case in cases {
            let input = nbt(&case["nbt"]);
            let source = input.extract_compound().unwrap();
            let decoded = decode(source);
            assert_eq!(
                decoded.is_some(),
                case["valid"].as_bool().unwrap(),
                "codec {}",
                case["input"]
            );
            let mut root = NbtCompound::new();
            root.put("lock", input.clone());
            let lock = ContainerLock::default();
            lock.read_nbt(&root);
            let mut saved = NbtCompound::new();
            lock.write_nbt(&mut saved);
            assert_eq!(
                compound(saved),
                nbt(&case["saved"]),
                "saved {}",
                case["input"]
            );
            let mut placed = ItemStack::new(1, &Item::CHEST);
            placed.set_data_component(LockImpl {
                predicate: source.clone(),
            });
            let placed_lock = ContainerLock::default();
            placed_lock.apply(&placed);
            let mut index = 0;
            for item in [
                &Item::AIR,
                &Item::STICK,
                &Item::STONE,
                &Item::OAK_PLANKS,
                &Item::DIAMOND_SWORD,
            ] {
                for count in [0, 1, 3] {
                    let mut stack = ItemStack::new(count, item);
                    stack.set_data_component(CustomNameImpl {
                        name: TextComponent::text("key"),
                    });
                    let expected = case["matches"][index].as_bool().unwrap();
                    assert_eq!(
                        lock.can_open(&stack, false),
                        expected,
                        "loaded {}, item {}, count {count}",
                        case["input"],
                        item.registry_key
                    );
                    assert_eq!(
                        placed_lock.can_open(&stack, false),
                        expected,
                        "placed {}",
                        case["input"]
                    );
                    assert!(lock.can_open(&stack, true));
                    index += 1;
                }
            }
        }
    }
}
