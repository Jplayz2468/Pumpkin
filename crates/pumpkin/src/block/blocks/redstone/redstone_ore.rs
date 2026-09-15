use crate::block::{
    AttackArgs, BlockBehaviour, BlockMetadata, CanUpdateAtArgs, OnEntityStepArgs, RandomTickArgs,
    UseWithItemArgs, registry::BlockActionResult,
};
use crate::world::World;
use pumpkin_data::block_properties::RedstoneOreLikeProperties;
use pumpkin_data::{Block, BlockId, BlockState};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

pub struct RedstoneOreBlock;

impl BlockMetadata for RedstoneOreBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::REDSTONE_ORE, BlockId::DEEPSLATE_REDSTONE_ORE].into()
    }
}

impl RedstoneOreBlock {
    fn light_up(world: &Arc<World>, pos: &BlockPos, block: &Block, state: &BlockState) {
        // Level.addParticle is client-only, but its two random draws per exposed
        // face still occur on the server before changing LIT.
        for direction in pumpkin_data::BlockDirection::all() {
            if !world
                .get_block_state(&pos.offset(direction.to_offset()))
                .is_solid_render()
            {
                world.rand_f32();
                world.rand_f32();
            }
        }
        let mut props = RedstoneOreLikeProperties::from_state_id(state.id);
        if !props.lit {
            props.lit = true;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        }
    }
}

impl BlockBehaviour for RedstoneOreBlock {
    fn attacked(&self, args: AttackArgs<'_>) {
        Self::light_up(
            args.world,
            args.position,
            args.block,
            args.state_id.to_state(),
        );
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        Self::light_up(
            args.world,
            args.position,
            args.block,
            args.world.get_block_state(args.position),
        );
        if let Some(placed_block) = Block::from_item_id(args.item_stack.item.id) {
            let target = args.position.offset(args.hit.face.to_offset());
            let (block, state) = args.world.get_block_and_state(&target);
            let can_replace = if block == placed_block {
                args.world
                    .block_registry
                    .get_pumpkin_block(block.id)
                    .is_some_and(|behaviour| {
                        behaviour.can_update_at(CanUpdateAtArgs {
                            world: args.world,
                            block,
                            state_id: state.id,
                            position: &target,
                            direction: *args.hit.face,
                            player: args.player,
                            cursor_pos: args.hit.cursor_pos,
                            replacing_clicked: false,
                        })
                    })
            } else {
                crate::block::registry::can_replace_with_other_block(block, state)
            };
            if can_replace {
                return BlockActionResult::Pass;
            }
        }
        BlockActionResult::Success
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        if !args.entity.get_entity().is_sneaking() {
            Self::light_up(args.world, args.position, args.block, args.state);
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = RedstoneOreLikeProperties::from_state_id(state.id);

        if props.lit {
            props.lit = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
