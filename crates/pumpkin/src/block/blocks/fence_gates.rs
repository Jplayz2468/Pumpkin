use std::sync::Arc;

use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, ExplodeArgs, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::world::World;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

type FenceGateProperties = pumpkin_data::block_properties::OakFenceGateLikeProperties;

fn get_sound(block: &Block, open: bool) -> Sound {
    match (block, open) {
        (b, true) if b == &Block::BAMBOO_FENCE_GATE => Sound::BlockBambooWoodFenceGateOpen,
        (b, false) if b == &Block::BAMBOO_FENCE_GATE => Sound::BlockBambooWoodFenceGateClose,
        (b, true) if b == &Block::CHERRY_FENCE_GATE => Sound::BlockCherryWoodFenceGateOpen,
        (b, false) if b == &Block::CHERRY_FENCE_GATE => Sound::BlockCherryWoodFenceGateClose,
        (b, true) if b == &Block::CRIMSON_FENCE_GATE || b == &Block::WARPED_FENCE_GATE => {
            Sound::BlockNetherWoodFenceGateOpen
        }
        (b, false) if b == &Block::CRIMSON_FENCE_GATE || b == &Block::WARPED_FENCE_GATE => {
            Sound::BlockNetherWoodFenceGateClose
        }
        (_, true) => Sound::BlockFenceGateOpen,
        (_, false) => Sound::BlockFenceGateClose,
    }
}

fn play_sound(
    world: &Arc<World>,
    pos: &BlockPos,
    block: &Block,
    open: bool,
    player: Option<&Player>,
    state: Option<BlockStateId>,
) {
    let sound = get_sound(block, open);
    let pitch = world.rand_f32() * 0.1 + 0.9;
    if let Some(player) = player {
        world.play_sound_raw_expect(
            player,
            sound as u16,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            pitch,
        );
    } else {
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            pitch,
        );
    }
    world.emit_game_event_from_entity(
        if open { "block_open" } else { "block_close" },
        pos.to_centered_f64(),
        player.map(|player| player as &dyn EntityBase),
        state,
    );
}

pub fn toggle_fence_gate(
    world: &Arc<World>,
    block_pos: &BlockPos,
    player: &Player,
) -> BlockStateId {
    let (block, state) = world.get_block_and_state_id(block_pos);

    let mut fence_gate_props = FenceGateProperties::from_state_id(state);
    if fence_gate_props.open {
        fence_gate_props.open = false;
    } else {
        if fence_gate_props.facing
            == player
                .living_entity
                .entity
                .get_horizontal_facing()
                .opposite()
        {
            fence_gate_props.facing = player.get_entity().get_horizontal_facing();
        }
        fence_gate_props.open = true;
    }

    world.set_block_state(
        block_pos,
        fence_gate_props.to_state_id(block),
        BlockFlags::NOTIFY_LISTENERS,
    );
    play_sound(
        world,
        block_pos,
        block,
        fence_gate_props.open,
        Some(player),
        None,
    );
    fence_gate_props.to_state_id(block)
}

#[pumpkin_block_from_tag("minecraft:fence_gates")]
pub struct FenceGateBlock;

impl BlockBehaviour for FenceGateBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut fence_gate_props = FenceGateProperties::default(args.block);
        fence_gate_props.facing = args.player.get_entity().get_horizontal_facing();

        let powered = block_receives_redstone_power(args.world, args.position);
        fence_gate_props.powered = powered;
        fence_gate_props.open = powered;

        let left = args
            .position
            .offset(fence_gate_props.facing.rotate_clockwise().to_offset());
        let right = args.position.offset(
            fence_gate_props
                .facing
                .rotate_counter_clockwise()
                .to_offset(),
        );
        fence_gate_props.in_wall = args
            .world
            .get_block(&left)
            .has_tag(&tag::Block::MINECRAFT_WALLS)
            || args
                .world
                .get_block(&right)
                .has_tag(&tag::Block::MINECRAFT_WALLS);
        fence_gate_props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let fence_props = is_in_wall(&args);
        fence_props.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        {
            toggle_fence_gate(args.world, args.position, args.player);

            BlockActionResult::Success
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let mut props =
            FenceGateProperties::from_state_id(args.world.get_block_state_id(args.position));
        let powered = block_receives_redstone_power(args.world, args.position);
        if powered != props.powered {
            let changed = props.open != powered;
            props.powered = powered;
            props.open = powered;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
            if changed {
                play_sound(args.world, args.position, args.block, powered, None, None);
            }
        }
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        let mut props = FenceGateProperties::from_state_id(args.state.id);
        if args.can_trigger_blocks && !props.powered {
            props.open = !props.open;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            play_sound(
                args.world,
                args.position,
                args.block,
                props.open,
                None,
                Some(args.state.id),
            );
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        match computation_type {
            PathComputationType::Land | PathComputationType::Air => {
                FenceGateProperties::from_state_id(state.id).open
            }
            PathComputationType::Water => false,
        }
    }
}

fn is_in_wall(args: &GetStateForNeighborUpdateArgs<'_>) -> FenceGateProperties {
    let mut props = FenceGateProperties::from_state_id(args.state_id);
    let side = props.facing.rotate_clockwise().to_offset();
    let axis_matches = if side.x != 0 {
        args.direction == pumpkin_data::BlockDirection::East
            || args.direction == pumpkin_data::BlockDirection::West
    } else {
        args.direction == pumpkin_data::BlockDirection::North
            || args.direction == pumpkin_data::BlockDirection::South
    };
    if axis_matches {
        let other = args.position.offset(args.direction.opposite().to_offset());
        props.in_wall = args
            .neighbor_state_id
            .to_block()
            .has_tag(&tag::Block::MINECRAFT_WALLS)
            || args
                .world
                .get_block(&other)
                .has_tag(&tag::Block::MINECRAFT_WALLS);
    }
    props
}
