use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    AttackArgs, BlockBehaviour, GetStateForNeighborUpdateArgs, NormalUseArgs, OnScheduledTickArgs,
    PathComputationType, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use std::sync::Arc;

#[pumpkin_block("minecraft:dragon_egg")]
pub struct DragonEggBlock;

impl DragonEggBlock {
    fn teleport(world: &Arc<World>, pos: &BlockPos) {
        for _ in 0..1000 {
            let x = pos.0.x + world.rand_bounded_i32(16) - world.rand_bounded_i32(16);
            let y = pos.0.y + world.rand_bounded_i32(8) - world.rand_bounded_i32(8);
            let z = pos.0.z + world.rand_bounded_i32(16) - world.rand_bounded_i32(16);
            let test_pos = BlockPos::new(x, y, z);

            let state = world.get_block_state(&test_pos);
            let below_state = world.get_block_state(&test_pos.down());

            if state.is_air()
                && !below_state.is_air()
                && world
                    .worldborder
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_block(test_pos.0.x, test_pos.0.z)
                && world.is_in_height_limit(test_pos.0.y)
            {
                let current_state = world.get_block_state(pos);
                world.set_block_state(
                    &test_pos,
                    current_state.id,
                    pumpkin_world::world::BlockFlags::NOTIFY_LISTENERS,
                );
                world.set_block_state(
                    pos,
                    pumpkin_data::Block::AIR.default_state.id,
                    pumpkin_world::world::BlockFlags::NOTIFY_ALL,
                );
                return;
            }
        }
    }
}

impl BlockBehaviour for DragonEggBlock {
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        args.world
            .schedule_block_tick(args.block, *args.position, 5, TickPriority::Normal);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        Self::teleport(args.world, args.position);
        BlockActionResult::Success
    }

    fn attacked(&self, args: AttackArgs<'_>) {
        Self::teleport(args.world, args.position);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        args.world
            .schedule_block_tick(args.block, *args.position, 5, TickPriority::Normal);
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        FallingBlock::on_scheduled_tick(&FallingBlock, args);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
