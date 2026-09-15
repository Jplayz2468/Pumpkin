use super::is_exception_for_connection;
use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use crate::world::World;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, HorizontalFacingExt, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;

type GlassPaneProperties = pumpkin_data::block_properties::OakFenceLikeProperties;

#[pumpkin_block_from_tag("c:glass_panes")]
pub struct GlassPaneBlock;

impl BlockBehaviour for GlassPaneBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut pane_props = GlassPaneProperties::default(args.block);
        pane_props.waterlogged = args.replacing.water_source();

        compute_pane_state(pane_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let pane_props = GlassPaneProperties::from_state_id(args.state_id);
        super::schedule_waterlogged_tick(args.world, args.position, pane_props.waterlogged);
        compute_pane_state(pane_props, args.world, args.block, args.position)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

pub fn compute_pane_state(
    mut pane_props: GlassPaneProperties,
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
) -> BlockStateId {
    for direction in BlockDirection::horizontal() {
        let other_block_pos = block_pos.offset(direction.to_offset());
        let (other_block, other_block_state) = world.get_block_and_state(&other_block_pos);

        let connected = is_connected(
            block,
            direction.to_block_direction(),
            other_block,
            other_block_state,
        );

        match direction {
            HorizontalFacing::North => pane_props.north = connected,
            HorizontalFacing::South => pane_props.south = connected,
            HorizontalFacing::West => pane_props.west = connected,
            HorizontalFacing::East => pane_props.east = connected,
        }
    }

    pane_props.to_state_id(block)
}

/// Whether a pane placed at the origin connects towards `other_block` in the direction
/// `towards` (`towards` points from the pane to the neighbour).
///
/// Vanilla: `IronBarsBlock.attachsTo` (`IronBarsBlock.java:101-103`) is
/// `!isExceptionForConnection(state) && faceSolid || instanceof IronBarsBlock || WALLS tag`.
/// `c:glass_panes` + `IRON_BARS` together stand in for `instanceof IronBarsBlock`
/// (every vanilla pane, colored or not, extends it, per `StainedGlassPaneBlock.java:8`).
/// The exception list (`Block.java:251-259`) matters: without it a pane would visually
/// connect to pumpkins, melons, leaves, barriers and shulker boxes just because those
/// happen to have a sturdy face.
fn is_connected(
    pane_block: &Block,
    towards: BlockDirection,
    other_block: &Block,
    other_block_state: &BlockState,
) -> bool {
    other_block == pane_block
        || (!is_exception_for_connection(other_block)
            && other_block_state.is_side_solid(towards.opposite()))
        || other_block.has_tag(&tag::Block::C_GLASS_PANES)
        // `instanceof IronBarsBlock` (IronBarsBlock.java:102) also covers every copper-bars
        // weathering stage, waxed or not (Blocks.java:2340-2346) -- `minecraft:bars` is the
        // tag equivalent of that instanceof check, not just the plain `minecraft:iron_bars` id.
        || other_block.has_tag(&tag::Block::MINECRAFT_BARS)
        || other_block.has_tag(&tag::Block::MINECRAFT_WALLS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two panes side by side always connect (`instanceof IronBarsBlock` in vanilla,
    /// IronBarsBlock.java:102) regardless of face sturdiness.
    #[test]
    fn panes_connect_to_each_other_and_to_iron_bars() {
        for pane in [&Block::GLASS_PANE, &Block::WHITE_STAINED_GLASS_PANE] {
            assert!(is_connected(
                pane,
                BlockDirection::North,
                &Block::WHITE_STAINED_GLASS_PANE,
                Block::WHITE_STAINED_GLASS_PANE.default_state,
            ));
            assert!(is_connected(
                pane,
                BlockDirection::North,
                &Block::GLASS_PANE,
                Block::GLASS_PANE.default_state,
            ));
            assert!(is_connected(
                pane,
                BlockDirection::North,
                &Block::IRON_BARS,
                Block::IRON_BARS.default_state,
            ));
        }
    }

    /// Walls are an explicit `state.is(BlockTags.WALLS)` case (IronBarsBlock.java:102),
    /// independent of face sturdiness.
    #[test]
    fn panes_connect_to_walls() {
        assert!(is_connected(
            &Block::WHITE_STAINED_GLASS_PANE,
            BlockDirection::North,
            &Block::COBBLESTONE_WALL,
            Block::COBBLESTONE_WALL.default_state,
        ));
    }

    /// A plain solid block (stone) has a sturdy face and is not in the exception list,
    /// so `faceSolid` alone is enough to connect (IronBarsBlock.java:102).
    #[test]
    fn panes_connect_to_a_solid_block() {
        assert!(is_connected(
            &Block::WHITE_STAINED_GLASS_PANE,
            BlockDirection::North,
            &Block::STONE,
            Block::STONE.default_state,
        ));
    }

    /// Air is neither solid nor tagged/typed as anything a pane connects to.
    #[test]
    fn panes_do_not_connect_to_air() {
        assert!(!is_connected(
            &Block::WHITE_STAINED_GLASS_PANE,
            BlockDirection::North,
            &Block::AIR,
            Block::AIR.default_state,
        ));
    }

    /// `Block.isExceptionForConnection` (Block.java:251-259) explicitly excludes leaves,
    /// pumpkins, melons, barriers and shulker boxes even though several of them have a
    /// sturdy face on every side. This is the bug this change fixes: previously the pane
    /// would connect to all of these because it only checked `is_side_solid`.
    #[test]
    fn panes_do_not_connect_to_exception_blocks_despite_sturdy_faces() {
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
                    &Block::WHITE_STAINED_GLASS_PANE,
                    BlockDirection::North,
                    exception,
                    exception.default_state,
                ),
                "pane should not connect to {}",
                exception.name
            );
        }
    }
}
