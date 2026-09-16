use super::is_exception_for_connection;
use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use crate::world::World;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, HorizontalFacingExt, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;

type IronBarsProperties = pumpkin_data::block_properties::OakFenceLikeProperties;

// Vanilla: Blocks.java:2340-2346 registers COPPER_BARS via
// `WeatheringCopperCollection.registerBlocks`, whose `waxedBlockFactory` is
// `(s, p) -> new IronBarsBlock(p)` (Blocks.java:2343) -- the waxed variants are plain
// `IronBarsBlock` instances with no weathering behaviour (waxing permanently strips
// `isRandomlyTicking`/`changeOverTime`). Only the *unwaxed* progression
// (copper_bars/exposed_copper_bars/weathered_copper_bars/oxidized_copper_bars) uses
// `WeatheringCopperBarsBlock`, registered after this base handler; it delegates
// connection behavior here and adds weathering (weathering_copper.rs).
#[pumpkin_block(
    "minecraft:iron_bars",
    "minecraft:waxed_copper_bars",
    "minecraft:waxed_exposed_copper_bars",
    "minecraft:waxed_weathered_copper_bars",
    "minecraft:waxed_oxidized_copper_bars"
)]
pub struct IronBarsBlock;

impl BlockBehaviour for IronBarsBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut bars_props = IronBarsProperties::default(args.block);
        bars_props.waterlogged = args.replacing.water_source();

        compute_bars_state(bars_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = IronBarsProperties::from_state_id(args.state_id);
        super::schedule_waterlogged_tick(args.world, args.position, props.waterlogged);
        if args.direction.is_horizontal() {
            let connected = IronBarsProperties::from_state_id(compute_bars_state(
                props,
                args.world,
                args.block,
                args.position,
            ));
            match args.direction {
                BlockDirection::North => props.north = connected.north,
                BlockDirection::East => props.east = connected.east,
                BlockDirection::South => props.south = connected.south,
                BlockDirection::West => props.west = connected.west,
                _ => {}
            }
        }
        props.to_state_id(args.block)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

pub fn compute_bars_state(
    mut bars_props: IronBarsProperties,
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
) -> BlockStateId {
    for direction in BlockDirection::horizontal() {
        let other_block_pos = block_pos.offset(direction.to_offset());
        let (other_block, other_block_state) = world.get_block_and_state(&other_block_pos);

        // Vanilla `IronBarsBlock.attachsTo` (IronBarsBlock.java:101-103):
        // `!isExceptionForConnection(state) && faceSolid || instanceof IronBarsBlock || WALLS tag`.
        // `minecraft:bars` (== `c:bars`) covers every block that `instanceof IronBarsBlock` in
        // vanilla: plain iron bars, all four unwaxed weathering stages, and all four waxed
        // stages (Blocks.java:2340-2346 registers the waxed variants as plain `IronBarsBlock`).
        let connected = other_block.has_tag(&tag::Block::MINECRAFT_BARS)
            || (!is_exception_for_connection(other_block)
                && other_block_state.is_side_solid(direction.opposite().to_block_direction()))
            || other_block.has_tag(&tag::Block::C_GLASS_PANES)
            || other_block.has_tag(&tag::Block::MINECRAFT_WALLS);

        match direction {
            HorizontalFacing::North => bars_props.north = connected,
            HorizontalFacing::South => bars_props.south = connected,
            HorizontalFacing::West => bars_props.west = connected,
            HorizontalFacing::East => bars_props.east = connected,
        }
    }

    bars_props.to_state_id(block)
}
