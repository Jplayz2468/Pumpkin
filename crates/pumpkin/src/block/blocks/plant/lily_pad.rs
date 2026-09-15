use pumpkin_data::BlockStateId;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};

use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnEntityCollisionArgs};

#[pumpkin_block("minecraft:lily_pad")]
pub struct LilyPadBlock;

impl BlockBehaviour for LilyPadBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if args
            .entity
            .cast_any()
            .is::<crate::entity::vehicle::boat::BoatEntity>()
        {
            args.world
                .break_block_from_entity(args.position, args.entity, BlockFlags::NOTIFY_ALL);
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

impl PlantBlockBase for LilyPadBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let (block, state) = block_accessor.get_block_and_state(pos);
        let (fluid, fluid_state) = crate::world::World::fluid_state_from_block_state(state.id);
        let fluid =
            if fluid_state.is_source && fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) {
                &pumpkin_data::fluid::Fluid::WATER
            } else {
                fluid
            };
        let (_, above_fluid) = crate::world::World::fluid_state_from_block_state(
            block_accessor.get_block_state_id(&pos.up()),
        );
        (fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_LILY_PAD)
            || block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_LILY_PAD))
            && above_fluid.is_empty
    }
}
