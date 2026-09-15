use crate::world::generation_cache::WorldGenerationCache;
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_data::{
    configured_feature::ConfiguredFeature as FeatureKey, placed_feature::PlacedFeature,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::lighting::LightEngine;
use pumpkin_world::world::BlockFlags;

use crate::block::{BlockBehaviour, BonemealArgs, RandomTickArgs};
use crate::world::World;

#[pumpkin_block_from_tag("minecraft:nylium")]
pub struct NyliumBlock;

impl NyliumBlock {
    #[must_use]
    pub(crate) const fn can_be_nylium_with_above(
        state: &BlockState,
        above_state: &BlockState,
    ) -> bool {
        let dampening = LightEngine::get_light_dampening_into(
            state,
            above_state,
            BlockDirection::Up,
            above_state.opacity,
        );
        dampening < 15
    }

    fn can_be_nylium(state: &BlockState, world: &World, pos: &BlockPos) -> bool {
        let above_pos = pos.up();
        let above_state = world.get_block_state(&above_pos);
        Self::can_be_nylium_with_above(state, above_state)
    }
}

impl BlockBehaviour for NyliumBlock {
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if !Self::can_be_nylium(state, args.world, args.position) {
            args.world.set_block_state(
                args.position,
                Block::NETHERRACK.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let above = args.position.up();
        args.world.is_in_height_limit(above.0.y) && args.world.get_block_state(&above).is_air()
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let above = args.position.up();
        let place = |key, placement| {
            if args.world.is_in_height_limit(above.0.y) {
                WorldGenerationCache::place_configured_feature(args.world, key, placement, above);
            }
        };
        let block = args.world.get_block(args.position);
        if block == &Block::CRIMSON_NYLIUM {
            place(
                FeatureKey::CrimsonForestVegetationBonemeal,
                PlacedFeature::CrimsonForestVegetation,
            );
        } else if block == &Block::WARPED_NYLIUM {
            place(
                FeatureKey::WarpedForestVegetationBonemeal,
                PlacedFeature::WarpedForestVegetation,
            );
            place(
                FeatureKey::NetherSproutsBonemeal,
                PlacedFeature::NetherSprouts,
            );
            if args.world.rand_bounded_i32(8) == 0 {
                place(
                    FeatureKey::TwistingVinesBonemeal,
                    PlacedFeature::TwistingVines,
                );
            }
        }
    }
}
