use std::sync::Arc;

use crate::block::entities::{has_block_block_entity, piston::PistonBlockEntity};
use crate::entity::EntityBase;
use pumpkin_data::BlockId;
use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId, FacingExt,
    block_properties::{MovingPistonLikeProperties, PistonHeadLikeProperties, PistonType},
    block_state::PistonBehavior,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rustc_hash::FxHashMap;

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, OnNeighborUpdateArgs, OnPlaceArgs, OnSyncedBlockEventArgs,
        PathComputationType, PlacedArgs, PlayerPlacedArgs,
        blocks::redstone::is_emitting_redstone_power,
    },
    world::World,
};

use super::PistonHandler;

pub(crate) type PistonProps = pumpkin_data::block_properties::StickyPistonLikeProperties;

pub struct PistonBlock;

impl BlockMetadata for PistonBlock {
    fn ids() -> Box<[BlockId]> {
        [Block::PISTON.id, Block::STICKY_PISTON.id].into()
    }
}

impl PistonBlock {
    #[must_use]
    /// Vanilla `PistonBaseBlock.isPushable` (`PistonBaseBlock.java:226`).
    pub fn is_movable(
        world: &World,
        pos: &BlockPos,
        block: &Block,
        state: &BlockState,
        dir: BlockDirection,
        can_break: bool,
        piston_dir: BlockDirection,
    ) -> bool {
        // Outside build height or beyond the world border is never pushable. Without
        // these a piston at the height limit or at the border moves blocks vanilla
        // refuses to move, which can duplicate or delete them.
        if pos.0.y < world.get_bottom_y()
            || pos.0.y > world.get_top_y()
            || !world
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_block(pos.0.x, pos.0.z)
        {
            return false;
        }

        if state.is_air() {
            return true;
        }
        // Vanilla hardcoded them aswell
        if block == &Block::OBSIDIAN
            || block == &Block::CRYING_OBSIDIAN
            || block == &Block::RESPAWN_ANCHOR
            || block == &Block::REINFORCED_DEEPSLATE
        {
            return false;
        }
        // A block cannot be pushed out through the bottom or top of the world.
        if dir == BlockDirection::Down && pos.0.y == world.get_bottom_y() {
            return false;
        }
        if dir == BlockDirection::Up && pos.0.y == world.get_top_y() {
            return false;
        }
        if block == &Block::PISTON || block == &Block::STICKY_PISTON {
            let props = PistonProps::from_state_id(state.id);
            // Extended pistons are immovable. Non-extended pistons are movable
            return !props.extended;
        }
        #[expect(clippy::float_cmp)]
        if state.hardness == -1.0 {
            return false;
        }
        match state.piston_behavior {
            pumpkin_data::block_state::PistonBehavior::Destroy => return can_break,
            pumpkin_data::block_state::PistonBehavior::Block => return false,
            pumpkin_data::block_state::PistonBehavior::PushOnly => return dir == piston_dir,
            _ => {}
        }
        !has_block_block_entity(block)
    }
}

