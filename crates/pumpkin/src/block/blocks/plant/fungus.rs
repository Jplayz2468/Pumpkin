use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
};
use crate::world::generation_cache::WorldGenerationCache;
use pumpkin_data::{
    Block, BlockId, BlockStateId, configured_feature::ConfiguredFeature as FeatureKey,
    placed_feature::PlacedFeature, tag, tag::Taggable,
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    generation::feature::configured_features::{CONFIGURED_FEATURES, ConfiguredFeature},
    world::BlockAccessor,
};

pub struct FungusBlock;
impl BlockMetadata for FungusBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::CRIMSON_FUNGUS, BlockId::WARPED_FUNGUS].into()
    }
}
impl FungusBlock {
    fn can_survive(accessor: &dyn BlockAccessor, pos: &BlockPos, block: &Block) -> bool {
        let support = if block == &Block::WARPED_FUNGUS {
            &tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS
        } else {
            &tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS
        };
        accessor.get_block(&pos.down()).has_tag(support)
    }
}
impl BlockBehaviour for FungusBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(args.block_accessor, args.position, args.block)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if Self::can_survive(args.world, args.position, args.block) {
            args.state_id
        } else {
            BlockStateId::AIR
        }
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let required = if args.block == &Block::WARPED_FUNGUS {
            &Block::WARPED_NYLIUM
        } else {
            &Block::CRIMSON_NYLIUM
        };
        args.world.get_block(&args.position.down()) == required
            && args.world.is_in_height_limit(args.position.0.y + 1)
    }
    fn is_bonemeal_success(&self, args: BonemealArgs<'_>) -> bool {
        args.world.rand_f32() < 0.4
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let (key, placed) = if args.block == &Block::WARPED_FUNGUS {
            (FeatureKey::WarpedFungusPlanted, PlacedFeature::WarpedFungi)
        } else {
            (
                FeatureKey::CrimsonFungusPlanted,
                PlacedFeature::CrimsonFungi,
            )
        };
        let Some(ConfiguredFeature::HugeFungus(feature)) = CONFIGURED_FEATURES.get(&key) else {
            return;
        };
        let portal = args.world.level.world_portal.load_full();
        let Some(portal) = portal.as_ref() else {
            return;
        };
        let mut cache = WorldGenerationCache::new(args.world.clone(), args.position);
        let generated = cache.generate_with_level_random(|cache, random| {
            feature.generate(
                cache,
                &**portal,
                args.world.dimension.min_y as i8,
                args.world.dimension.height as u16,
                placed,
                random,
                *args.position,
            )
        });
        if generated {
            cache.apply();
        }
    }
}
