//! PrimedTnt's saved payload. Fuse narrowing happens on save, as in Java.
use pumpkin_data::{Block, BlockStateId};
use pumpkin_nbt::compound::NbtCompound;

#[derive(Clone, Copy)]
pub struct TntState {
    pub fuse: i32,
    pub power: f32,
    pub block: BlockStateId,
}

impl TntState {
    pub fn read(nbt: &NbtCompound) -> Self {
        let state = nbt.get_compound("block_state");
        let resolved = state
            .and_then(|tag| tag.get_string("Name"))
            .and_then(Block::from_name);
        let block = resolved.unwrap_or(&Block::TNT);
        let mut block_state = block.default_state.id;
        if resolved.is_some()
            && let Some(values) = state.and_then(|state| state.get_compound("Properties"))
            && let Some(properties) = block.properties(block_state)
        {
            let mut properties = properties.to_props();
            for index in 0..properties.len() {
                if let Some(value) = values.get_string(properties[index].0) {
                    let old = properties[index].1;
                    properties[index].1 = value;
                    if let Some(state) = block.state_from_properties(&properties) {
                        block_state = state.id;
                    } else {
                        properties[index].1 = old;
                    }
                }
            }
        }
        Self {
            fuse: i32::from(
                nbt.get("fuse")
                    .and_then(super::nbt_number::short)
                    .unwrap_or(80),
            ),
            power: nbt
                .get("explosion_power")
                .and_then(super::nbt_number::float)
                .unwrap_or(4.0)
                .clamp(0.0, 128.0),
            block: block_state,
        }
    }

    pub fn write(self, nbt: &mut NbtCompound) {
        nbt.put_short("fuse", self.fuse as i16);
        let block = self.block.to_block();
        let mut state = NbtCompound::new();
        state.put_string("Name", format!("minecraft:{}", block.name));
        if let Some(properties) = block.properties(self.block) {
            let mut values = NbtCompound::new();
            for (name, value) in properties.to_props() {
                values.put_string(name, value.to_string());
            }
            if !values.is_empty() {
                state.put_compound("Properties", values);
            }
        }
        nbt.put_compound("block_state", state);
        if self.power != 4.0 {
            nbt.put_float("explosion_power", self.power);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::entity_reference::EntityReference;
    use pumpkin_nbt::tag::NbtTag;

    fn compound(text: String) -> NbtCompound {
        let mut reader = crate::command::string_reader::StringReader::new(text);
        match crate::command::snbt::SnbtParser::parse_for_commands(&mut reader).unwrap() {
            NbtTag::Compound(value) => value,
            _ => panic!("expected compound"),
        }
    }

    #[test]
    fn java_tnt_saved_state_and_owner_codec() {
        let cases: Vec<[String; 2]> =
            serde_json::from_str(include_str!("tnt_state_cases.json")).unwrap();
        for (index, [input, expected]) in cases.into_iter().enumerate() {
            let input = compound(input);
            let mut output = NbtCompound::new();
            TntState::read(&input).write(&mut output);
            if let Some(owner) = EntityReference::<usize>::read(&input, "owner") {
                owner.write(&mut output, "owner");
            }
            assert_eq!(output, compound(expected), "case {index}: {input:?}");
        }
    }
}
