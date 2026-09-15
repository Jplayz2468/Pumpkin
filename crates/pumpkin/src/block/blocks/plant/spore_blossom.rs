use crate::block::{BlockBehaviour, CanPlaceAtArgs};
use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use pumpkin_data::BlockStateId;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, fluid::Fluid, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

#[pumpkin_block("minecraft:spore_blossom")]
pub struct SporeBlossomBlock;

impl BlockBehaviour for SporeBlossomBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == BlockDirection::Up
            && !<Self as PlantBlockBase>::can_place_at(self, args.world, args.position)
        {
            return Block::AIR.default_state.id;
        }
        args.state_id
    }
}
impl PlantBlockBase for SporeBlossomBlock {
    fn can_plant_on_top(
        &self,
        _block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        _pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        false
    }
    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        let (ceiling_block, ceiling_state) = block_accessor.get_block_and_state(&block_pos.up());
        let (fluid, _) = crate::world::World::fluid_state_from_block_state(
            block_accessor.get_block_state_id(block_pos),
        );
        !ceiling_block.has_tag(&tag::Block::MINECRAFT_UNSTABLE_BOTTOM_CENTER)
            && ceiling_state.is_center_solid(BlockDirection::Down)
            && !fluid.matches_type(&Fluid::WATER)
    }
}
