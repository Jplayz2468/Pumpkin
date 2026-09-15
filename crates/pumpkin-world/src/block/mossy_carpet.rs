//! MossyCarpetBlock state rules shared by live blocks and feature placement.
use crate::world::BlockAccessor;
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_util::math::position::BlockPos;

const DIRECTIONS: [BlockDirection; 4] = [
    BlockDirection::North,
    BlockDirection::East,
    BlockDirection::South,
    BlockDirection::West,
];
const NAMES: [&str; 4] = ["north", "east", "south", "west"];
#[derive(Clone, Copy, PartialEq)]
enum Side {
    None,
    Low,
    Tall,
}
impl Side {
    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Tall => "tall",
        }
    }
}
struct State {
    bottom: bool,
    sides: [Side; 4],
}
impl State {
    fn read(state: BlockStateId) -> Self {
        let mut result = Self {
            bottom: true,
            sides: [Side::None; 4],
        };
        if let Some(props) = Block::PALE_MOSS_CARPET.properties(state) {
            for (key, value) in props.to_props() {
                if key == "bottom" {
                    result.bottom = value == "true";
                } else if let Some(index) = NAMES.iter().position(|name| *name == key) {
                    result.sides[index] = match value {
                        "low" => Side::Low,
                        "tall" => Side::Tall,
                        _ => Side::None,
                    };
                }
            }
        }
        result
    }
    fn id(&self) -> BlockStateId {
        Block::PALE_MOSS_CARPET
            .from_properties(&[
                ("bottom", if self.bottom { "true" } else { "false" }),
                ("north", self.sides[0].name()),
                ("east", self.sides[1].name()),
                ("south", self.sides[2].name()),
                ("west", self.sides[3].name()),
            ])
            .to_state_id(&Block::PALE_MOSS_CARPET)
    }
    fn has_faces(&self) -> bool {
        self.bottom || self.sides.iter().any(|side| *side != Side::None)
    }
}
pub fn is_base(state: BlockStateId) -> bool {
    State::read(state).bottom
}
pub fn can_survive(accessor: &dyn BlockAccessor, pos: &BlockPos, state: BlockStateId) -> bool {
    let below = accessor.get_block_state_id(&pos.down());
    if is_base(state) {
        !below.to_state().is_air()
    } else {
        below.to_block() == &Block::PALE_MOSS_CARPET && is_base(below)
    }
}
pub fn has_faces(state: BlockStateId) -> bool {
    State::read(state).has_faces()
}
pub fn updated_state(
    accessor: &dyn BlockAccessor,
    pos: &BlockPos,
    state: BlockStateId,
    create_sides: bool,
) -> BlockStateId {
    let mut props = State::read(state);
    let create_sides = create_sides || props.bottom;
    let above = accessor.get_block_state_id(&pos.up());
    let below = accessor.get_block_state_id(&pos.down());
    for (index, direction) in DIRECTIONS.into_iter().enumerate() {
        let neighbor_pos = pos.offset(direction.to_offset());
        let neighbor = accessor.get_block_state(&neighbor_pos);
        let face = direction.opposite();
        let supports = neighbor.is_side_solid(face)
            || neighbor.collision_face_covers(neighbor_pos, face, [0.0, 0.0, 1.0, 1.0]);
        let mut side = if supports {
            if create_sides {
                Side::Low
            } else {
                props.sides[index]
            }
        } else {
            Side::None
        };
        if side == Side::Low {
            if above.to_block() == &Block::PALE_MOSS_CARPET {
                let top = State::read(above);
                if !top.bottom && top.sides[index] != Side::None {
                    side = Side::Tall;
                }
            }
            if !props.bottom
                && below.to_block() == &Block::PALE_MOSS_CARPET
                && State::read(below).sides[index] == Side::None
            {
                side = Side::None;
            }
        }
        props.sides[index] = side;
    }
    props.id()
}
pub fn create_topper(
    accessor: &dyn BlockAccessor,
    pos: &BlockPos,
    mut keep_side: impl FnMut() -> bool,
) -> BlockStateId {
    let above_pos = pos.up();
    let previous = accessor.get_block_state_id(&above_pos);
    let moss = previous.to_block() == &Block::PALE_MOSS_CARPET;
    if (moss && is_base(previous)) || (!moss && !previous.to_state().replaceable()) {
        return BlockStateId::AIR;
    }
    let empty_top = State {
        bottom: false,
        sides: [Side::None; 4],
    }
    .id();
    let mut props = State::read(updated_state(accessor, &above_pos, empty_top, true));
    for side in &mut props.sides {
        if *side != Side::None && !keep_side() {
            *side = Side::None;
        }
    }
    let result = props.id();
    if props.has_faces() && result != previous {
        result
    } else {
        BlockStateId::AIR
    }
}
