use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, ExplodeArgs, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::world::World;
use pumpkin_data::block_properties::Half;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use std::sync::Arc;

type TrapDoorProperties = pumpkin_data::block_properties::OakTrapdoorLikeProperties;

fn get_sound(block: &Block, open: bool) -> Sound {
    let (closed, opened) = if block == &Block::CHERRY_TRAPDOOR {
        (
            Sound::BlockCherryWoodTrapdoorClose,
            Sound::BlockCherryWoodTrapdoorOpen,
        )
    } else if block == &Block::BAMBOO_TRAPDOOR {
        (
            Sound::BlockBambooWoodTrapdoorClose,
            Sound::BlockBambooWoodTrapdoorOpen,
        )
    } else if block == &Block::CRIMSON_TRAPDOOR || block == &Block::WARPED_TRAPDOOR {
        (
            Sound::BlockNetherWoodTrapdoorClose,
            Sound::BlockNetherWoodTrapdoorOpen,
        )
    } else if block.has_tag(&tag::Block::MINECRAFT_WOODEN_TRAPDOORS) {
        (
            Sound::BlockWoodenTrapdoorClose,
            Sound::BlockWoodenTrapdoorOpen,
        )
    } else if block == &Block::IRON_TRAPDOOR {
        (Sound::BlockIronTrapdoorClose, Sound::BlockIronTrapdoorOpen)
    } else {
        (
            Sound::BlockCopperTrapdoorClose,
            Sound::BlockCopperTrapdoorOpen,
        )
    };
    if open { opened } else { closed }
}

fn play_sound(
    world: &Arc<World>,
    pos: &BlockPos,
    block: &Block,
    open: bool,
    player: Option<&Player>,
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
        player.map(|p| p as &dyn EntityBase),
        None,
    );
}

fn toggle(world: &Arc<World>, pos: &BlockPos, player: Option<&Player>) {
    let (block, state) = world.get_block_and_state_id(pos);
    let mut props = TrapDoorProperties::from_state_id(state);
    props.open = !props.open;
    world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
    if props.waterlogged {
        world.schedule_fluid_tick(&Fluid::WATER, *pos, 5, TickPriority::Normal);
    }
    play_sound(world, pos, block, props.open, player);
}

#[pumpkin_block_from_tag("minecraft:trapdoors")]
pub struct TrapDoorBlock;

impl BlockBehaviour for TrapDoorBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if args.block == &Block::IRON_TRAPDOOR {
            return BlockActionResult::Pass;
        }
        toggle(args.world, args.position, Some(args.player));
        BlockActionResult::Success
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        if args.can_trigger_blocks
            && args.block != &Block::IRON_TRAPDOOR
            && !TrapDoorProperties::from_state_id(args.state.id).powered
        {
            toggle(args.world, args.position, None);
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = TrapDoorProperties::default(args.block);
        let clicked = args.direction.opposite();
        if args.use_item_on.position != *args.position && clicked.is_horizontal() {
            props.facing = clicked.to_cardinal_direction();
            props.half = if args.use_item_on.cursor_pos.y > 0.5 {
                Half::Top
            } else {
                Half::Bottom
            };
        } else {
            props.facing = args.player.get_entity().get_horizontal_facing().opposite();
            props.half = if clicked == BlockDirection::Up {
                Half::Bottom
            } else {
                Half::Top
            };
        }
        let (fluid, fluid_state) =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position));
        props.waterlogged = fluid.matches_type(&Fluid::WATER) && fluid_state.is_source;
        props.powered = block_receives_redstone_power(args.world, args.position);
        props.open = props.powered;
        props.to_state_id(args.block)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let mut props =
            TrapDoorProperties::from_state_id(args.world.get_block_state_id(args.position));
        let powered = block_receives_redstone_power(args.world, args.position);
        if powered != props.powered {
            if powered != props.open {
                props.open = powered;
                play_sound(args.world, args.position, args.block, powered, None);
            }
            props.powered = powered;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
            if props.waterlogged {
                args.world.schedule_fluid_tick(
                    &Fluid::WATER,
                    *args.position,
                    5,
                    TickPriority::Normal,
                );
            }
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if TrapDoorProperties::from_state_id(args.state_id).waterlogged {
            args.world
                .schedule_fluid_tick(&Fluid::WATER, *args.position, 5, TickPriority::Normal);
        }
        args.state_id
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        let props = TrapDoorProperties::from_state_id(state.id);
        match computation_type {
            PathComputationType::Land | PathComputationType::Air => props.open,
            PathComputationType::Water => props.waterlogged,
        }
    }
}
