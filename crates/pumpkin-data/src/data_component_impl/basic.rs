use crate::data_component_impl::{DataComponentImpl, get_i32_hash, get_str_hash};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::text::TextComponent;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub struct CustomDataImpl {
    pub data: NbtCompound,
}
impl CustomDataImpl {
    #[must_use]
    pub const fn new(data: NbtCompound) -> Self {
        Self { data }
    }
    #[must_use]
    pub fn read_data(tag: &NbtTag) -> Option<Self> {
        if let NbtTag::Compound(c) = tag {
            Some(Self { data: c.clone() })
        } else {
            None
        }
    }
}
impl DataComponentImpl for CustomDataImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Compound(self.data.clone())
    }
    fn get_hash(&self) -> i32 {
        0
    }
    default_impl!(CustomData);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MaxStackSizeImpl {
    pub size: u8,
}
impl MaxStackSizeImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_int().map(|size| Self { size: size as u8 })
    }
}
impl DataComponentImpl for MaxStackSizeImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Int(self.size as i32)
    }
    fn get_hash(&self) -> i32 {
        get_i32_hash(self.size as i32) as i32
    }
    default_impl!(MaxStackSize);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MaxDamageImpl {
    pub max_damage: i32,
}
impl MaxDamageImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_int().map(|max_damage| Self { max_damage })
    }
}
impl DataComponentImpl for MaxDamageImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Int(self.max_damage)
    }
    default_impl!(MaxDamage);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DamageImpl {
    pub damage: i32,
}
impl DamageImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_int().map(|damage| Self { damage })
    }
}
impl DataComponentImpl for DamageImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Int(self.damage)
    }
    fn get_hash(&self) -> i32 {
        get_i32_hash(self.damage) as i32
    }
    default_impl!(Damage);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct UnbreakableImpl;
impl UnbreakableImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for UnbreakableImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Compound(NbtCompound::new())
    }
    fn get_hash(&self) -> i32 {
        0
    }
    default_impl!(Unbreakable);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CustomNameImpl {
    pub name: TextComponent,
}
impl CustomNameImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        Some(Self {
            name: TextComponent::from_nbt(data),
        })
    }
}
impl DataComponentImpl for CustomNameImpl {
    fn write_data(&self) -> NbtTag {
        self.name
            .to_nbt_tag_for_version(&pumpkin_util::version::JavaMinecraftVersion::V_26_2)
    }
    fn get_hash(&self) -> i32 {
        crate::component_hash::text(&self.name) as i32
    }
    default_impl!(CustomName);
}

/// Compact translation keys keep generated prototypes const; dynamic names retain
/// their complete component, including literal text, arguments and style.
#[derive(Clone, Debug)]
pub enum ItemNameValue {
    Translation(Cow<'static, str>),
    Component(TextComponent),
}
impl PartialEq for ItemNameValue {
    fn eq(&self, other: &Self) -> bool {
        self.component() == other.component()
    }
}
impl Eq for ItemNameValue {}
impl std::hash::Hash for ItemNameValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.component().hash(state);
    }
}
impl ItemNameValue {
    #[allow(deprecated)]
    pub fn component(&self) -> TextComponent {
        match self {
            Self::Translation(key) => TextComponent::translate(key.clone(), &[]),
            Self::Component(component) => component.clone(),
        }
    }
}
impl From<&'static str> for ItemNameValue {
    fn from(value: &'static str) -> Self {
        Self::Translation(value.into())
    }
}
impl From<String> for ItemNameValue {
    fn from(value: String) -> Self {
        Self::Translation(value.into())
    }
}
impl From<Cow<'static, str>> for ItemNameValue {
    fn from(value: Cow<'static, str>) -> Self {
        Self::Translation(value)
    }
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ItemNameImpl {
    pub name: ItemNameValue,
}
impl ItemNameImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        Some(Self {
            name: ItemNameValue::Component(TextComponent::from_nbt(data)),
        })
    }
}
impl DataComponentImpl for ItemNameImpl {
    fn write_data(&self) -> NbtTag {
        self.name
            .component()
            .to_nbt_tag_for_version(&pumpkin_util::version::JavaMinecraftVersion::V_26_2)
    }
    fn get_hash(&self) -> i32 {
        crate::component_hash::text(&self.name.component()) as i32
    }
    default_impl!(ItemName);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ItemModelImpl {
    pub id: Cow<'static, str>,
}
impl ItemModelImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_string().map(|id| Self {
            id: Cow::Owned(id.to_string()),
        })
    }
}
impl DataComponentImpl for ItemModelImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::String(self.id.clone().into_owned().into())
    }
    fn get_hash(&self) -> i32 {
        get_str_hash(self.id.as_ref()) as i32
    }
    default_impl!(ItemModel);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LoreImpl {
    pub lines: Vec<TextComponent>,
}
impl LoreImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        let NbtTag::List(lines) = data else {
            return None;
        };

        Some(Self {
            lines: lines
                .iter()
                .filter_map(NbtTag::extract_string)
                .map(|line| TextComponent::text(line.to_owned()))
                .collect(),
        })
    }
}
impl DataComponentImpl for LoreImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::List(
            self.lines
                .iter()
                .map(|line| NbtTag::String(line.clone().get_text().into_boxed_str()))
                .collect(),
        )
    }
    default_impl!(Lore);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Rarity {
    #[default]
    Common = 0,
    Uncommon = 1,
    Rare = 2,
    Epic = 3,
}