impl BlockBehaviour for PistonBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = PistonProps::default(args.block);
        props.extended = false;
        props.facing = args.player.get_entity().get_facing().opposite();
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.world.get_block_entity(args.position).is_some() {
            return;
        }
        try_move(args.world, args.block, args.position);
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        try_move(args.world, args.block, args.position);
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.world.get_block(args.position) != args.block {
            return;
        }
        try_move(args.world, args.block, args.position);
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        let block_id = args.block.id;
        let block = Block::from_id(block_id);
        Self::handle_synced_block_event(block, args.world, args.position, args.r#type, args.data)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

impl PistonBlock {
    #[expect(clippy::too_many_lines)]
    fn handle_synced_block_event(
        block: &Block,
        world: &Arc<World>,
        pos: &BlockPos,
        r#type: u8,
        data: u8,
    ) -> bool {
        let state = world.get_block_state(pos);
        if state.id.to_block() != block {
            return false;
        }
        let mut props = PistonProps::from_state_id(state.id);
        let dir = props.facing.to_block_direction();

        // I don't think this is optimal ?
        let sticky = block == &Block::STICKY_PISTON;

        let should_extend = should_extend(world, pos, dir);
        if should_extend && (r#type == 1 || r#type == 2) {
            props.extended = true;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
            return false;
        }

        // This may prevents when something happens in the one tick before this function got called
        if !should_extend && r#type == 0 {
            return false;
        }

        // Extend Piston
        if r#type == 0 {
            let mut event =
                crate::plugin::api::events::block::block_piston::BlockPistonExtendEvent::new(
                    *pos,
                    format!("{dir:?}"),
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return false;
            }

            if !move_piston(world, dir, pos, true, sticky) {
                return false;
            }
            props.extended = true;
            world.set_block_state(
                pos,
                props.to_state_id(block),
                BlockFlags::NOTIFY_ALL | BlockFlags::MOVED,
            );
            // Play piston extend sound
            let pitch = world.rand_f32() * 0.25 + 0.6;
            world.play_sound_fine(
                Sound::BlockPistonExtend,
                SoundCategory::Blocks,
                &pos.to_centered_f64(),
                0.5,
                pitch,
            );
            world.emit_game_event_with_context(
                pumpkin_data::game_event::GameEvent::BlockActivate.name(),
                pos.to_centered_f64(),
                None,
                Some(props.to_state_id(block)),
            );
            return true;
        }
        if r#type != 1 && r#type != 2 {
            return true;
        }
        // Reduce Piston

        let mut event =
            crate::plugin::api::events::block::block_piston::BlockPistonRetractEvent::new(
                *pos,
                format!("{dir:?}"),
            );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        let extended_pos = pos.offset(dir.to_offset());

        if let Some(block_entity) = world.get_block_entity(&extended_pos)
            && let Some(piston) = block_entity.as_any().downcast_ref::<PistonBlockEntity>()
        {
            piston.finish(world);
        }

        let mut props = MovingPistonLikeProperties::default(&Block::MOVING_PISTON);
        props.facing = dir.to_facing();
        props.r#type = if sticky {
            PistonType::Sticky
        } else {
            PistonType::Normal
        };

        let moving_state = props.to_state_id(&Block::MOVING_PISTON);
        world.set_block_state(
            pos,
            moving_state,
            BlockFlags::SKIP_SHAPE_UPDATES | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
        );

        let mut props = PistonProps::default(block);
        props.facing = BlockDirection::by_index((data & 7) as usize)
            .unwrap_or(BlockDirection::North)
            .to_facing();

        world.add_block_entity(Arc::new(PistonBlockEntity {
            position: *pos,
            facing: dir,
            pushed_block_state: BlockState::from_id(props.to_state_id(block)),
            current_progress: 0.0.into(),
            last_progress: 0.0.into(),
            extending: false,
            source: true,
            last_ticked: 0.into(),
        }));

        world.update_neighbors_at(pos, &Block::MOVING_PISTON, None);
        world
            .block_registry
            .update_neighbors(world, pos, BlockFlags::NOTIFY_LISTENERS);
        if sticky {
            let pull_pos = pos.offset_dir(dir.to_offset(), 2);
            let (pull_block, pull_state) = world.get_block_and_state(&pull_pos);
            let mut piston_piece = false;
            if pull_block == &Block::MOVING_PISTON
                && let Some(entity) = world.get_block_entity(&pull_pos)
                && let Some(piston) = entity.as_any().downcast_ref::<PistonBlockEntity>()
                && piston.facing == dir
                && piston.extending
            {
                piston.finish(world);
                piston_piece = true;
            }
            if !piston_piece {
                if r#type == 1
                    && !pull_state.is_air()
                    && Self::is_movable(
                        world,
                        &pull_pos,
                        pull_block,
                        pull_state,
                        dir.opposite(),
                        false,
                        dir,
                    )
                    && (pull_state.piston_behavior == PistonBehavior::Normal
                        || pull_block == &Block::PISTON
                        || pull_block == &Block::STICKY_PISTON)
                {
                    move_piston(world, dir, pos, false, sticky);
                } else {
                    world.set_block_state(&extended_pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
                }
            }
        } else {
            world.set_block_state(&extended_pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
        }
        // Play piston contract sound
        let pitch = world.rand_f32() * 0.15 + 0.6;
        world.play_sound_fine(
            Sound::BlockPistonContract,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            0.5,
            pitch,
        );
        world.emit_game_event_with_context(
            pumpkin_data::game_event::GameEvent::BlockDeactivate.name(),
            pos.to_centered_f64(),
            None,
            Some(moving_state),
        );
        true
    }
}

