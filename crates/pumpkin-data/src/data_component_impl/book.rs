use crate::{
    component_hash,
    data_component_impl::{DataComponentImpl, get_i32_hash, get_str_hash},
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::{text::TextComponent, version::JavaMinecraftVersion};

/// The original value and its optional server-filtered replacement.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Filterable<T> {
    pub raw: T,
    pub filtered: Option<T>,
}
impl<T> From<T> for Filterable<T> {
    fn from(raw: T) -> Self {
        Self {
            raw,
            filtered: None,
        }
    }
}
impl<T> Filterable<T> {
    fn read(tag: &NbtTag, read: impl Fn(&NbtTag) -> Option<T>) -> Option<Self> {
        if let Some(c) = tag.extract_compound() {
            if let Some(raw) = c.get("raw").and_then(&read) {
                let filtered = match c.get("filtered") {
                    Some(v) => Some(read(v)?),
                    None => None,
                };
                return Some(Self { raw, filtered });
            }
        }
        read(tag).map(Self::from)
    }
    fn write(&self, write: impl Fn(&T) -> NbtTag) -> NbtTag {
        let mut c = NbtCompound::new();
        c.put("raw", write(&self.raw));
        if let Some(v) = &self.filtered {
            c.put("filtered", write(v));
        }
        NbtTag::Compound(c)
    }
    fn component_hash(&self, hash: impl Fn(&T) -> u32) -> u32 {
        let mut entries = vec![(get_str_hash("raw"), hash(&self.raw))];
        if let Some(v) = &self.filtered {
            entries.push((get_str_hash("filtered"), hash(v)));
        }
        component_hash::map(entries)
    }
}
fn string(tag: &NbtTag, max: usize) -> Option<String> {
    let value = tag.extract_string()?;
    (value.encode_utf16().count() <= max).then(|| value.to_string())
}
fn int(tag: &NbtTag) -> Option<i32> {
    Some(match tag {
        NbtTag::Byte(v) => i32::from(*v),
        NbtTag::Short(v) => i32::from(*v),
        NbtTag::Int(v) => *v,
        NbtTag::Long(v) => *v as i32,
        NbtTag::Float(v) => *v as i32,
        NbtTag::Double(v) => *v as i32,
        _ => return None,
    })
}
fn write_string(value: &String) -> NbtTag {
    NbtTag::String(value.clone().into_boxed_str())
}
fn write_text(value: &TextComponent) -> NbtTag {
    value.to_nbt_tag_for_version(&JavaMinecraftVersion::V_26_2)
}
fn read_text(tag: &NbtTag) -> Option<TextComponent> {
    // Check the shared text decoder before restoring exact NBT numeric widths.
    let component = TextComponent::try_from_nbt(tag)?;
    let json = component
        .0
        .to_json_value_for_version(&JavaMinecraftVersion::V_26_2);
    let flat = pumpkin_util::serde_json::to_string(&json).ok()?;
    // Gson escapes the JavaScript line separators even with HTML escaping disabled.
    let separators = flat
        .chars()
        .filter(|c| matches!(c, '\u{2028}' | '\u{2029}'))
        .count();
    (flat.encode_utf16().count() + separators * 5 <= 32767).then_some(component)
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WritableBookContentImpl {
    pub pages: Vec<Filterable<String>>,
}
impl WritableBookContentImpl {
    pub fn read_data(tag: &NbtTag) -> Option<Self> {
        let c = tag.extract_compound()?;
        let pages = match c.get("pages") {
            Some(v) => v.extract_list()?,
            None => &[],
        };
        if pages.len() > 100 {
            return None;
        }
        Some(Self {
            pages: pages
                .iter()
                .map(|p| Filterable::read(p, |v| string(v, 1024)))
                .collect::<Option<_>>()?,
        })
    }
}
impl DataComponentImpl for WritableBookContentImpl {
    fn write_data(&self) -> NbtTag {
        let mut c = NbtCompound::new();
        if !self.pages.is_empty() {
            c.put(
                "pages",
                NbtTag::List(self.pages.iter().map(|p| p.write(write_string)).collect()),
            );
        }
        NbtTag::Compound(c)
    }
    fn get_hash(&self) -> i32 {
        component_hash::map(if self.pages.is_empty() {
            vec![]
        } else {
            vec![(
                get_str_hash("pages"),
                component_hash::list(
                    self.pages
                        .iter()
                        .map(|p| p.component_hash(|v| get_str_hash(v))),
                ),
            )]
        }) as i32
    }
    default_impl!(WritableBookContent);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct WrittenBookContentImpl {
    pub title: Filterable<String>,
    pub author: String,
    pub generation: i32,
    pub pages: Vec<Filterable<TextComponent>>,
    pub resolved: bool,
}
impl WrittenBookContentImpl {
    pub fn read_data(tag: &NbtTag) -> Option<Self> {
        let c = tag.extract_compound()?;
        let generation = match c.get("generation") {
            Some(v) => int(v)?,
            None => 0,
        };
        if !(0..=3).contains(&generation) {
            return None;
        }
        let resolved = match c.get("resolved") {
            Some(v) => (int(v)? as i8) != 0,
            None => false,
        };
        let pages = match c.get("pages") {
            Some(v) => v.extract_list()?,
            None => &[],
        };
        Some(Self {
            title: Filterable::read(c.get("title")?, |v| string(v, 32))?,
            author: c.get_string("author")?.to_owned(),
            generation,
            pages: pages
                .iter()
                .map(|p| Filterable::read(p, read_text))
                .collect::<Option<_>>()?,
            resolved,
        })
    }
}
impl DataComponentImpl for WrittenBookContentImpl {
    fn write_data(&self) -> NbtTag {
        let mut c = NbtCompound::new();
        c.put("title", self.title.write(write_string));
        c.put_string("author", self.author.clone());
        if self.generation != 0 {
            c.put_int("generation", self.generation);
        }
        if !self.pages.is_empty() {
            c.put(
                "pages",
                NbtTag::List(self.pages.iter().map(|p| p.write(write_text)).collect()),
            );
        }
        if self.resolved {
            c.put_bool("resolved", true);
        }
        NbtTag::Compound(c)
    }
    fn get_hash(&self) -> i32 {
        let mut entries = vec![
            (
                get_str_hash("title"),
                self.title.component_hash(|v| get_str_hash(v)),
            ),
            (get_str_hash("author"), get_str_hash(&self.author)),
        ];
        if self.generation != 0 {
            entries.push((get_str_hash("generation"), get_i32_hash(self.generation)));
        }
        if !self.pages.is_empty() {
            entries.push((
                get_str_hash("pages"),
                component_hash::list(
                    self.pages
                        .iter()
                        .map(|p| p.component_hash(component_hash::text)),
                ),
            ));
        }
        if self.resolved {
            entries.push((
                get_str_hash("resolved"),
                component_hash::json(&pumpkin_util::serde_json::Value::Bool(true)),
            ));
        }
        component_hash::map(entries) as i32
    }
    default_impl!(WrittenBookContent);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DebugStickStateImpl;
impl DebugStickStateImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for DebugStickStateImpl {
    default_impl!(DebugStickState);
}
