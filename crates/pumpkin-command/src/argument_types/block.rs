use crate::{
    argument_types::argument_type::{ArgumentType, JavaClientArgumentType},
    context::command_context::CommandContext,
    errors::command_syntax_error::CommandSyntaxError,
    errors::error_types::CommandErrorType,
    string_reader::StringReader,
    suggestion::suggestions::{Suggestions, SuggestionsBuilder},
};
use pumpkin_data::{Block, translation};
use pumpkin_util::text::TextComponent;

pub const INVALID_BLOCK_ERROR_TYPE: CommandErrorType<1> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_ID_INVALID,
    translation::java::ARGUMENT_BLOCK_ID_INVALID,
);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for BlockArgumentType {
    type Item = &'static Block;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let start = reader.cursor();
        while let Some(c) = reader.peek() {
            if c.is_alphanumeric() || c == '_' || c == ':' || c == '/' || c == '.' || c == '-' {
                reader.skip();
            } else {
                break;
            }
        }
        let block_name = &reader.string()[start..reader.cursor()];
        let normalized = if block_name.contains(':') {
            block_name.to_string()
        } else {
            format!("minecraft:{block_name}")
        };

        Block::from_name(&normalized)
            .ok_or_else(|| INVALID_BLOCK_ERROR_TYPE.create(reader, TextComponent::text(normalized)))
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::BlockState
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        builder.build()
    }
}

impl BlockArgumentType {
    pub fn get<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static Block, CommandSyntaxError> {
        context.get_argument::<&'static Block>(name).copied()
    }
}

/// A concrete placement state. Unspecified properties retain the block defaults.
/// Block-entity NBT and block/tag predicates have separate parsing paths.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockStateArgumentType;

const UNKNOWN_PROPERTY: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNKNOWN,
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNKNOWN,
);
const DUPLICATE_PROPERTY: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_DUPLICATE,
    translation::java::ARGUMENT_BLOCK_PROPERTY_DUPLICATE,
);
const INVALID_PROPERTY: CommandErrorType<3> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_INVALID,
    translation::java::ARGUMENT_BLOCK_PROPERTY_INVALID,
);
const MISSING_VALUE: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_NOVALUE,
    translation::java::ARGUMENT_BLOCK_PROPERTY_NOVALUE,
);
const UNCLOSED_PROPERTIES: CommandErrorType<0> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNCLOSED,
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNCLOSED,
);

impl<S: crate::source::CommandSource> ArgumentType<S> for BlockStateArgumentType {
    type Item = pumpkin_data::BlockStateId;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let block = ArgumentType::<S>::parse(&BlockArgumentType, reader)?;
        let mut state = block.default_state.id;
        if reader.peek() != Some('[') {
            return Ok(state);
        }
        let mut properties: Vec<(String, String)> = block
            .properties(state)
            .map(|properties| {
                properties
                    .to_props()
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let mut specified = std::collections::HashSet::new();
        let name = || TextComponent::text(format!("minecraft:{}", block.name));
        reader.skip();
        reader.skip_whitespace();
        while reader.can_read_char() && reader.peek() != Some(']') {
            reader.skip_whitespace();
            let key_start = reader.cursor();
            let key = reader.read_string()?;
            let Some(index) = properties.iter().position(|(property, _)| property == &key) else {
                reader.set_cursor(key_start);
                return Err(UNKNOWN_PROPERTY.create(reader, name(), TextComponent::text(key)));
            };
            if !specified.insert(key.clone()) {
                reader.set_cursor(key_start);
                return Err(DUPLICATE_PROPERTY.create(reader, name(), TextComponent::text(key)));
            }
            reader.skip_whitespace();
            if reader.peek() != Some('=') {
                return Err(MISSING_VALUE.create(reader, name(), TextComponent::text(key)));
            }
            reader.skip();
            reader.skip_whitespace();
            let value_start = reader.cursor();
            let value = reader.read_string()?;
            properties[index].1.clone_from(&value);
            let refs: Vec<_> = properties
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let Some(parsed) = block.state_from_properties(&refs) else {
                reader.set_cursor(value_start);
                return Err(INVALID_PROPERTY.create(
                    reader,
                    name(),
                    TextComponent::text(key),
                    TextComponent::text(value),
                ));
            };
            state = parsed.id;
            reader.skip_whitespace();
            match reader.peek() {
                Some(',') => {
                    reader.skip();
                }
                Some(']') => break,
                _ => return Err(UNCLOSED_PROPERTIES.create(reader)),
            }
        }
        if reader.peek() != Some(']') {
            return Err(UNCLOSED_PROPERTIES.create(reader));
        }
        reader.skip();
        Ok(state)
    }

    fn client_side_parser(&self) -> JavaClientArgumentType {
        JavaClientArgumentType::BlockState
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        builder.build()
    }
}

impl BlockStateArgumentType {
    pub fn get<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<pumpkin_data::BlockStateId, CommandSyntaxError> {
        context
            .get_argument::<pumpkin_data::BlockStateId>(name)
            .copied()
    }
}
