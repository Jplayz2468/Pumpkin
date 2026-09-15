use pumpkin_data::configured_feature::ConfiguredFeature as FeatureKey;
use pumpkin_data::{Block, BlockId};

use crate::block::{BlockBehaviour, BlockMetadata, BonemealArgs};
use crate::world::generation_cache::WorldGenerationCache;

/// Moss and pale moss blocks generate their bonemeal "moss patch" via a
/// [`ConfiguredFeature`], mirroring `BonemealableFeaturePlacerBlock` (vanilla
/// 26.2, `BonemealableFeaturePlacerBlock.java`), which is constructed with a
/// `ResourceKey<ConfiguredFeature<?, ?>>` per block instance.
pub struct MossBlock;

impl BlockMetadata for MossBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::MOSS_BLOCK, BlockId::PALE_MOSS_BLOCK].into()
    }
}

impl MossBlock {
    /// The configured feature each block places, per vanilla's block registration
    /// (`Blocks.java`: `moss_block` -> `MOSS_PATCH_BONEMEAL`, `pale_moss_block` ->
    /// `PALE_MOSS_PATCH_BONEMEAL`).
    fn feature_for(block: &Block) -> Option<FeatureKey> {
        if block == &Block::MOSS_BLOCK {
            Some(FeatureKey::MossPatchBonemeal)
        } else if block == &Block::PALE_MOSS_BLOCK {
            Some(FeatureKey::PaleMossPatchBonemeal)
        } else {
            None
        }
    }
}

impl BlockBehaviour for MossBlock {
    // BonemealableFeaturePlacerBlock.java:34-36 - only the block directly above
    // must be air; there is no loaded/height-limit check like grass block has.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        args.world.get_block_state(&args.position.up()).is_air()
    }

    // BonemealableFeaturePlacerBlock.java:39-41 - always succeeds once targetable.
    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    // BonemealableFeaturePlacerBlock.java:44-49 - looks up the configured feature
    // and places it one block above the moss block.
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let Some(key) = Self::feature_for(args.block) else {
            return;
        };
        WorldGenerationCache::place_configured_feature(
            args.world,
            key,
            pumpkin_data::placed_feature::PlacedFeature::PaleMossPatch,
            args.position.up(),
        );
    }
}
