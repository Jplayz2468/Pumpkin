use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::tag::Taggable;
use pumpkin_util::math::position::BlockPos;

use crate::world::World;

pub mod coral_block;
pub mod coral_fan;
pub mod coral_plant;

pub fn scan_for_water(world: &World, pos: &BlockPos) -> bool {
    for direction in BlockDirection::all() {
        let neighbor_pos = pos.offset(direction.to_offset());
        let block = world.get_fluid(&neighbor_pos);
        if block.has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER) {
            return true;
        }
    }
    false
}
fn is_dead_coral(block: &Block) -> bool {
    block == &Block::DEAD_BRAIN_CORAL
        || block == &Block::DEAD_BUBBLE_CORAL
        || block == &Block::DEAD_FIRE_CORAL
        || block == &Block::DEAD_HORN_CORAL
        || block == &Block::DEAD_TUBE_CORAL
        || block == &Block::DEAD_BRAIN_CORAL_BLOCK
        || block == &Block::DEAD_BUBBLE_CORAL_BLOCK
        || block == &Block::DEAD_FIRE_CORAL_BLOCK
        || block == &Block::DEAD_HORN_CORAL_BLOCK
        || block == &Block::DEAD_TUBE_CORAL_BLOCK
        || block == &Block::DEAD_BRAIN_CORAL_FAN
        || block == &Block::DEAD_BRAIN_CORAL_WALL_FAN
        || block == &Block::DEAD_BUBBLE_CORAL_FAN
        || block == &Block::DEAD_BUBBLE_CORAL_WALL_FAN
        || block == &Block::DEAD_FIRE_CORAL_FAN
        || block == &Block::DEAD_FIRE_CORAL_WALL_FAN
        || block == &Block::DEAD_HORN_CORAL_FAN
        || block == &Block::DEAD_HORN_CORAL_WALL_FAN
        || block == &Block::DEAD_TUBE_CORAL_FAN
        || block == &Block::DEAD_TUBE_CORAL_WALL_FAN
}
pub fn try_schedule_die_tick(block: &Block, world: &World, pos: &BlockPos) {
    let tick_delay = 60 + world.rand_bounded_i32(40);
    world.schedule_block_tick(
        block,
        *pos,
        tick_delay as u32,
        pumpkin_world::tick::TickPriority::Normal,
    );
}

fn scan_for_waterlogged_coral(
    world: &World,
    pos: &BlockPos,
    state: pumpkin_data::BlockStateId,
) -> bool {
    state.to_state().is_waterlogged() || scan_for_water(world, pos)
}

fn placement_waterlogged(world: &World, pos: &BlockPos) -> bool {
    let (fluid, state) = World::fluid_state_from_block_state(world.get_block_state_id(pos));
    fluid.has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER) && state.level == 8
}

fn schedule_water_tick(world: &World, pos: &BlockPos, state: pumpkin_data::BlockStateId) {
    if state.to_state().is_waterlogged() {
        world.schedule_fluid_tick(
            &pumpkin_data::fluid::Fluid::WATER,
            *pos,
            pumpkin_data::fluid::Fluid::WATER.flow_speed as u32,
            pumpkin_world::tick::TickPriority::Normal,
        );
    }
}
