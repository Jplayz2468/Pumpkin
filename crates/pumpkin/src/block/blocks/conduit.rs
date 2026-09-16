use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:conduit")]
pub struct ConduitBlock;

impl BlockBehaviour for ConduitBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props =
            pumpkin_data::block_properties::MangroveRootsLikeProperties::default(args.block);
        let (fluid, state) = args.world.get_fluid_and_fluid_state(args.position);
        props.waterlogged =
            fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) && state.level == 8;

        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if pumpkin_data::block_properties::MangroveRootsLikeProperties::from_state_id(args.state_id)
            .waterlogged
        {
            args.world.schedule_fluid_tick(
                &pumpkin_data::fluid::Fluid::WATER,
                *args.position,
                5,
                pumpkin_world::tick::TickPriority::Normal,
            );
        }
        args.state_id
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
