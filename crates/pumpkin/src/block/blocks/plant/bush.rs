use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, spreadable_neighbor},
};

pub struct BushBlock;

impl BlockMetadata for BushBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BUSH, BlockId::FIREFLY_BUSH].into()
    }
}

impl BlockBehaviour for BushBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        spreadable_neighbor(args.world, args.position, args.state_id.to_state(), false).is_some()
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        if let Some(position) =
            spreadable_neighbor(args.world, args.position, args.state_id.to_state(), true)
        {
            args.world.set_block_state(
                &position,
                args.block.default_state.id,
                pumpkin_world::world::BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
}

impl PlantBlockBase for BushBlock {}
