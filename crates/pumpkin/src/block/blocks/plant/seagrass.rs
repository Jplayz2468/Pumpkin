use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId,
    block_properties::{DoubleBlockHalf, TallSeagrassLikeProperties},
    tag::{self, Taggable},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, full_water_at},
};
#[pumpkin_block("minecraft:seagrass")]
pub struct SeaGrassBlock;
impl BlockBehaviour for SeaGrassBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
            && (args.use_item_on.is_none() || full_water_at(args.block_accessor, args.position))
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let result = <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        );
        if !result.to_state().is_air() {
            args.world.schedule_fluid_tick(
                &pumpkin_data::fluid::Fluid::WATER,
                *args.position,
                pumpkin_data::fluid::Fluid::WATER.flow_speed as u32,
                pumpkin_world::tick::TickPriority::Normal,
            );
        }
        result
    }

    // SeagrassBlock.java:76 isValidBonemealTarget: needs water directly above to grow into.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        args.world.get_block(&args.position.up()) == &Block::WATER
    }

    // SeagrassBlock.java:91 performBonemeal: turns into a two-tall tall_seagrass.
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let mut lower = TallSeagrassLikeProperties::default(&Block::TALL_SEAGRASS);
        lower.half = DoubleBlockHalf::Lower;
        let mut upper = TallSeagrassLikeProperties::default(&Block::TALL_SEAGRASS);
        upper.half = DoubleBlockHalf::Upper;
        // Vanilla uses setBlock flag 2 (NOTIFY_LISTENERS only) for both halves.
        args.world.set_block_state(
            args.position,
            lower.to_state_id(&Block::TALL_SEAGRASS),
            BlockFlags::NOTIFY_LISTENERS,
        );
        args.world.set_block_state(
            &args.position.up(),
            upper.to_state_id(&Block::TALL_SEAGRASS),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }
}

impl PlantBlockBase for SeaGrassBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let (support_block, support_block_state) = block_accessor.get_block_and_state(pos);
        supports_seagrass(support_block, support_block_state)
    }
}
#[must_use]
pub fn supports_seagrass(support_block: &Block, support_block_state: &BlockState) -> bool {
    support_block_state.is_side_solid(BlockDirection::Up)
        && !support_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_SEAGRASS)
}
