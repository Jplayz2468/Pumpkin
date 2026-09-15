use pumpkin_data::{
    BlockDirection, BlockId, BlockStateId, block_properties::MangroveRootsLikeProperties, tag,
};
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, PlacedArgs,
    blocks::coral::{
        is_dead_coral, placement_waterlogged, scan_for_waterlogged_coral, schedule_water_tick,
        try_schedule_die_tick,
    },
};
pub struct CoralPlantBlock;
impl BlockMetadata for CoralPlantBlock {
    fn ids() -> Box<[BlockId]> {
        let alive_plants = tag::Block::MINECRAFT_CORAL_PLANTS.1;
        let mut plants = Vec::new();
        for alive_plant_id in alive_plants {
            let block_id = BlockId::new_or_air(*alive_plant_id);
            plants.push(block_id);
            plants.push(get_dead_type(block_id).unwrap_or_default());
        }
        plants.into()
    }
}
pub type CoralPlantLikeProperties = MangroveRootsLikeProperties;

impl BlockBehaviour for CoralPlantBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = CoralPlantLikeProperties::default(args.block);
        props.waterlogged = placement_waterlogged(args.world, args.position);
        props.to_state_id(args.block)
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        if !is_dead_coral(args.block)
            && !scan_for_waterlogged_coral(args.world, args.position, args.state_id)
        {
            try_schedule_die_tick(args.block, args.world, args.position);
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !is_dead_coral(args.block)
            && !scan_for_waterlogged_coral(
                args.world,
                args.position,
                args.world.get_block_state_id(args.position),
            )
        {
            let current_state = args.world.get_block_state(args.position);
            let dead_block_state_id = {
                let mut props = CoralPlantLikeProperties::from_state_id(current_state.id);
                props.waterlogged = false;
                props.to_state_id(get_dead_type(args.block.id).unwrap_or_default().to_block())
            };
            args.world.set_block_state(
                args.position,
                dead_block_state_id,
                BlockFlags::NOTIFY_LISTENERS,
            );
        }
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support_block = args.block_accessor.get_block_state(&args.position.down());
        if support_block.is_side_solid(BlockDirection::Up) {
            return true;
        }
        false
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if is_dead_coral(args.block) {
            schedule_water_tick(args.world, args.position, args.state_id);
        }

        if args.direction == BlockDirection::Down {
            let support_block = args.world.get_block_state(&args.position.down());
            if !support_block.is_side_solid(BlockDirection::Up) {
                return BlockStateId::AIR;
            }
        }
        if !is_dead_coral(args.block)
            && !scan_for_waterlogged_coral(args.world, args.position, args.state_id)
        {
            try_schedule_die_tick(args.block, args.world, args.position);
        }
        if !is_dead_coral(args.block) {
            schedule_water_tick(args.world, args.position, args.state_id);
        }
        args.state_id
    }
}
const fn get_dead_type(id: BlockId) -> Option<BlockId> {
    match id {
        BlockId::BRAIN_CORAL => Some(BlockId::DEAD_BRAIN_CORAL),
        BlockId::BUBBLE_CORAL => Some(BlockId::DEAD_BUBBLE_CORAL),
        BlockId::FIRE_CORAL => Some(BlockId::DEAD_FIRE_CORAL),
        BlockId::HORN_CORAL => Some(BlockId::DEAD_HORN_CORAL),
        BlockId::TUBE_CORAL => Some(BlockId::DEAD_TUBE_CORAL),
        _ => None,
    }
}
