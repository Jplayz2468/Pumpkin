use super::common;
use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnNeighborUpdateArgs,
    OnPlaceArgs, OnStateReplacedArgs, PlacedArgs,
};
use pumpkin_data::{BlockDirection, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block("minecraft:rail")]
pub struct RailBlock;
impl BlockBehaviour for RailBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        common::placement(args)
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        common::update_direction(args.world, *args.position, args.state_id, true);
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        common::water_update(args)
    }
    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        common::removed(args, false);
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        common::can_place_rail_at(args.block_accessor, args.position)
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.world.get_block(args.position) != args.block {
            return;
        }
        if !common::rail_placement_is_valid(args.world, args.block, args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        } else if args.world.block_registry.emits_redstone_power(
            args.source_block,
            args.source_block.default_state,
            BlockDirection::Up,
        ) && common::potential_connections(args.world, *args.position) == 3
        {
            common::update_direction(
                args.world,
                *args.position,
                args.world.get_block_state_id(args.position),
                false,
            );
        }
    }
}
