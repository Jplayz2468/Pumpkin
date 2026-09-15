use super::growing::WEEPING;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, RandomTickArgs,
};
use pumpkin_data::{BlockId, BlockStateId};
pub struct WeepingVinesBlock;
impl BlockMetadata for WeepingVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::WEEPING_VINES, BlockId::WEEPING_VINES_PLANT].into()
    }
}
impl BlockBehaviour for WeepingVinesBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        WEEPING.can_place_at(args)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        WEEPING.on_place(args)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        WEEPING.neighbor_state(args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        WEEPING.scheduled_tick(args);
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        WEEPING.random_tick(args);
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        WEEPING.bonemeal_target(args)
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        WEEPING.bonemeal(args);
    }
}
