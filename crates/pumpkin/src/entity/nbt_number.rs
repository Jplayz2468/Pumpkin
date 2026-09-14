//! NbtOps numeric coercion used by persistent attribute double fields.
use pumpkin_nbt::tag::NbtTag;
pub fn double(tag: &NbtTag) -> Option<f64> {
    Some(match tag {
        NbtTag::Byte(v) => f64::from(*v),
        NbtTag::Short(v) => f64::from(*v),
        NbtTag::Int(v) => f64::from(*v),
        NbtTag::Long(v) => *v as f64,
        NbtTag::Float(v) => f64::from(*v),
        NbtTag::Double(v) => *v,
        _ => return None,
    })
}
