//! NBT boundary for the Warden's lenient anger codec.
use super::warden_anger::AngerManagement;
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};

fn number(tag: &NbtTag) -> Option<i32> {
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
fn uuid(tag: &NbtTag) -> Option<u128> {
    let values = match tag {
        NbtTag::IntArray(v) => v.clone(),
        NbtTag::ByteArray(v) => v.iter().map(|v| i32::from(*v)).collect(),
        NbtTag::LongArray(v) => v.iter().map(|v| *v as i32).collect(),
        NbtTag::List(v) => v.iter().map(number).collect::<Option<Vec<_>>>()?,
        _ => return None,
    };
    if values.len() != 4 {
        return None;
    }
    Some(
        values
            .into_iter()
            .fold(0, |result, word| (result << 32) | u128::from(word as u32)),
    )
}
pub fn read(nbt: Option<&NbtCompound>, delay: i32) -> AngerManagement {
    let pairs = || {
        let NbtTag::List(list) = nbt?.get("suspects")? else {
            return None;
        };
        list.iter()
            .map(|entry| {
                let NbtTag::Compound(entry) = entry else {
                    return None;
                };
                let id = uuid(entry.get("uuid")?)?;
                let anger = number(entry.get("anger")?)?;
                (anger >= 0).then_some((id, anger))
            })
            .collect::<Option<Vec<_>>>()
    };
    // lenientOptionalFieldOf discards the complete malformed list, not merely
    // the bad entry. Duplicate UUIDs keep the last value in the Java hash table.
    AngerManagement::new(delay, &pairs().unwrap_or_default())
}
pub fn write(state: &AngerManagement) -> Option<NbtCompound> {
    let mut pairs = state
        .suspects
        .iter()
        .map(|s| (s.uuid, state.anger(Some(s.id))))
        .collect::<Vec<_>>();
    // Java serializes through the entry-set spliterator (ascending slots),
    // while resolution/decay use the iterator (descending slots).
    pairs.extend(state.saved().into_iter().rev());
    if pairs.iter().any(|(_, anger)| *anger < 0) {
        return None;
    }
    let mut nbt = NbtCompound::new();
    if !pairs.is_empty() {
        nbt.put(
            "suspects",
            NbtTag::List(
                pairs
                    .into_iter()
                    .map(|(id, anger)| {
                        let mut entry = NbtCompound::new();
                        entry.put(
                            "uuid",
                            NbtTag::IntArray(
                                [96, 64, 32, 0].map(|shift| (id >> shift) as i32).to_vec(),
                            ),
                        );
                        entry.put_int("anger", anger);
                        NbtTag::Compound(entry)
                    })
                    .collect(),
            ),
        );
    }
    Some(nbt)
}
