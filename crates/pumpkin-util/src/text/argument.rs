//! Translation arguments retain Java's primitive types, including NBT numeric widths.
use super::{TextComponent, TextComponentBase};
use crate::version::JavaMinecraftVersion;
use pumpkin_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextArgument {
    Component(TextComponentBase),
    Boolean(bool),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(u32),
    Double(u64),
}
impl<'de> Deserialize<'de> for TextArgument {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(match &v {
            serde_json::Value::Bool(v) => Self::Boolean(*v),
            serde_json::Value::Number(v) => {
                // JsonOps.convertTo chooses the narrowest exactly representable numeric type.
                let n = v.as_f64().unwrap();
                let integer = v.as_i64().or_else(|| {
                    (n.fract() == 0.0 && n >= i64::MIN as f64 && n < (i64::MAX as f64))
                        .then_some(n as i64)
                });
                if let Some(n) = integer {
                    if let Ok(n) = i8::try_from(n) {
                        Self::Byte(n)
                    } else if let Ok(n) = i16::try_from(n) {
                        Self::Short(n)
                    } else if let Ok(n) = i32::try_from(n) {
                        Self::Int(n)
                    } else {
                        Self::Long(n)
                    }
                } else if f64::from(n as f32) == n {
                    Self::Float((n as f32).to_bits())
                } else {
                    Self::Double(n.to_bits())
                }
            }
            _ => Self::Component(
                serde_json::from_value::<TextComponent>(v)
                    .map_err(serde::de::Error::custom)?
                    .0,
            ),
        })
    }
}
impl Serialize for TextArgument {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json_value_for_version(&JavaMinecraftVersion::V_26_2)
            .serialize(s)
    }
}
impl TextArgument {
    pub fn from_nbt(tag: &NbtTag) -> Self {
        match tag {
            NbtTag::Byte(v) => Self::Byte(*v),
            NbtTag::Short(v) => Self::Short(*v),
            NbtTag::Int(v) => Self::Int(*v),
            NbtTag::Long(v) => Self::Long(*v),
            NbtTag::Float(v) => Self::Float(v.to_bits()),
            NbtTag::Double(v) => Self::Double(v.to_bits()),
            _ => Self::Component(TextComponent::from_nbt(tag).0),
        }
    }
    pub fn to_nbt_tag_for_version(&self, version: &JavaMinecraftVersion) -> NbtTag {
        match self {
            Self::Component(v) => v.to_nbt_tag_for_version(version),
            Self::Boolean(v) => NbtTag::Byte(i8::from(*v)),
            Self::Byte(v) => NbtTag::Byte(*v),
            Self::Short(v) => NbtTag::Short(*v),
            Self::Int(v) => NbtTag::Int(*v),
            Self::Long(v) => NbtTag::Long(*v),
            Self::Float(v) => NbtTag::Float(f32::from_bits(*v)),
            Self::Double(v) => NbtTag::Double(f64::from_bits(*v)),
        }
    }
    pub fn to_json_value_for_version(&self, version: &JavaMinecraftVersion) -> serde_json::Value {
        match self {
            Self::Component(v) => v.to_json_value_for_version(version),
            Self::Boolean(v) => serde_json::json!(v),
            Self::Byte(v) => serde_json::json!(v),
            Self::Short(v) => serde_json::json!(v),
            Self::Int(v) => serde_json::json!(v),
            Self::Long(v) => serde_json::json!(v),
            Self::Float(v) => serde_json::from_str(&float_string(f32::from_bits(*v)))
                .unwrap_or(serde_json::Value::Null),
            Self::Double(v) => serde_json::json!(f64::from_bits(*v)),
        }
    }
    pub fn component(&self) -> TextComponentBase {
        let plain = match self {
            Self::Component(v) => return v.clone(),
            Self::Boolean(v) => v.to_string(),
            Self::Byte(v) => v.to_string(),
            Self::Short(v) => v.to_string(),
            Self::Int(v) => v.to_string(),
            Self::Long(v) => v.to_string(),
            Self::Float(v) => float_string(f32::from_bits(*v)),
            Self::Double(v) => float_string(f64::from_bits(*v)),
        };
        TextComponent::text(plain).0
    }
    pub fn to_bedrock_string(&self) -> String {
        self.component().to_bedrock_string()
    }
    pub fn to_translated(self) -> Self {
        match self {
            Self::Component(v) => Self::Component(v.to_translated()),
            v => v,
        }
    }
}
fn float_string<T: std::fmt::Debug + std::fmt::LowerExp + Copy + Into<f64>>(v: T) -> String {
    let number = v.into();
    if number.is_nan() {
        return "NaN".into();
    }
    if number == f64::INFINITY {
        return "Infinity".into();
    }
    if number == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if number != 0.0 && !(0.001..10_000_000.0).contains(&number.abs()) {
        let scientific = format!("{v:e}");
        let scientific = if scientific.split_once('e').unwrap().0.contains('.') {
            scientific
        } else {
            format!("{v:.1e}")
        };
        scientific.replace('e', "E")
    } else {
        format!("{v:?}")
    }
}

pub(super) fn list_to_nbt(args: &[TextArgument], version: &JavaMinecraftVersion) -> NbtTag {
    NbtTag::List(
        args.iter()
            .map(|v| v.to_nbt_tag_for_version(version))
            .collect(),
    )
}
pub(super) fn list_from_nbt(tag: &NbtTag) -> Option<Vec<TextArgument>> {
    Some(match tag {
        NbtTag::List(args) => args.iter().map(TextArgument::from_nbt).collect(),
        NbtTag::ByteArray(args) => args.iter().map(|v| TextArgument::Byte(*v as i8)).collect(),
        NbtTag::IntArray(args) => args.iter().map(|v| TextArgument::Int(*v)).collect(),
        NbtTag::LongArray(args) => args.iter().map(|v| TextArgument::Long(*v)).collect(),
        _ => return None,
    })
}
