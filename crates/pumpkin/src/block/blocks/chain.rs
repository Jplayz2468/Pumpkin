use crate::block::{BlockBehaviour, OnPlaceArgs, PathComputationType};
use pumpkin_data::block_properties::Axis;
use pumpkin_data::{BlockDirection, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;

/// Covers `iron_chain` and the four waxed copper chain variants.
///
/// Vanilla `Blocks.java:2347-2358`: `IRON_CHAIN` is registered directly as a plain
/// `ChainBlock`, and `COPPER_CHAIN` is registered through
/// `WeatheringCopperCollection.registerBlocks` with `(s, p) -> new ChainBlock(p)` as the
/// *waxed* factory and `WeatheringCopperChainBlock::new` as the *weathering* (unwaxed)
/// factory. `WeatheringCopperCollection.registerBlocks` (see the copper bars/grate/golem
/// families above it) applies the waxed factory to the plain and all three
/// waxed-and-weathered ids, so `waxed_copper_chain`, `waxed_exposed_copper_chain`,
/// `waxed_weathered_copper_chain`, and `waxed_oxidized_copper_chain` are plain
/// `ChainBlock` instances -- byte-identical behaviour to `iron_chain`, since waxing only
/// suppresses `WeatheringCopper.onRandomTick`, which a plain `ChainBlock` never had to
/// begin with. The four blocks share `iron_chain`'s state layout (`WATERLOGGED` x `AXIS`,
/// see `crates/pumpkin-data/src/generated/block.rs`), so `IronChainLikeProperties`
/// applies unchanged.
///
/// The unwaxed weathering variants (`copper_chain`, `exposed_copper_chain`,
/// `weathered_copper_chain`, `oxidized_copper_chain`) are a different, already-registered
/// family: `WeatheringCopperBlock` in
/// `crates/pumpkin/src/block/blocks/weathering_copper.rs` lists them in its `ids()` and
/// drives their oxidation via `random_tick`.
#[pumpkin_block(
    "minecraft:iron_chain",
    "minecraft:waxed_copper_chain",
    "minecraft:waxed_exposed_copper_chain",
    "minecraft:waxed_weathered_copper_chain",
    "minecraft:waxed_oxidized_copper_chain"
)]
pub struct ChainBlock;

impl BlockBehaviour for ChainBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props =
            pumpkin_data::block_properties::IronChainLikeProperties::default(args.block);
        props.r#waterlogged = args.replacing.water_source();
        props.r#axis = match args.direction {
            BlockDirection::East | BlockDirection::West => Axis::X,
            BlockDirection::Up | BlockDirection::Down => Axis::Y,
            BlockDirection::North | BlockDirection::South => Axis::Z,
        };

        props.to_state_id(args.block)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockMetadata;
    use pumpkin_data::BlockId;

    /// The four waxed copper chain ids (`Blocks.java:2352-2358`, waxed factory
    /// `(s, p) -> new ChainBlock(p)`) previously had no registered behaviour at all --
    /// this pins that `ChainBlock::ids()` now covers `iron_chain` plus all four.
    #[test]
    fn ids_cover_iron_and_all_waxed_copper_variants() {
        let ids = ChainBlock::ids();
        for id in [
            BlockId::IRON_CHAIN,
            BlockId::WAXED_COPPER_CHAIN,
            BlockId::WAXED_EXPOSED_COPPER_CHAIN,
            BlockId::WAXED_WEATHERED_COPPER_CHAIN,
            BlockId::WAXED_OXIDIZED_COPPER_CHAIN,
        ] {
            assert!(ids.contains(&id), "{id:?} missing from ChainBlock ids");
        }
        assert_eq!(ids.len(), 5);
    }
}
