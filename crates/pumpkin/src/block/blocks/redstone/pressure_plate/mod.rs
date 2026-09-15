use std::sync::Arc;

use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos};
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

use crate::{
    block::{OnEntityCollisionArgs, OnStateReplacedArgs},
    world::World,
};

pub mod plate;
pub mod weighted;

#[cfg(test)]
mod tests;

// Vanilla pressure plates detect entities in a centered 14x4x14-pixel volume.
const PRESSURE_PLATE_DETECTION_BOX: BoundingBox = BoundingBox::new_array(
    [1.0 / 16.0, 0.0, 1.0 / 16.0],
    [15.0 / 16.0, 4.0 / 16.0, 15.0 / 16.0],
);

fn detection_box_at(pos: &BlockPos) -> BoundingBox {
    PRESSURE_PLATE_DETECTION_BOX.at_pos(*pos)
}

pub fn get_pressure_plate_sounds(block: &Block) -> (Sound, Sound) {
    if block == &Block::LIGHT_WEIGHTED_PRESSURE_PLATE
        || block == &Block::HEAVY_WEIGHTED_PRESSURE_PLATE
    {
        (
            Sound::BlockMetalPressurePlateClickOn,
            Sound::BlockMetalPressurePlateClickOff,
        )
    } else if block == &Block::STONE_PRESSURE_PLATE
        || block == &Block::POLISHED_BLACKSTONE_PRESSURE_PLATE
    {
        (
            Sound::BlockStonePressurePlateClickOn,
            Sound::BlockStonePressurePlateClickOff,
        )
    } else if block == &Block::BAMBOO_PRESSURE_PLATE {
        (
            Sound::BlockBambooWoodPressurePlateClickOn,
            Sound::BlockBambooWoodPressurePlateClickOff,
        )
    } else if block == &Block::CHERRY_PRESSURE_PLATE {
        (
            Sound::BlockCherryWoodPressurePlateClickOn,
            Sound::BlockCherryWoodPressurePlateClickOff,
        )
    } else if block == &Block::CRIMSON_PRESSURE_PLATE || block == &Block::WARPED_PRESSURE_PLATE {
        (
            Sound::BlockNetherWoodPressurePlateClickOn,
            Sound::BlockNetherWoodPressurePlateClickOff,
        )
    } else {
        (
            Sound::BlockWoodenPressurePlateClickOn,
            Sound::BlockWoodenPressurePlateClickOff,
        )
    }
}

pub(crate) trait PressurePlate {
    fn on_entity_collision_pp(&self, args: OnEntityCollisionArgs<'_>) {
        let output = self.get_redstone_output(args.block, args.state.id);
        if output == 0 {
            self.update_plate_state(
                args.world,
                args.position,
                args.block,
                args.state,
                output,
                Some(args.entity),
            );
        }
    }

    fn on_state_replaced_pp(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved && self.get_redstone_output(args.block, args.old_state_id) > 0 {
            args.world
                .update_neighbors_at(args.position, args.block, None);
            args.world
                .update_neighbors_at(&args.position.down(), args.block, None);
        }
    }

    fn update_plate_state(
        &self,
        world: &Arc<World>,
        pos: &BlockPos,
        block: &Block,
        state: &BlockState,
        output: u8,
        source: Option<&dyn crate::entity::EntityBase>,
    ) {
        let calc_output = self.calculate_redstone_output(world, block, pos);
        let mut has_output = calc_output > 0;
        let was_pressed = output > 0;
        if calc_output != output {
            let next_output = if let Some(server) = world.server.upgrade() {
                let mut event = crate::plugin::block::block_redstone::BlockRedstoneEvent::new(
                    world.clone(),
                    state.id,
                    *pos,
                    i32::from(output),
                    i32::from(calc_output),
                );
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return;
                }
                event.new_current.clamp(0, 15) as u8
            } else {
                calc_output
            };
            let state = self.set_redstone_output(block, state, next_output);
            has_output = self.get_redstone_output(block, state) > 0;
            world.set_block_state(pos, state, BlockFlags::NOTIFY_LISTENERS);
            world.update_neighbors_at(pos, block, None);
            world.update_neighbors_at(&pos.down(), block, None);
        }

        let (click_on, click_off) = get_pressure_plate_sounds(block);
        if !has_output && was_pressed {
            world.play_block_sound(click_off, SoundCategory::Blocks, *pos);
            world.emit_game_event_from_entity(
                GameEvent::BlockDeactivate.name(),
                pos.to_centered_f64(),
                source,
                None,
            );
        } else if has_output && !was_pressed {
            world.play_block_sound(click_on, SoundCategory::Blocks, *pos);
            world.emit_game_event_from_entity(
                GameEvent::BlockActivate.name(),
                pos.to_centered_f64(),
                source,
                None,
            );
        }

        if has_output {
            world.schedule_block_tick(block, *pos, self.tick_rate(), TickPriority::Normal);
        }
    }

    fn can_pressure_plate_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        let floor = world.get_block_state(&block_pos.down());
        floor.is_center_solid(BlockDirection::Up)
            || [
                [0.0, 0.125, 0.0, 1.0],
                [0.875, 1.0, 0.0, 1.0],
                [0.125, 0.875, 0.0, 0.125],
                [0.125, 0.875, 0.875, 1.0],
            ]
            .into_iter()
            .all(|region| floor.collision_face_covers(block_pos.down(), BlockDirection::Up, region))
    }

    fn get_redstone_output(&self, block: &Block, state: BlockStateId) -> u8;

    fn set_redstone_output(&self, block: &Block, state: &BlockState, output: u8) -> BlockStateId;

    fn calculate_redstone_output(&self, world: &World, block: &Block, pos: &BlockPos) -> u8;

    fn tick_rate(&self) -> u32 {
        20
    }
}
