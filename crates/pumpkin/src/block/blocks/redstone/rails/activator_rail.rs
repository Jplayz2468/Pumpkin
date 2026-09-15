use super::powered_rail::PoweredRailBlock;
use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnNeighborUpdateArgs,
    OnPlaceArgs, OnStateReplacedArgs, PlacedArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:activator_rail")]
pub struct ActivatorRailBlock;

// Both registrations use vanilla PoweredRailBlock; only the block identity differs.
impl BlockBehaviour for ActivatorRailBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        PoweredRailBlock.on_place(args)
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        PoweredRailBlock.placed(args);
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        PoweredRailBlock.on_neighbor_update(args);
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        PoweredRailBlock.get_state_for_neighbor_update(args)
    }
    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        PoweredRailBlock.on_state_replaced(args);
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        PoweredRailBlock.can_place_at(args)
    }
}
