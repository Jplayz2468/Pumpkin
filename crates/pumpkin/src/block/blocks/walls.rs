use super::is_exception_for_connection;
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
type WallProperties = pumpkin_data::block_properties::ResinBrickWallLikeProperties;

#[pumpkin_block_from_tag("minecraft:walls")]
pub struct WallBlock;

impl BlockBehaviour for WallBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut wall_props = WallProperties::default(args.block);
        wall_props.waterlogged =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position))
                .0
                .matches_type(&pumpkin_data::fluid::Fluid::WATER);

        compute_wall_state(wall_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut wall_props = WallProperties::from_state_id(args.state_id);
        super::schedule_waterlogged_tick(args.world, args.position, wall_props.waterlogged);
        if args.direction == BlockDirection::Down {
            return args.state_id;
        }
        if let Some(direction) = args.direction.to_horizontal_facing() {
            let (neighbor, state) = args
                .world
                .get_block_and_state(&args.position.offset(args.direction.to_offset()));
            let side = if is_connected(args.block, direction, neighbor, state) {
                WallShape::Low
            } else {
                WallShape::None
            };
            set_side(&mut wall_props, direction, side);
        }
        update_shape(wall_props, args.world, args.block, args.position)
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
    for direction in HorizontalFacing::all() {
        let (neighbor, state) = world.get_block_and_state(&block_pos.offset(direction.to_offset()));
        let side = if is_connected(block, direction, neighbor, state) {
            WallShape::Low
        } else {
            WallShape::None
        };
        set_side(&mut wall_props, direction, side);
    }
    update_shape(wall_props, world, block, block_pos)
}

fn set_side(props: &mut WallProperties, direction: HorizontalFacing, side: WallShape) {
    match direction {
        HorizontalFacing::North => props.north = side.into(),
        HorizontalFacing::South => props.south = side.into(),
        HorizontalFacing::East => props.east = side.into(),
        HorizontalFacing::West => props.west = side.into(),
    }
}

fn update_shape(
    mut props: WallProperties,
    world: &World,
    block: &Block,
    pos: &BlockPos,
) -> BlockStateId {
    let above_pos = pos.up();
    let (above, state) = world.get_block_and_state(&above_pos);
    let covered = |region: [f64; 4]| {
        crate::block::shape::collision_face_covers(
            state,
            above_pos,
            BlockDirection::Down,
            region.map(|v| v / 16.0),
        )
    };
    // WallBlock.TEST_SHAPES_WALL: two-pixel-wide strips reaching from the
    // outer edge through the center. The actual bottom collision face determines
    // height, including slabs, stairs, fences, panes, gates and unusual shapes.
    let connections = [
        (
            HorizontalFacing::North,
            props.north != NorthWall::None,
            [7.0, 9.0, 0.0, 9.0],
        ),
        (
            HorizontalFacing::East,
            props.east != EastWall::None,
            [7.0, 16.0, 7.0, 9.0],
        ),
        (
            HorizontalFacing::South,
            props.south != SouthWall::None,
            [7.0, 9.0, 7.0, 16.0],
        ),
        (
            HorizontalFacing::West,
            props.west != WestWall::None,
            [0.0, 9.0, 7.0, 9.0],
        ),
    ];
    for (direction, connected, region) in connections {
        set_side(
            &mut props,
            direction,
            if !connected {
                WallShape::None
            } else if covered(region) {
                WallShape::Tall
            } else {
                WallShape::Low
            },
        );
    }
    let north_none = props.north == NorthWall::None;
    let south_none = props.south == SouthWall::None;
    let east_none = props.east == EastWall::None;
    let west_none = props.west == WestWall::None;
    let corner = (north_none && south_none && east_none && west_none)
        || north_none != south_none
        || east_none != west_none;
    let high_wall = (props.north == NorthWall::Tall && props.south == SouthWall::Tall)
        || (props.east == EastWall::Tall && props.west == WestWall::Tall);
    // A wall post above wins before the opposite-tall-sides suppression.
    props.up = if (above.has_tag(&tag::Block::MINECRAFT_WALLS)
        && WallProperties::from_state_id(state.id).up)
        || corner
    {
        true
    } else if high_wall {
        false
    } else {
        above.has_tag(&tag::Block::MINECRAFT_WALL_POST_OVERRIDE) || covered([7.0, 9.0, 7.0, 9.0])
    };
    props.to_state_id(block)
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
    // The bars and panes tags cover IronBarsBlock and its copper subclasses,
    // independently of the sturdy-face exception check.
    let face_solid = other_block_state.is_side_solid(BlockDirection::from_cardinal_direction(
        direction.opposite(),
    ));

    let mut connected = other_block == block
        || other_block.has_tag(&tag::Block::MINECRAFT_WALLS)
        || (!is_exception_for_connection(other_block) && face_solid)
        || other_block.has_tag(&tag::Block::MINECRAFT_BARS)
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