impl Rarity {
    #[must_use]
    pub fn from_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::Common),
            1 => Some(Self::Uncommon),
            2 => Some(Self::Rare),
            3 => Some(Self::Epic),
            _ => None,
        }
    }

    #[must_use]
    pub fn to_id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "common" => Some(Self::Common),
            "uncommon" => Some(Self::Uncommon),
            "rare" => Some(Self::Rare),
            "epic" => Some(Self::Epic),
            _ => None,
        }
    }

    #[must_use]
    pub fn to_name(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct RarityImpl {
    pub rarity: Rarity,
}

impl RarityImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        let name = data.extract_string()?;
        Some(Self {
            rarity: Rarity::from_name(name)?,
        })
    }
}

impl DataComponentImpl for RarityImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::String(self.rarity.to_name().into())
    }

    fn get_hash(&self) -> i32 {
        crate::data_component_impl::get_i32_hash(self.rarity.to_id()) as i32
    }

    default_impl!(Rarity);
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomModelDataImpl {
    pub floats: Vec<f32>,
    pub flags: Vec<bool>,
    pub strings: Vec<String>,
    pub colors: Vec<i32>,
}
impl CustomModelDataImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        let compound = data.extract_compound()?;
        let floats = compound
            .get_list("floats")
            .map(|l| l.iter().filter_map(NbtTag::extract_float).collect())
            .unwrap_or_default();
        let flags = compound
            .get_list("flags")
            .map(|l| l.iter().filter_map(NbtTag::extract_bool).collect())
            .unwrap_or_default();
        let strings = compound
            .get_list("strings")
            .map(|l| {
                l.iter()
                    .filter_map(|t| t.extract_string().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        // Vanilla encodes the color list as ints, but tolerate a packed int array too.
        let colors = if let Some(arr) = compound.get_int_array("colors") {
            arr.to_vec()
        } else if let Some(l) = compound.get_list("colors") {
            l.iter().filter_map(NbtTag::extract_int).collect()
        } else {
            Vec::new()
        };
        Some(Self {
            floats,
            flags,
            strings,
            colors,
        })
    }
}
impl DataComponentImpl for CustomModelDataImpl {
    fn write_data(&self) -> NbtTag {
        let mut compound = NbtCompound::new();
        compound.put_list(
            "floats",
            self.floats.iter().map(|f| NbtTag::Float(*f)).collect(),
        );
        compound.put_list(
            "flags",
            self.flags.iter().map(|b| NbtTag::Byte(*b as i8)).collect(),
        );
        compound.put_list(
            "strings",
            self.strings
                .iter()
                .map(|s| NbtTag::String(s.clone().into()))
                .collect(),
        );
        compound.put_list(
            "colors",
            self.colors.iter().map(|c| NbtTag::Int(*c)).collect(),
        );
        NbtTag::Compound(compound)
    }
    default_impl!(CustomModelData);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TooltipDisplayImpl;
impl TooltipDisplayImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for TooltipDisplayImpl {
    default_impl!(TooltipDisplay);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CreativeSlotLockImpl;
impl CreativeSlotLockImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for CreativeSlotLockImpl {
    default_impl!(CreativeSlotLock);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct EnchantmentGlintOverrideImpl;
impl EnchantmentGlintOverrideImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for EnchantmentGlintOverrideImpl {
    default_impl!(EnchantmentGlintOverride);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TooltipStyleImpl {
    pub id: String,
}
impl TooltipStyleImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_string().map(|id| Self { id: id.to_string() })
    }
}
impl DataComponentImpl for TooltipStyleImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::String(self.id.clone().into())
    }
    fn get_hash(&self) -> i32 {
        get_str_hash(&self.id) as i32
    }
    default_impl!(TooltipStyle);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct NoteBlockSoundImpl {
    pub sound: String,
}
impl NoteBlockSoundImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_string().map(|sound| Self {
            sound: sound.to_string(),
        })
    }
}
impl DataComponentImpl for NoteBlockSoundImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::String(self.sound.clone().into())
    }
    fn get_hash(&self) -> i32 {
        get_str_hash(&self.sound) as i32
    }
    default_impl!(NoteBlockSound);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BaseColorImpl {
    pub color: String,
}
impl BaseColorImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_string().map(|color| Self {
            color: color.to_string(),
        })
    }
}
impl DataComponentImpl for BaseColorImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::String(self.color.clone().into())
    }
    fn get_hash(&self) -> i32 {
        get_str_hash(&self.color) as i32
    }
    default_impl!(BaseColor);
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstrumentValue {
    Reference(Cow<'static, str>),
    Inline(NbtTag),
}
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentImpl {
    pub instrument: InstrumentValue,
}
#[derive(Clone, Debug)]
pub struct InstrumentPlayback {
    pub sound: crate::data_component_impl::IdOr<SoundEvent>,
    pub range: f32,
    pub duration_ticks: i32,
}
impl InstrumentImpl {
    pub fn playback(&self) -> Option<InstrumentPlayback> {
        let definition = self.definition()?;
        let definition = definition.extract_compound()?;
        let float = |value: &NbtTag| match value {
            NbtTag::Float(v) => Some(*v),
            NbtTag::Double(v) => Some(*v as f32),
            NbtTag::Int(v) => Some(*v as f32),
            NbtTag::Byte(v) => Some(f32::from(*v)),
            NbtTag::Short(v) => Some(f32::from(*v)),
            NbtTag::Long(v) => Some(*v as f32),
            _ => None,
        };
        let sound = match definition.get("sound_event")? {
            NbtTag::String(name) => crate::data_component_impl::IdOr::Id(
                crate::sound::Sound::from_name(name.strip_prefix("minecraft:").unwrap_or(name))?,
            ),
            NbtTag::Compound(sound) => crate::data_component_impl::IdOr::Value(SoundEvent::new(
                sound.get_string("sound_id")?.to_owned(),
                sound.get("range").and_then(float),
            )),
            _ => return None,
        };
        Some(InstrumentPlayback {
            sound,
            range: float(definition.get("range")?)?,
            duration_ticks: (float(definition.get("use_duration")?)? * 20.0).floor() as i32,
        })
    }
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        Some(Self {
            instrument: match data {
                NbtTag::String(name) => InstrumentValue::Reference(if name.contains(':') {
                    name.to_string().into()
                } else {
                    format!("minecraft:{name}").into()
                }),
                NbtTag::Compound(_) => InstrumentValue::Inline(data.clone()),
                _ => return None,
            },
        })
    }
    pub fn definition(&self) -> Option<NbtTag> {
        match &self.instrument {
            InstrumentValue::Reference(name) => {
                crate::registry_reference::definition("instrument", name)
            }
            InstrumentValue::Inline(data) => Some(data.clone()),
        }
    }
}
impl DataComponentImpl for InstrumentImpl {
    fn get_hash(&self) -> i32 {
        use crate::component_hash;
        use crate::data_component_impl::{get_f32_hash, get_str_hash};
        let identifier = |name: &str| {
            get_str_hash(&if name.contains(':') {
                name.to_owned()
            } else {
                format!("minecraft:{name}")
            })
        };
        let InstrumentValue::Inline(data) = &self.instrument else {
            if let InstrumentValue::Reference(name) = &self.instrument {
                return identifier(name) as i32;
            }
            unreachable!();
        };
        let Some(data) = data.extract_compound() else {
            return 0;
        };
        let float = |v: &NbtTag| match v {
            NbtTag::Float(v) => Some(*v),
            NbtTag::Double(v) => Some(*v as f32),
            NbtTag::Int(v) => Some(*v as f32),
            _ => None,
        };
        let Some(duration) = data.get("use_duration").and_then(float) else {
            return 0;
        };
        let Some(range) = data.get("range").and_then(float) else {
            return 0;
        };
        let Some(description) = data.get("description") else {
            return 0;
        };
        let sound = match data.get("sound_event") {
            Some(NbtTag::String(name)) => identifier(name),
            Some(NbtTag::Compound(value)) => {
                let Some(name) = value.get_string("sound_id") else {
                    return 0;
                };
                let mut entries = vec![(get_str_hash("sound_id"), identifier(name))];
                if let Some(range) = value.get("range").and_then(float) {
                    entries.push((get_str_hash("range"), get_f32_hash(range)));
                }
                component_hash::map(entries)
            }
            _ => return 0,
        };
        component_hash::map(vec![
            (get_str_hash("sound_event"), sound),
            (get_str_hash("use_duration"), get_f32_hash(duration)),
            (get_str_hash("range"), get_f32_hash(range)),
            (
                get_str_hash("description"),
                component_hash::text(&TextComponent::from_nbt(description)),
            ),
        ]) as i32
    }
    fn write_data(&self) -> NbtTag {
        match &self.instrument {
            InstrumentValue::Reference(name) => NbtTag::String(if name.contains(':') {
                name.to_string().into()
            } else {
                format!("minecraft:{name}").into()
            }),
            InstrumentValue::Inline(data) => data.clone(),
        }
    }
    default_impl!(Instrument);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ProvidesTrimMaterialImpl;
impl ProvidesTrimMaterialImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for ProvidesTrimMaterialImpl {
    default_impl!(ProvidesTrimMaterial);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ProvidesBannerPatternsImpl;
impl ProvidesBannerPatternsImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for ProvidesBannerPatternsImpl {
    default_impl!(ProvidesBannerPatterns);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BannerPatternLayer {
    pub pattern: String,
    pub color: crate::dye_color::DyeColor,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct BannerPatternsImpl {
    pub layers: Vec<BannerPatternLayer>,
}

impl BannerPatternsImpl {
    pub const EMPTY: Self = Self { layers: Vec::new() };

    #[must_use]
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        let mut layers = Vec::new();
        if let NbtTag::List(list) = data {
            for tag in list {
                if let Some(compound) = tag.extract_compound() {
                    let pattern = compound.get_string("pattern")?.to_string();
                    let color_str = compound.get_string("color")?;
                    let color = crate::dye_color::DyeColor::by_name(color_str).unwrap_or_default();
                    layers.push(BannerPatternLayer { pattern, color });
                }
            }
        }
        Some(Self { layers })
    }
}

impl DataComponentImpl for BannerPatternsImpl {
    fn write_data(&self) -> NbtTag {
        let mut list = Vec::new();
        for layer in &self.layers {
            let mut compound = NbtCompound::new();
            compound.put_string("pattern", layer.pattern.clone());
            compound.put_string("color", layer.color.name().to_string());
            list.push(NbtTag::Compound(compound));
        }
        NbtTag::List(list)
    }

