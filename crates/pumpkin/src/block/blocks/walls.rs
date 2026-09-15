use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use crate::world::World;
use pumpkin_data::block_properties::EastWall;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::block_properties::NorthWall;
use pumpkin_data::block_properties::SouthWall;
use pumpkin_data::block_properties::WestWall;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;

type FenceGateProperties = pumpkin_data::block_properties::OakFenceGateLikeProperties;
type FenceLikeProperties = pumpkin_data::block_properties::OakFenceLikeProperties;
type WallProperties = pumpkin_data::block_properties::ResinBrickWallLikeProperties;

#[pumpkin_block_from_tag("minecraft:walls")]
pub struct WallBlock;

impl BlockBehaviour for WallBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut wall_props = WallProperties::default(args.block);
        wall_props.waterlogged = args.replacing.water_source();

        compute_wall_state(wall_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let wall_props = WallProperties::from_state_id(args.state_id);
        compute_wall_state(wall_props, args.world, args.block, args.position)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

pub fn compute_wall_state(
    mut wall_props: WallProperties,
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
) -> BlockStateId {
    let (block_above, block_above_state) = world.get_block_and_state(&block_pos.up());

    for direction in HorizontalFacing::all() {
        let other_block_pos = block_pos.offset(direction.to_offset());
        let (other_block, other_block_state) = world.get_block_and_state(&other_block_pos);

        let connected = is_connected(block, direction, other_block, other_block_state);

        let shape = if connected {
            let raise = if block_above_state.is_full_cube() {
                true
            } else if block_above.has_tag(&tag::Block::MINECRAFT_WALLS) {
                let other_props = WallProperties::from_state_id(block_above_state.id);
                match direction {
                    HorizontalFacing::North => other_props.north != NorthWall::None,
                    HorizontalFacing::South => other_props.south != SouthWall::None,
                    HorizontalFacing::East => other_props.east != EastWall::None,
                    HorizontalFacing::West => other_props.west != WestWall::None,
                }
            } else if block_above.has_tag(&tag::Block::C_GLASS_PANES)
                || block_above.has_tag(&tag::Block::MINECRAFT_FENCES)
                || block_above == &Block::IRON_BARS
            {
                let other_props = FenceLikeProperties::from_state_id(block_above_state.id);
                match direction {
                    HorizontalFacing::North => other_props.north,
                    HorizontalFacing::South => other_props.south,
                    HorizontalFacing::East => other_props.east,
                    HorizontalFacing::West => other_props.west,
                }
            } else if block_above.has_tag(&tag::Block::MINECRAFT_FENCE_GATES) {
                let other_props = FenceGateProperties::from_state_id(block_above_state.id);
                // gate is perp to connected direction
                let perpendicular_gate = direction == other_props.facing.rotate_clockwise()
                    || direction == other_props.facing.rotate_counter_clockwise();

                perpendicular_gate && !other_props.open
            } else {
                false
            };
            if raise {
                WallShape::Tall
            } else {
                WallShape::Low
            }
        } else {
            WallShape::None
        };

        match direction {
            HorizontalFacing::North => wall_props.north = shape.into(),
            HorizontalFacing::South => wall_props.south = shape.into(),
            HorizontalFacing::East => wall_props.east = shape.into(),
            HorizontalFacing::West => wall_props.west = shape.into(),
        }
    }

    let connected_north_south = wall_props.north != NorthWall::None
        && wall_props.south != SouthWall::None
        && wall_props.east == EastWall::None
        && wall_props.west == WestWall::None;
    let connected_east_west = wall_props.north == NorthWall::None
        && wall_props.south == SouthWall::None
        && wall_props.east != EastWall::None
        && wall_props.west != WestWall::None;
    let cross = wall_props.north != NorthWall::None
        && wall_props.south != SouthWall::None
        && wall_props.east != EastWall::None
        && wall_props.west != WestWall::None;

    // Vanilla: WallBlock.shouldRaisePost (WallBlock.java:210-231). After the corner check
    // above (`hasCorner`, ported as `!(cross || connected_north_south || connected_east_west)`),
    // vanilla checks `hasHighWall`: two opposite TALL sides already reach full height, so no
    // center post is needed regardless of what's above.
    let has_high_wall = (wall_props.north == NorthWall::Tall && wall_props.south == SouthWall::Tall)
        || (wall_props.east == EastWall::Tall && wall_props.west == WestWall::Tall);

    wall_props.up = if !(cross || connected_north_south || connected_east_west) {
        true
    } else if has_high_wall {
        false
    } else if block_above.has_tag(&tag::Block::MINECRAFT_WALLS) {
        let other_props = WallProperties::from_state_id(block_above_state.id);
        other_props.up
    } else if block_above.has_tag(&tag::Block::MINECRAFT_FENCE_GATES) {
        let other_props = FenceGateProperties::from_state_id(block_above_state.id);
        if other_props.open {
            false
        } else {
            match other_props.facing {
                HorizontalFacing::East | HorizontalFacing::West => connected_east_west,
                HorizontalFacing::South | HorizontalFacing::North => connected_north_south,
            }
        }
    } else if block_above.has_tag(&tag::Block::MINECRAFT_WALL_POST_OVERRIDE) {
        // Vanilla: `topNeighbour.is(BlockTags.WALL_POST_OVERRIDE)` (WallBlock.java:230),
        // e.g. torches and signs force a raised post regardless of their shape.
        true
    } else {
        false
    };
    wall_props.to_state_id(block)
}

fn is_connected(
    block: &Block,
    direction: HorizontalFacing,
    other_block: &Block,
    other_block_state: &BlockState,
) -> bool {
    // Vanilla: WallBlock.connectsTo (WallBlock.java:105-109) is
    // `state.is(BlockTags.WALLS) || !isExceptionForConnection(state) && faceSolid ||
    // block instanceof IronBarsBlock || connectedFenceGate`. The exception check only
    // gates the `faceSolid` branch: without it a wall would connect to pumpkins, melons,
    // leaves, barriers and shulker boxes just because those happen to be full/sturdy-faced.
    // `other_block == &Block::IRON_BARS || other_block.has_tag(C_GLASS_PANES)` stands in for
    // `instanceof IronBarsBlock`, since every vanilla pane (colored or not) is literally an
    // `IronBarsBlock` instance (see the note in `glass_panes.rs`), and is intentionally left
    // outside the exception check, matching vanilla.
    let face_solid = (other_block_state.is_solid() && other_block_state.is_full_cube())
        || other_block_state.is_side_solid(BlockDirection::from_cardinal_direction(
            direction.opposite(),
        ));

    let mut connected = other_block == block
        || other_block.has_tag(&tag::Block::MINECRAFT_WALLS)
        || (!is_exception_for_connection(other_block) && face_solid)
        || other_block == &Block::IRON_BARS
        || other_block.has_tag(&tag::Block::C_GLASS_PANES);

    // fence gates do not pass is_side_solid check
    if !connected && other_block.has_tag(&tag::Block::MINECRAFT_FENCE_GATES) {
        let fence_props = FenceGateProperties::from_state_id(other_block_state.id);
        if fence_props.facing == direction.rotate_clockwise()
            || fence_props.facing == direction.rotate_counter_clockwise()
        {
            connected = true;
        }
    }
    connected
}

/// `Block.isExceptionForConnection` (Block.java:251-259): these blocks are excluded from
/// the generic "sturdy face" connection rule used by panes, fences and walls even though
/// several of them (pumpkins, melons, leaves, barriers, closed shulker boxes) do have a
/// sturdy face on every side. Mirrors the helper of the same name in
/// `block/blocks/glass_panes.rs`; see that file's note about a shared helper.
fn is_exception_for_connection(block: &Block) -> bool {
    block.has_tag(&tag::Block::MINECRAFT_LEAVES)
        || block == &Block::BARRIER
        || block == &Block::CARVED_PUMPKIN
        || block == &Block::JACK_O_LANTERN
        || block == &Block::MELON
        || block == &Block::PUMPKIN
        || block.has_tag(&tag::Block::MINECRAFT_SHULKER_BOXES)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WallShape {
    None,
    Low,
    Tall,
}

impl From<WallShape> for NorthWall {
    fn from(value: WallShape) -> Self {
        match value {
            WallShape::None => Self::None,
            WallShape::Low => Self::Low,
            WallShape::Tall => Self::Tall,
        }
    }
}

impl From<WallShape> for SouthWall {
    fn from(value: WallShape) -> Self {
        match value {
            WallShape::None => Self::None,
            WallShape::Low => Self::Low,
            WallShape::Tall => Self::Tall,
        }
    }
}

impl From<WallShape> for EastWall {
    fn from(value: WallShape) -> Self {
        match value {
            WallShape::None => Self::None,
            WallShape::Low => Self::Low,
            WallShape::Tall => Self::Tall,
        }
    }
}

impl From<WallShape> for WestWall {
    fn from(value: WallShape) -> Self {
        match value {
            WallShape::None => Self::None,
            WallShape::Low => Self::Low,
            WallShape::Tall => Self::Tall,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two walls of any type connect to each other via the `minecraft:walls` tag
    /// (`state.is(BlockTags.WALLS)`, WallBlock.java:108), and iron bars via
    /// `instanceof IronBarsBlock`.
    #[test]
    fn walls_connect_to_each_other_and_to_iron_bars() {
        assert!(is_connected(
            &Block::COBBLESTONE_WALL,
            HorizontalFacing::North,
            &Block::COBBLESTONE_WALL,
            Block::COBBLESTONE_WALL.default_state,
        ));
        assert!(is_connected(
            &Block::COBBLESTONE_WALL,
            HorizontalFacing::North,
            &Block::IRON_BARS,
            Block::IRON_BARS.default_state,
        ));
        // Every vanilla glass pane is literally an `IronBarsBlock` instance
        // (`Blocks.GLASS_PANE = register(..., IronBarsBlock::new, ...)`).
        assert!(is_connected(
            &Block::COBBLESTONE_WALL,
            HorizontalFacing::North,
            &Block::GLASS_PANE,
            Block::GLASS_PANE.default_state,
        ));
    }

    /// A plain solid block (stone) has a sturdy face and is not in the exception list,
    /// so `faceSolid` alone is enough to connect (WallBlock.java:108).
    #[test]
    fn wall_connects_to_a_solid_block() {
        assert!(is_connected(
            &Block::COBBLESTONE_WALL,
            HorizontalFacing::North,
            &Block::STONE,
            Block::STONE.default_state,
        ));
    }

    /// Air is neither solid nor tagged/typed as anything a wall connects to.
    #[test]
    fn wall_does_not_connect_to_air() {
        assert!(!is_connected(
            &Block::COBBLESTONE_WALL,
            HorizontalFacing::North,
            &Block::AIR,
            Block::AIR.default_state,
        ));
    }

    /// `Block.isExceptionForConnection` (Block.java:251-259) explicitly excludes leaves,
    /// pumpkins, melons, barriers and shulker boxes even though several of them have a
    /// sturdy face on every side. This is the bug this change fixes: previously a wall
    /// would connect to all of these because it only checked full-cube/face-sturdy state.
    #[test]
    fn wall_does_not_connect_to_exception_blocks_despite_sturdy_faces() {
        for exception in [
            &Block::OAK_LEAVES,
            &Block::PUMPKIN,
            &Block::CARVED_PUMPKIN,
            &Block::JACK_O_LANTERN,
            &Block::MELON,
            &Block::BARRIER,
            &Block::SHULKER_BOX,
        ] {
            assert!(
                !is_connected(
                    &Block::COBBLESTONE_WALL,
                    HorizontalFacing::North,
                    exception,
                    exception.default_state,
                ),
                "wall should not connect to {}",
                exception.name
            );
        }
    }
}
