use super::growing::TWISTING;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, RandomTickArgs,
};
use pumpkin_data::{BlockId, BlockStateId};
pub struct TwistingVinesBlock;
impl BlockMetadata for TwistingVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::TWISTING_VINES, BlockId::TWISTING_VINES_PLANT].into()
    }
}
impl BlockBehaviour for TwistingVinesBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        TWISTING.can_place_at(args)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        TWISTING.on_place(args)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        TWISTING.neighbor_state(args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        TWISTING.scheduled_tick(args);
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        TWISTING.random_tick(args);
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        TWISTING.bonemeal_target(args)
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        TWISTING.bonemeal(args);
    }
}
