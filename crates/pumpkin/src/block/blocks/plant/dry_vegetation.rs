use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, BlockStateId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, spreadable_neighbor},
};

pub struct DryVegetationBlock;

impl BlockMetadata for DryVegetationBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::DEAD_BUSH,
            BlockId::TALL_DRY_GRASS,
            BlockId::SHORT_DRY_GRASS,
        ]
        .into()
    }
}

impl BlockBehaviour for DryVegetationBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        if args.block == &Block::SHORT_DRY_GRASS {
            return true;
        }
        args.block == &Block::TALL_DRY_GRASS
            && spreadable_neighbor(
                args.world,
                args.position,
                Block::SHORT_DRY_GRASS.default_state,
                false,
            )
            .is_some()
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        if args.block == &Block::SHORT_DRY_GRASS {
            args.world.set_block_state(
                args.position,
                Block::TALL_DRY_GRASS.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        } else if args.block == &Block::TALL_DRY_GRASS
            && let Some(position) = spreadable_neighbor(
                args.world,
                args.position,
                Block::SHORT_DRY_GRASS.default_state,
                true,
            )
        {
            args.world.set_block_state(
                &position,
                Block::SHORT_DRY_GRASS.default_state.id,
                BlockFlags::NOTIFY_ALL,
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

impl PlantBlockBase for DryVegetationBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        let block_below = block_accessor.get_block(block_pos);
        block_below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_DRY_VEGETATION)
    }
}
