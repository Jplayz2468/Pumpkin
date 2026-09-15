use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    PlayerPlacedArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::is_air;
use pumpkin_macros::{pumpkin_block, pumpkin_block_from_tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::RandomImpl;
use pumpkin_world::block::mossy_carpet;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block_from_tag("minecraft:wool_carpets")]
pub struct CarpetBlock;

impl BlockBehaviour for CarpetBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if can_place_at(args.world, args.position) {
            args.state_id
        } else {
            BlockStateId::AIR
        }
    }
}

#[pumpkin_block("minecraft:moss_carpet")]
pub struct MossCarpetBlock;

impl BlockBehaviour for MossCarpetBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if can_place_at(args.world, args.position) {
            args.state_id
        } else {
            BlockStateId::AIR
        }
    }
}

#[pumpkin_block("minecraft:pale_moss_carpet")]
pub struct PaleMossCarpetBlock;

impl BlockBehaviour for PaleMossCarpetBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        mossy_carpet::can_survive(args.block_accessor, args.position, args.state.id)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        mossy_carpet::updated_state(args.world, args.position, args.block.default_state.id, true)
    }
    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        let mut random = crate::block::random::BlockRandom::Shared(&args.world.random);
        let topper =
            mossy_carpet::create_topper(args.world.as_ref(), args.position, || random.next_bool());
        if topper != BlockStateId::AIR {
            args.world
                .set_block_state(&args.position.up(), topper, BlockFlags::NOTIFY_ALL);
        }
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !mossy_carpet::can_survive(args.world, args.position, args.state_id) {
            return BlockStateId::AIR;
        }
        let state = mossy_carpet::updated_state(args.world, args.position, args.state_id, false);
        if mossy_carpet::has_faces(state) {
            state
        } else {
            BlockStateId::AIR
        }
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        mossy_carpet::is_base(args.state_id)
            && mossy_carpet::create_topper(args.world.as_ref(), args.position, || true)
                != BlockStateId::AIR
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let topper = mossy_carpet::create_topper(args.world.as_ref(), args.position, || true);
        if topper != BlockStateId::AIR {
            args.world
                .set_block_state(&args.position.up(), topper, BlockFlags::NOTIFY_ALL);
        }
    }
}

fn can_place_at(block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    !is_air(block_accessor.get_block_state_id(&block_pos.down()))
}