    default_impl!(BannerPatterns);
}

/// Back, left, right and front decorations, with brick representing an undecorated side.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PotDecorationsImpl {
    pub sherds: [u16; 4],
}
impl PotDecorationsImpl {
    pub const EMPTY: Self = Self {
        sherds: [crate::item::Item::BRICK.id; 4],
    };

    pub fn read_data(data: &NbtTag) -> Option<Self> {
        let NbtTag::List(items) = data else {
            return None;
        };
        if items.len() > 4 {
            return None;
        }
        let mut result = Self::EMPTY;
        for (slot, item) in items.iter().enumerate() {
            let NbtTag::String(name) = item else {
                return None;
            };
            result.sherds[slot] = crate::item::Item::from_registry_key(name)?.id;
        }
        Some(result)
    }
}
impl DataComponentImpl for PotDecorationsImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::List(
            self.sherds
                .iter()
                .map(|id| {
                    let item = crate::item::Item::from_id(*id).unwrap_or(&crate::item::Item::BRICK);
                    NbtTag::String(item.registry_key.into())
                })
                .collect(),
        )
    }
    default_impl!(PotDecorations);
}

/// The lock's saved item predicate. The engine evaluates this shared representation
/// when opening containers; it is a persistent-only component in Java 26.2.
#[derive(Clone, Debug, PartialEq)]
pub struct LockImpl {
    pub predicate: NbtCompound,
}
impl LockImpl {
    pub fn read_data(data: &NbtTag) -> Option<Self> {
        data.extract_compound().map(|predicate| Self {
            predicate: predicate.clone(),
        })
    }
}
impl DataComponentImpl for LockImpl {
    fn write_data(&self) -> NbtTag {
        NbtTag::Compound(self.predicate.clone())
    }
    default_impl!(Lock);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BreakSoundImpl;
impl BreakSoundImpl {
    pub const fn read_data(_data: &NbtTag) -> Option<Self> {
        Some(Self)
    }
}
impl DataComponentImpl for BreakSoundImpl {
    default_impl!(BreakSound);
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundEvent {
    pub sound_name: String,
    pub range: Option<f32>,
}
impl std::hash::Hash for SoundEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.sound_name.hash(state);
        if let Some(val) = self.range {
            true.hash(state);
            unsafe { (*(&raw const val).cast::<u32>()).hash(state) };
        } else {
            false.hash(state);
        }
    }
}
impl SoundEvent {
    pub const fn new(sound_name: String, range: Option<f32>) -> Self {
        Self { sound_name, range }
    }
}
