use super::growing::KELP;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, RandomTickArgs,
};
use pumpkin_data::{BlockId, BlockStateId};
pub struct KelpBlock;
impl BlockMetadata for KelpBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::KELP, BlockId::KELP_PLANT].into()
    }
}
impl BlockBehaviour for KelpBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        KELP.can_place_at(args)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        KELP.on_place(args)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        KELP.neighbor_state(args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        KELP.scheduled_tick(args);
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        KELP.random_tick(args);
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        KELP.bonemeal_target(args)
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        KELP.bonemeal(args);
    }
}
