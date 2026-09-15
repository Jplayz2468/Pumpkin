use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, GetComparatorOutputArgs, NormalUseArgs, OnScheduledTickArgs,
        PathComputationType, PlacedArgs, UseWithItemArgs, registry::BlockActionResult,
    },
    entity::{Entity, EntityBase, item::ItemEntity},
    world::World,
};
use pumpkin_data::{
    Block, BlockState, BlockStateId,
    block_properties::ComposterLikeProperties,
    composter_increase_chance::get_composter_increase_chance_from_item_id,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    world::WorldEvent,
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

#[pumpkin_block("minecraft:composter")]
pub struct ComposterBlock;

impl BlockBehaviour for ComposterBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        if ComposterLikeProperties::from_state_id(args.state_id).level == 7 {
            args.world
                .schedule_block_tick(args.block, *args.position, 20, TickPriority::Normal);
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state_id(args.position);
        if ComposterLikeProperties::from_state_id(state).level != 8 {
            return BlockActionResult::Pass;
        }
        Self::extract(args.world, args.position, state, Some(args.player.as_ref()));
        BlockActionResult::Success
    }
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state_id(args.position);
        let level = ComposterLikeProperties::from_state_id(state).level;
        let Some(chance) = get_composter_increase_chance_from_item_id(args.item_stack.item.id)
        else {
            return BlockActionResult::PassToDefaultBlockAction;
        };
        if level == 8 {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if level < 7 {
            let succeeds = level == 0 && chance > 0.0 || args.world.rand_f64() < f64::from(chance);
            if succeeds {
                let new_state = Self::set_level(args.world, args.position, state, level + 1);
                args.world.emit_game_event_from_entity(
                    "block_change",
                    args.position.to_centered_f64(),
                    Some(args.player.as_ref()),
                    Some(new_state),
                );
                if level + 1 == 7 {
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        20,
                        TickPriority::Normal,
                    );
                }
            }
            args.world.sync_world_event(
                WorldEvent::ComposterFill,
                *args.position,
                i32::from(succeeds),
            );
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Used,
                i32::from(args.item_stack.item.id),
                1,
            );
            if !args.player.has_infinite_materials() {
                args.item_stack.decrement(1);
            }
        }
        BlockActionResult::Success
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state_id(args.position);
        if ComposterLikeProperties::from_state_id(state).level == 7 {
            Self::set_level(args.world, args.position, state, 8);
            args.world.play_sound(
                Sound::BlockComposterReady,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        Some(ComposterLikeProperties::from_state_id(args.state.id).level)
    }
    fn is_pathfindable(&self, _: &BlockState, _: PathComputationType) -> bool {
        false
    }
}
impl ComposterBlock {
    fn set_level(
        world: &Arc<World>,
        pos: &BlockPos,
        state: BlockStateId,
        level: u8,
    ) -> BlockStateId {
        let mut props = ComposterLikeProperties::from_state_id(state);
        props.level = level;
        let next = props.to_state_id(&Block::COMPOSTER);
        world.set_block_state(pos, next, BlockFlags::NOTIFY_ALL);
        next
    }
    fn extract(
        world: &Arc<World>,
        pos: &BlockPos,
        state: BlockStateId,
        source: Option<&dyn EntityBase>,
    ) {
        let item_pos = pos.to_f64().add_raw(
            0.5 + f64::from((world.rand_f32() - 0.5) * 0.7),
            1.01,
            0.5 + f64::from((world.rand_f32() - 0.5) * 0.7),
        );
        world.spawn_entity(Arc::new(ItemEntity::new(
            Entity::new(world.clone(), item_pos, &EntityType::ITEM),
            ItemStack::new(1, &Item::BONE_MEAL),
        )));
        let next = Self::set_level(world, pos, state, 0);
        world.emit_game_event_from_entity(
            "block_change",
            pos.to_centered_f64(),
            source,
            Some(next),
        );
        world.play_sound(
            Sound::BlockComposterEmpty,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
        );
    }
}
