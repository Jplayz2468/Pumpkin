//! Entity.load's Vec3 codec and per-axis motion limits.
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::vector3::Vector3;

pub fn read(nbt: &NbtCompound) -> Vector3<f64> {
    // ValueInput retains partial codec results: extra entries are truncated and
    // nonnumeric list entries are omitted before checking the vector's size.
    let values: Vec<f64> = match nbt.get("Motion") {
        Some(NbtTag::List(values)) => values
            .iter()
            .filter_map(super::nbt_number::double)
            .take(3)
            .collect(),
        Some(NbtTag::ByteArray(values)) => values.iter().take(3).map(|v| f64::from(*v)).collect(),
        Some(NbtTag::IntArray(values)) => values.iter().take(3).map(|v| f64::from(*v)).collect(),
        Some(NbtTag::LongArray(values)) => values.iter().take(3).map(|v| *v as f64).collect(),
        _ => return Vector3::default(),
    };
    let [x, y, z] = values.as_slice() else {
        return Vector3::default();
    };
    let limit = |v: f64| if v.abs() > 10.0 { 0.0 } else { v };
    Vector3::new(limit(*x), limit(*y), limit(*z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_saved_motion_codec_and_limits() {
        let cases: Vec<(String, [String; 3])> =
            serde_json::from_str(include_str!("saved_motion_cases.json")).unwrap();
        for (index, (input, bits)) in cases.into_iter().enumerate() {
            let mut reader = crate::command::string_reader::StringReader::new(input);
            let NbtTag::Compound(input) =
                crate::command::snbt::SnbtParser::parse_for_commands(&mut reader).unwrap()
            else {
                panic!("expected compound")
            };
            let motion = read(&input);
            assert_eq!(
                [motion.x.to_bits(), motion.y.to_bits(), motion.z.to_bits()],
                bits.map(|value| value.parse::<u64>().unwrap()),
                "case {index}: {input:?}"
            );
        }
    }
}
