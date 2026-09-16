use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, CanPlaceAtArgs, GetComparatorOutputArgs, GetStateForNeighborUpdateArgs,
        NormalUseArgs, OnPlaceArgs, PathComputationType, UseWithItemArgs,
        blocks::candle_cakes::cake_from_candle, registry::BlockActionResult,
    },
    entity::player::Player,
    world::World,
};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId,
    block_properties::CakeLikeProperties,
    sound::{Sound, SoundCategory},
    tag,
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

/// Vanilla `CakeBlock.getOutputSignal`. Saturates -> an out-of-range bite count reads 0.
#[must_use]
pub const fn cake_output_signal(bites: u8) -> u8 {
    7u8.saturating_sub(bites) * 2
}

/// Vanilla `CakeBlock.FULL_CAKE_SIGNAL`.
pub const FULL_CAKE_SIGNAL: u8 = cake_output_signal(0);

#[pumpkin_block("minecraft:cake")]
pub struct CakeBlock;

impl CakeBlock {
    pub fn consume_if_hungry(
        world: &Arc<World>,
        player: &Player,
        block: &Block,
        location: &BlockPos,
        state_id: BlockStateId,
    ) -> BlockActionResult {
        let food = player.hunger_manager.level.load();
        if !player
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable
            && food >= 20
        {
            return BlockActionResult::Pass;
        }
        player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::EatCakeSlice as i32,
            1,
        );
        let food = food.saturating_add(2).min(20);
        player.hunger_manager.level.store(food);
        player
            .hunger_manager
            .saturation
            .store((player.hunger_manager.saturation.load() + 0.4).clamp(0.0, f32::from(food)));
        player.send_health();
        let mut props = CakeLikeProperties::from_state_id(state_id);
        world.emit_game_event_from_entity("eat", location.to_centered_f64(), Some(player), None);
        if props.bites < 6 {
            props.bites += 1;
            world.set_block_state(location, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        } else {
            world.set_block_state(
                location,
                Block::AIR.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            world.emit_game_event_from_entity(
                "block_destroy",
                location.to_centered_f64(),
                Some(player),
                None,
            );
        }
        BlockActionResult::Success
    }
}

impl BlockBehaviour for CakeBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            return Block::AIR.default_state.id;
        }
        Block::CAKE.default_state.id
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let props = CakeLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        let item = args.item_stack.item;
        let cake = cake_from_candle(item);
        if !args.item_stack.is_empty()
            && item.has_tag(&tag::Item::MINECRAFT_CANDLES)
            && props.bites == 0
            && cake != &Block::CAKE
        {
            if !args.player.has_infinite_materials() {
                args.item_stack.decrement(1);
            }
            args.world.play_sound(
                Sound::BlockCakeAddCandle,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
            args.world.set_block_state(
                args.position,
                cake.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                Some(args.player.as_ref()),
                None,
            );
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Used,
                i32::from(item.id),
                1,
            );
            BlockActionResult::Success
        } else {
            BlockActionResult::PassToDefaultBlockAction
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        Self::consume_if_hungry(args.world, args.player, args.block, args.position, state_id)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == BlockDirection::Down && !can_place_at(args.world, args.position) {
            Block::AIR.default_state.id
        } else {
            args.state_id
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let state_id = args.world.get_block_state_id(args.position);
        let properties = CakeLikeProperties::from_state_id(state_id);
        Some(cake_output_signal(properties.bites))
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn can_place_at(world: &dyn BlockAccessor, position: &BlockPos) -> bool {
    let state = world.get_block_state(&position.down());
    state.is_solid()
}
