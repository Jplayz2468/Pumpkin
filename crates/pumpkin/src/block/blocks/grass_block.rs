use super::spreading_snowy_block::{SnowyBlock, SpreadingSnowyBlock};
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    RandomTickArgs,
};
use crate::world::generation_cache::WorldGenerationCache;
use pumpkin_data::{Block, BlockId, BlockStateId, placed_feature::PlacedFeature};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::generation::feature::{
    configured_features::BONE_MEAL_FEATURES, placed_features::PLACED_FEATURES,
};

pub struct GrassBlock;
impl BlockMetadata for GrassBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::GRASS_BLOCK].into()
    }
}

impl BlockBehaviour for GrassBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        SnowyBlock::on_place(args.block, args.world, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        SnowyBlock::get_state_for_neighbor_update(&args)
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        SpreadingSnowyBlock::random_tick(
            state,
            args.world,
            args.position,
            &Block::DIRT,
            Block::GRASS_BLOCK.default_state,
            &mut args.random,
        );
    }

    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        let above = args.position.up();
        args.world.is_in_height_limit(above.0.y) && args.world.get_block_state(&above).is_air()
    }

    fn is_bonemeal_success(&self, _args: crate::block::BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let origin = args.position.up();
        'attempt: for attempt in 0..128 {
            let mut target = origin;
            for _ in 0..attempt / 16 {
                target = target.add(
                    args.world.rand_bounded_i32(3) - 1,
                    (args.world.rand_bounded_i32(3) - 1) * args.world.rand_bounded_i32(3) / 2,
                    args.world.rand_bounded_i32(3) - 1,
                );
                if args.world.get_block(&target.down()) != args.block
                    || args.world.get_block_state(&target).is_full_cube()
                {
                    continue 'attempt;
                }
            }
            let state = args.world.get_block_state(&target);
            if state.id.to_block() == &Block::SHORT_GRASS && args.world.rand_bounded_i32(10) == 0 {
                let short_grass = super::plant::short_plant::ShortPlantBlock;
                let growth = BonemealArgs {
                    world: args.world,
                    block: &Block::SHORT_GRASS,
                    position: &target,
                    state_id: state.id,
                };
                if short_grass.is_valid_bonemeal_target(growth) {
                    short_grass.perform_bonemeal(growth);
                }
            }
            if state.is_air() && args.world.is_in_height_limit(target.0.y) {
                if args.world.rand_bounded_i32(8) == 0 {
                    place_biome_bonemeal_feature(args.world, target);
                } else {
                    WorldGenerationCache::place_placed_feature(
                        args.world,
                        PlacedFeature::GrassBonemeal,
                        target,
                    );
                }
            }
        }
    }
}

fn place_biome_bonemeal_feature(world: &std::sync::Arc<crate::world::World>, position: BlockPos) {
    let mut features = Vec::new();
    for placement in world
        .get_biome(&position)
        .features
        .iter()
        .flat_map(|step| step.iter())
    {
        if let Some(feature) = PLACED_FEATURES.get(placement) {
            let mut keys = Vec::new();
            feature.collect_feature_keys(&mut keys);
            features.extend(
                keys.into_iter()
                    .filter(|key| BONE_MEAL_FEATURES.contains(key))
                    .map(|key| (*placement, key)),
            );
        }
    }
    if !features.is_empty() {
        let (placement, key) = features[world.rand_bounded_i32(features.len() as i32) as usize];
        WorldGenerationCache::place_configured_feature(world, key, placement, position);
    }
}
