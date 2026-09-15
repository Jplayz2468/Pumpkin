use pumpkin_data::block_properties::BrownMushroomBlockLikeProperties;
use pumpkin_data::{Block, BlockDirection, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::block::{BlockBehaviour, BlockMetadata, GetStateForNeighborUpdateArgs, OnPlaceArgs};

/// `brown_mushroom_block`, `red_mushroom_block` and `mushroom_stem` each have six boolean
/// face properties (one per direction) that decide whether that face renders the cap/stem
/// texture. Vanilla hides a face only when the neighbour in that direction is the exact
/// same block (HugeMushroomBlock.java:70 checks `neighbourState.is(this)`, i.e. the same
/// `Block` instance) -- a brown cap does not hide against a red cap or a stem, and vice
/// versa.
pub struct HugeMushroomBlock;

impl BlockMetadata for HugeMushroomBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::BROWN_MUSHROOM_BLOCK,
            BlockId::RED_MUSHROOM_BLOCK,
            BlockId::MUSHROOM_STEM,
        ]
        .into()
    }
}

impl BlockBehaviour for HugeMushroomBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        // HugeMushroomBlock.java:47-57 (getStateForPlacement): every face starts visible
        // and is hidden only where the already-placed neighbour is this same block.
        get_state_with_connections(args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        // HugeMushroomBlock.java:59-73 (updateShape): a matching neighbour hides the shared
        // face. A non-matching neighbour falls back to the default Block behaviour, which
        // does not restore the face -- so once hidden, a face stays hidden even if the
        // matching neighbour is later removed (this is vanilla's actual, if odd, behaviour).
        let neighbor_block = args.world.get_block(args.neighbor_position);
        if neighbor_block != args.block {
            return args.state_id;
        }

        let mut props = BrownMushroomBlockLikeProperties::from_state_id(args.state_id);
        match args.direction {
            BlockDirection::Down => props.down = false,
            BlockDirection::Up => props.up = false,
            BlockDirection::North => props.north = false,
            BlockDirection::South => props.south = false,
            BlockDirection::East => props.east = false,
            BlockDirection::West => props.west = false,
        }
        props.to_state_id(args.block)
    }
}

#[must_use]
fn get_state_with_connections(
    block_accessor: &dyn BlockAccessor,
    block: &Block,
    pos: &BlockPos,
) -> BlockStateId {
    let matches_self = |b: &Block| b == block;

    let block_down = block_accessor.get_block(&pos.down());
    let block_up = block_accessor.get_block(&pos.up());
    let block_north = block_accessor.get_block(&pos.offset(BlockDirection::North.to_offset()));
    let block_east = block_accessor.get_block(&pos.offset(BlockDirection::East.to_offset()));
    let block_south = block_accessor.get_block(&pos.offset(BlockDirection::South.to_offset()));
    let block_west = block_accessor.get_block(&pos.offset(BlockDirection::West.to_offset()));

    let props = BrownMushroomBlockLikeProperties {
        down: !matches_self(block_down),
        up: !matches_self(block_up),
        north: !matches_self(block_north),
        east: !matches_self(block_east),
        south: !matches_self(block_south),
        west: !matches_self(block_west),
    };
    props.to_state_id(block)
}
