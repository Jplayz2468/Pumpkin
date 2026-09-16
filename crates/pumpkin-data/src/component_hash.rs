//! CRC32C HashOps primitives used by Java's component codecs.
use crate::data_component_impl::{get_i32_hash, get_str_hash};
use crc_fast::{CrcAlgorithm::Crc32Iscsi, Digest};
use pumpkin_util::{text::TextComponent, version::JavaMinecraftVersion};
use pumpkin_util::serde_json::Value;

fn digest(bytes: &[u8]) -> u32 {
    let mut digest = Digest::new(Crc32Iscsi);
    digest.update(bytes);
    digest.finalize() as u32
}
pub fn map(mut entries: Vec<(u32, u32)>) -> u32 {
    entries.sort_unstable();
    let mut data = vec![2];
    for (key, value) in entries {
        data.extend(key.to_le_bytes());
        data.extend(value.to_le_bytes());
    }
    data.push(3);
    digest(&data)
}
pub fn list(values: impl IntoIterator<Item = u32>) -> u32 {
    let mut data = vec![4];
    for value in values {
        data.extend(value.to_le_bytes());
    }
    data.push(5);
    digest(&data)
}
pub fn json(value: &Value) -> u32 {
    match value {
        Value::Null => digest(&[1]),
        Value::Bool(v) => digest(&[13, u8::from(*v)]),
        Value::String(v) => get_str_hash(v),
        Value::Array(v) => list(v.iter().map(json)),
        Value::Object(v) => map(v.iter().map(|(k, v)| (get_str_hash(k), json(v))).collect()),
        Value::Number(v) => {
            if let Some(v) = v.as_i64().and_then(|v| i32::try_from(v).ok()) {
                get_i32_hash(v)
            } else {
                let mut data = vec![11];
                data.extend(v.as_f64().unwrap().to_le_bytes());
                digest(&data)
            }
        }
    }
}
pub fn text(value: &TextComponent) -> u32 {
    json(
        &value
            .0
            .to_json_value_for_version(&JavaMinecraftVersion::V_26_2),
    )
}