fn should_extend(world: &World, block_pos: &BlockPos, piston_dir: BlockDirection) -> bool {
    for dir in BlockDirection::all() {
        let neighbor_pos = block_pos.offset(dir.to_offset());
        let (block, state) = world.get_block_and_state(&neighbor_pos);
        // Pistons can't be powered from the same direction as they are facing
        if dir == piston_dir || !is_emitting_redstone_power(block, state, world, &neighbor_pos, dir)
        {
            continue;
        }
        return true;
    }
    let neighbor_pos = block_pos.offset(BlockDirection::Down.to_offset());
    let (block, state) = world.get_block_and_state(&neighbor_pos);
    if is_emitting_redstone_power(block, state, world, block_pos, BlockDirection::Down) {
        return true;
    }
    for dir in BlockDirection::all() {
        let neighbor_pos = block_pos.up().offset(dir.to_offset());
        let (block, state) = world.get_block_and_state(&neighbor_pos);
        if dir == BlockDirection::Down
            || !is_emitting_redstone_power(block, state, world, &neighbor_pos, dir)
        {
            continue;
        }
        return true;
    }
    false
}

pub fn try_move(world: &Arc<World>, _block: &Block, block_pos: &BlockPos) {
    let state = world.get_block_state(block_pos);
    let props = PistonProps::from_state_id(state.id);
    let dir = props.facing.to_block_direction();
    let should_extent = should_extend(world, block_pos, dir);

    if should_extent && !props.extended {
        if PistonHandler::new(world, *block_pos, dir, true).calculate_push() {
            world.add_synced_block_event(*block_pos, 0, dir.to_index());
        }
    } else if !should_extent && props.extended {
        let new_pos = block_pos.offset_dir(dir.to_offset(), 2);
        let (new_block, new_state) = world.get_block_and_state_id(&new_pos);
        let mut r#type = 1;

        if new_block == &Block::MOVING_PISTON {
            let new_props = MovingPistonLikeProperties::from_state_id(new_state);
            if new_props.facing == props.facing
                && let Some(entity) = world.get_block_entity(&new_pos)
            {
                let Some(piston) = entity.as_any().downcast_ref::<PistonBlockEntity>() else {
                    return;
                };
                // Vanilla `PistonBaseBlock.checkIfExtend` (`PistonBaseBlock.java:119`):
                //   isExtending() && (getProgress(0.0F) < 0.5F
                //                     || gameTime == getLastTicked()
                //                     || level.isHandlingTick())
                // Only the first clause was checked here. The other two are what make a
                // sticky piston still drag its block when the retraction lands on the
                // same tick the mover was ticked, or while the level is still inside the
                // block-tick/block-event portion of the tick -- the short-pulse cases.
                if piston.extending
                    && (piston.current_progress.load() < 0.5
                        || world.get_world_age() == piston.last_ticked.load()
                        || world.is_handling_tick())
                {
                    // Piston reduced too quickly, if its a sticky piston no blocks will be dragged
                    r#type = 2;
                }
            }
        }
        world.add_synced_block_event(*block_pos, r#type, dir.to_index());
    }
}

#[expect(clippy::too_many_lines)]
fn move_piston(
    world: &Arc<World>,
    dir: BlockDirection,
    block_pos: &BlockPos,
    extend: bool,
    sticky: bool,
) -> bool {
    let extended_pos = block_pos.offset(dir.to_offset());
    if !extend && world.get_block(&extended_pos) == &Block::PISTON_HEAD {
        world.set_block_state(
            &extended_pos,
            Block::AIR.default_state.id,
            BlockFlags::SKIP_SHAPE_UPDATES | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
        );
    }
    let mut handler = PistonHandler::new(world, *block_pos, dir, extend);
    if !handler.calculate_push() {
        return false;
    }

    let mut moved_blocks_map: FxHashMap<BlockPos, &BlockState> = FxHashMap::default();
    let moved_blocks: Vec<BlockPos> = handler.moved_blocks;

    let mut moved_block_states: Vec<&BlockState> = Vec::new();

    for &block_pos in &moved_blocks {
        let block_state = world.get_block_state(&block_pos);
        moved_block_states.push(block_state);
        moved_blocks_map.insert(block_pos, block_state);
    }

    let broken_blocks: Vec<BlockPos> = handler.broken_blocks;
    let mut affected_block_states: Vec<&BlockState> =
        Vec::with_capacity(moved_blocks.len() + broken_blocks.len());
    let move_direction = if extend { dir } else { dir.opposite() };

    for &broken_block_pos in broken_blocks.iter().rev() {
        let block_state = world.get_block_state(&broken_block_pos);
        world.break_block(
            &broken_block_pos,
            None,
            BlockFlags::NOTIFY_LISTENERS | BlockFlags::SKIP_SHAPE_UPDATES,
        );
        affected_block_states.push(block_state);
    }

    for (index, &moved_block_pos) in moved_blocks.iter().rev().enumerate() {
        let block_state = world.get_block_state(&moved_block_pos);
        let target_pos = moved_block_pos.offset(move_direction.to_offset());
        moved_blocks_map.remove(&target_pos);

        let mut props = MovingPistonLikeProperties::default(&Block::MOVING_PISTON);
        props.facing = dir.to_facing();
        let state = props.to_state_id(&Block::MOVING_PISTON);

        world.set_block_state(
            &target_pos,
            state,
            BlockFlags::MOVED | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
        );

        if let Some(moved_state) = moved_block_states.get(moved_blocks.len() - 1 - index) {
            world.add_block_entity(Arc::new(PistonBlockEntity {
                position: target_pos,
                facing: dir.to_facing().to_block_direction(),
                pushed_block_state: moved_state,
                current_progress: 0.0.into(),
                last_progress: 0.0.into(),
                extending: extend,
                source: false,
                last_ticked: 0.into(),
            }));
        }
        affected_block_states.push(block_state);
    }

    if extend {
        let pistion_type = if sticky {
            PistonType::Sticky
        } else {
            PistonType::Normal
        };
        let mut props = MovingPistonLikeProperties::default(&Block::MOVING_PISTON);
        props.facing = dir.to_facing();
        props.r#type = pistion_type;
        moved_blocks_map.remove(&extended_pos);
        world.set_block_state(
            &extended_pos,
            props.to_state_id(&Block::MOVING_PISTON),
            BlockFlags::MOVED | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
        );
        let mut props = PistonHeadLikeProperties::default(&Block::PISTON_HEAD);
        props.facing = dir.to_facing();
        props.r#type = pistion_type;
        world.add_block_entity(Arc::new(PistonBlockEntity {
            position: extended_pos,
            facing: dir.to_facing().to_block_direction(),
            pushed_block_state: BlockState::from_id(props.to_state_id(&Block::PISTON_HEAD)),
            current_progress: 0.0.into(),
            last_progress: 0.0.into(),
            extending: true,
            source: true,
            last_ticked: 0.into(),
        }));
    }

    let air_state = Block::AIR.default_state.id;
    for &pos in moved_blocks_map.keys() {
        world.set_block_state(
            &pos,
            air_state,
            BlockFlags::NOTIFY_LISTENERS | BlockFlags::SKIP_SHAPE_UPDATES | BlockFlags::MOVED,
        );
    }

    for (pos, state) in &moved_blocks_map {
        world.block_registry.prepare(
            world,
            pos,
            Block::from_state_id(state.id),
            state.id,
            BlockFlags::NOTIFY_LISTENERS,
        );
        world
            .block_registry
            .update_neighbors(world, pos, BlockFlags::NOTIFY_LISTENERS);
        world.block_registry.prepare(
            world,
            pos,
            &Block::AIR,
            air_state,
            BlockFlags::NOTIFY_LISTENERS,
        );
    }

    for (i, &broken_block_pos) in broken_blocks.iter().rev().enumerate() {
        if let Some(block_state) = affected_block_states.get(i) {
            world.block_registry.on_state_replaced(
                world,
                Block::from_state_id(block_state.id),
                &broken_block_pos,
                block_state.id, // ?
                false,
            );
            world.block_registry.prepare(
                world,
                &broken_block_pos,
                Block::from_state_id(block_state.id),
                block_state.id,
                BlockFlags::NOTIFY_LISTENERS,
            );
            world.update_neighbors_at(
                &broken_block_pos,
                Block::from_state_id(block_state.id),
                None,
            );
        }
    }
    for (i, &moved_block_pos) in moved_blocks.iter().rev().enumerate() {
        if let Some(old_state) = moved_block_states.get(moved_blocks.len() - 1 - i) {
            let old_block = Block::from_state_id(old_state.id);
            world.update_neighbors_at(&moved_block_pos, old_block, None);
        }
    }

    if extend {
        world.update_neighbors_at(&extended_pos, &Block::PISTON_HEAD, None);
    }

    true
}
