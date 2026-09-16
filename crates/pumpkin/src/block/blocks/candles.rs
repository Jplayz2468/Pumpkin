use std::sync::Arc;

use pumpkin_data::{
    Block, BlockDirection, BlockStateId,
    block_properties::CandleLikeProperties,
    fluid::Fluid,
    sound::{Sound, SoundCategory},
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

use crate::{
    block::{
        BlockBehaviour, BlockIsReplacing, CanPlaceAtArgs, CanUpdateAtArgs, ExplodeArgs,
        GetStateForNeighborUpdateArgs, OnPlaceArgs, OnProjectileHitArgs, UseWithItemArgs,
        registry::BlockActionResult,
    },
    entity::{EntityBase, player::Player},
    world::World,
};

/// AbstractCandleBlock.setLit. Java's additional flag 8 is a client render hint.
fn with_lit(block: &Block, state: BlockStateId, lit: bool) -> BlockStateId {
    let mut properties = block
        .properties(state)
        .expect("candle properties")
        .to_props();
    if let Some((_, value)) = properties.iter_mut().find(|(key, _)| *key == "lit") {
        *value = if lit { "true" } else { "false" };
    }
    block.from_properties(&properties).to_state_id(block)
}

pub(crate) fn is_lit(block: &Block, state: BlockStateId) -> bool {
    block.properties(state).is_some_and(|props| {
        props
            .to_props()
            .iter()
            .any(|(key, value)| *key == "lit" && *value == "true")
    })
}

pub(crate) fn extinguish(
    world: &Arc<World>,
    pos: &BlockPos,
    block: &Block,
    state: BlockStateId,
    player: Option<&Player>,
) {
    world.set_block_state(pos, with_lit(block, state, false), BlockFlags::NOTIFY_ALL);
    // LevelAccessor.addParticle is a no-op on ServerLevel; the client handles smoke.
    world.play_sound(
        Sound::BlockCandleExtinguish,
        SoundCategory::Blocks,
        &pos.to_centered_f64(),
    );
    world.emit_game_event_from_entity(
        "block_change",
        pos.to_centered_f64(),
        player.map(|player| player as &dyn EntityBase),
        None,
    );
}

pub(crate) fn projectile_hit(args: OnProjectileHitArgs<'_>) {
    if args.projectile.get_entity().is_on_fire()
        && !is_lit(args.block, args.state.id)
        && !args.block.is_waterlogged(args.state.id)
    {
        args.world.set_block_state(
            args.position,
            with_lit(args.block, args.state.id, true),
            BlockFlags::NOTIFY_ALL,
        );
    }
}

pub(crate) fn explosion_hit(args: ExplodeArgs<'_>) {
    if args.can_trigger_blocks && is_lit(args.block, args.state.id) {
        extinguish(args.world, args.position, args.block, args.state.id, None);
    }
}

#[pumpkin_block_from_tag("minecraft:candles")]
pub struct CandleBlock;

impl BlockBehaviour for CandleBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let mut props = CandleLikeProperties::from_state_id(state_id);
            props.candles = props.candles % 4 + 1;
            return props.to_state_id(args.block);
        }
        let mut props = CandleLikeProperties::default(args.block);
        let (fluid, state) =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position));
        props.waterlogged = fluid.matches_type(&Fluid::WATER) && state.is_source;
        props.to_state_id(args.block)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state_id(args.position);
        if args.item_stack.is_empty() && args.player.may_build() && is_lit(args.block, state) {
            extinguish(
                args.world,
                args.position,
                args.block,
                state,
                Some(args.player),
            );
            BlockActionResult::Success
        } else {
            BlockActionResult::PassToDefaultBlockAction
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.block_accessor
            .get_block_state(&args.position.down())
            .is_center_solid(BlockDirection::Up)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        !args.player.get_entity().is_sneaking()
            && CandleLikeProperties::from_state_id(args.state_id).candles < 4
            && args.block == args.world.get_block(args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if CandleLikeProperties::from_state_id(args.state_id).waterlogged {
            args.world
                .schedule_fluid_tick(&Fluid::WATER, *args.position, 5, TickPriority::Normal);
        }
        // CandleBlock inherits Block.updateShape: support loss does not remove it.
        args.state_id
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        projectile_hit(args);
    }
    fn explode(&self, args: ExplodeArgs<'_>) {
        explosion_hit(args);
    }
}
