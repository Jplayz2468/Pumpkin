use crate::block::blocks::falling::FallingBlock;
use crate::block::entities::brushable_block::BrushableBlockBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, GetStateForNeighborUpdateArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::entity::falling::FallingEntity;
use pumpkin_data::{BlockId, BlockStateId};
use pumpkin_world::tick::TickPriority;

pub struct BrushableBlock;
impl BlockMetadata for BrushableBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SUSPICIOUS_SAND, BlockId::SUSPICIOUS_GRAVEL].into()
    }
}
impl BlockBehaviour for BrushableBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world
            .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        args.world
            .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        args.state_id
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(brushable) = entity.as_any().downcast_ref::<BrushableBlockBlockEntity>()
        {
            brushable.check_reset(args.world);
        }
        let (below_block, below_state) = args.world.get_block_and_state(&args.position.down());
        if FallingBlock::can_fall_through(below_state, below_block)
            && args.position.0.y >= args.world.min_y
        {
            FallingEntity::replace_spawn(
                args.world,
                *args.position,
                args.world.get_block_state_id(args.position),
            );
        }
    }
}
