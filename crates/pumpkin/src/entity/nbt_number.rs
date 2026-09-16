//! Java numeric-tag coercion for persistent entity fields.
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

pub fn float(tag: &NbtTag) -> Option<f32> {
    Some(match tag {
        NbtTag::Byte(v) => f32::from(*v),
        NbtTag::Short(v) => f32::from(*v),
        NbtTag::Int(v) => *v as f32,
        NbtTag::Long(v) => *v as f32,
        NbtTag::Float(v) => *v,
        NbtTag::Double(v) => *v as f32,
        _ => return None,
    })
}

pub fn short(tag: &NbtTag) -> Option<i16> {
    Some(match tag {
        NbtTag::Byte(v) => i16::from(*v),
        NbtTag::Short(v) => *v,
        NbtTag::Int(v) => *v as i16,
        NbtTag::Long(v) => *v as i16,
        // 26.2 Mth.floor uses Math.floor followed by a saturating Java int cast.
        NbtTag::Float(v) => v.floor() as i32 as i16,
        NbtTag::Double(v) => v.floor() as i32 as i16,
        _ => return None,
    })
}
