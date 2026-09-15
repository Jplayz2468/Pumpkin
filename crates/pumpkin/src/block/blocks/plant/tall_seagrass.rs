use crate::block::{
    BlockBehaviour, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, PlayerPlacedArgs,
    blocks::plant::{
        double_plant_neighbor_state, double_plant_survives, full_water_at,
        seagrass::supports_seagrass, tall_plant::TallPlantBlock,
    },
};
use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:tall_seagrass")]
pub struct TallSeaGrassBlock;

impl BlockBehaviour for TallSeaGrassBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let (support, state) = args
            .block_accessor
            .get_block_and_state(&args.position.down());
        let lower_survives =
            supports_seagrass(support, state) && full_water_at(args.block_accessor, args.position);
        if !double_plant_survives(
            args.block_accessor,
            args.block,
            args.state.id,
            args.position,
            lower_survives,
        ) {
            return false;
        }
        if args.use_item_on.is_some() {
            let above = args.position.up();
            let (block, state) = args.block_accessor.get_block_and_state(&above);
            return !args
                .world
                .is_some_and(|world| !world.is_in_height_limit(above.0.y))
                && full_water_at(args.block_accessor, &above)
                && crate::block::registry::can_replace_with_other_block(block, state);
        }
        true
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let (support, state) = args.world.get_block_and_state(&args.position.down());
        let lower_survives =
            supports_seagrass(support, state) && full_water_at(args.world, args.position);
        double_plant_neighbor_state(&args, lower_survives)
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        TallPlantBlock.player_placed(args);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        TallPlantBlock.broken(args);
    }
}
