use std::sync::Arc;

use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::block_properties::{
    CalibratedSculkSensorLikeProperties, HorizontalFacing, SculkSensorLikeProperties,
    SculkSensorPhase,
};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

pub struct SculkSensorBlock;

impl BlockMetadata for SculkSensorBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_SENSOR, BlockId::CALIBRATED_SCULK_SENSOR].into()
    }
}

const fn horizontal_facing_to_dir(facing: HorizontalFacing) -> BlockDirection {
    match facing {
        HorizontalFacing::North => BlockDirection::North,
        HorizontalFacing::South => BlockDirection::South,
        HorizontalFacing::West => BlockDirection::West,
        HorizontalFacing::East => BlockDirection::East,
    }
}

/// Both sensor variants carry the same phase property under different types.
fn sculk_sensor_phase(block: &Block, state_id: BlockStateId) -> SculkSensorPhase {
    if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
        CalibratedSculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    } else {
        SculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    }
}

impl SculkSensorBlock {
    pub(crate) fn can_receive(world: &World, pos: &BlockPos, event: &str, frequency: i32, source: [i32; 3]) -> bool {
        let state = world.get_block_state(pos);
        let block = Block::from_state_id(state.id);
        if !matches!(block.id, BlockId::SCULK_SENSOR | BlockId::CALIBRATED_SCULK_SENSOR) { return false; }
        if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            let direction = horizontal_facing_to_dir(props.facing).opposite();
            let back = pos.offset(direction.to_offset());
            let state = world.get_block_state(&back);
            let signal = crate::block::blocks::redstone::get_redstone_power(Block::from_state_id(state.id), state, world, &back, direction);
            if signal != 0 && i32::from(signal) != frequency { return false; }
        }
        !(matches!(event, "block_destroy" | "block_place") && source == [pos.0.x, pos.0.y, pos.0.z])
            && frequency != 0 && sculk_sensor_phase(block, state.id) == SculkSensorPhase::Inactive
    }

    pub(crate) fn trigger(world: &Arc<World>, pos: &BlockPos, frequency: i32, distance: f32, source: Option<&dyn crate::entity::EntityBase>) {
        let state = world.get_block_state(pos);
        let block = Block::from_state_id(state.id);
        if !matches!(block.id, BlockId::SCULK_SENSOR | BlockId::CALIBRATED_SCULK_SENSOR)
            || sculk_sensor_phase(block, state.id) != SculkSensorPhase::Inactive { return; }
        let calibrated = block.id == BlockId::CALIBRATED_SCULK_SENSOR;
        let radius = if calibrated { 16 } else { 8 };
        let power = (15 - ((15.0 / f64::from(radius)) * f64::from(distance)).floor() as i32).max(1) as u8;
        if let Some(be) = world.get_block_entity(pos) {
            let frequency_field = if let Some(be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>() { Some(&be.last_vibration_frequency) }
                else { be.as_any().downcast_ref::<CalibratedSculkSensorBlockEntity>().map(|be| &be.last_vibration_frequency) };
            if let Some(field) = frequency_field { *field.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = frequency; }
        }
        let (state_id, waterlogged) = if calibrated {
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            props.sculk_sensor_phase = SculkSensorPhase::Active; props.power = power;
            (props.to_state_id(block), props.waterlogged)
        } else {
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            props.sculk_sensor_phase = SculkSensorPhase::Active; props.power = power;
            (props.to_state_id(block), props.waterlogged)
        };
        world.set_block_state(pos, state_id, BlockFlags::NOTIFY_ALL);
        world.schedule_block_tick(block, *pos, if calibrated { 10 } else { 30 }, TickPriority::Normal);
        Self::update_neighbors(world, pos);
        Self::resonate(world, pos, frequency, source);
        world.emit_game_event_from("minecraft:sculk_sensor_tendrils_clicking", pos.to_centered_f64(), source, None);
        if !waterlogged {
            world.play_sound_fine(pumpkin_data::sound::Sound::BlockSculkSensorClicking, pumpkin_data::sound::SoundCategory::Blocks, &pos.to_centered_f64(), 1.0, rand::random::<f32>() * 0.2 + 0.8);
        }
    }

    fn update_neighbors(world: &Arc<World>, pos: &BlockPos) {
        world.update_neighbors(pos, None);
        world.update_neighbors(&pos.offset(BlockDirection::Down.to_offset()), None);
    }

    fn resonate(world: &World, pos: &BlockPos, frequency: i32, source: Option<&dyn crate::entity::EntityBase>) {
        use pumpkin_data::tag::Taggable;
        const NOTES: [i32; 16] = [0, 0, 2, 4, 6, 7, 9, 10, 12, 14, 15, 18, 19, 21, 22, 24];
        for direction in [BlockDirection::Down, BlockDirection::Up, BlockDirection::North, BlockDirection::South, BlockDirection::West, BlockDirection::East] {
            let adjacent = pos.offset(direction.to_offset());
            let state = world.get_block_state(&adjacent);
            if Block::from_state_id(state.id).is_tagged_with("minecraft:vibration_resonators").unwrap_or(false) {
                world.emit_game_event_from(format!("minecraft:resonate_{frequency}"), adjacent.to_centered_f64(), source, Some(state.id));
                let pitch = 2.0_f64.powf(f64::from(NOTES[frequency as usize] - 12) / 12.0) as f32;
                world.play_sound_fine(pumpkin_data::sound::Sound::BlockAmethystBlockResonate, pumpkin_data::sound::SoundCategory::Blocks, &adjacent.to_centered_f64(), 1.0, pitch);
            }
        }
    }

}

impl BlockBehaviour for SculkSensorBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::default(args.block);
            props.facing = args.player.living_entity.entity.get_horizontal_facing();
            props.to_state_id(args.block)
        } else {
            let props = SculkSensorLikeProperties::default(args.block);
            props.to_state_id(args.block)
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let entity = CalibratedSculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        } else if args.block.id == BlockId::SCULK_SENSOR {
            let entity = SculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.block.id == BlockId::SCULK_SENSOR {
            let props = SculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active {
                props.power
            } else {
                0
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let props = CalibratedSculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active {
                props.power
            } else {
                0
            }
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // Vanilla reads the frequency only while the sensor is active.
        if sculk_sensor_phase(args.block, args.state.id) != SculkSensorPhase::Active {
            return Some(0);
        }

        let be = args.world.get_block_entity(args.position)?;
        if let Some(sensor_be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>() {
            return Some(
                *sensor_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        if let Some(cal_be) = be
            .as_any()
            .downcast_ref::<CalibratedSculkSensorBlockEntity>()
        {
            return Some(
                *cal_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        None
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if args.block.id == BlockId::SCULK_SENSOR {
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                    Self::update_neighbors(args.world, args.position);
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    if !props.waterlogged {
                        args.world.play_sound_fine(pumpkin_data::sound::Sound::BlockSculkSensorClickingStop, pumpkin_data::sound::SoundCategory::Blocks, &args.position.to_centered_f64(), 1.0, rand::random::<f32>() * 0.2 + 0.8);
                    }
                }
                SculkSensorPhase::Inactive => {}
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                    Self::update_neighbors(args.world, args.position);
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    if !props.waterlogged {
                        args.world.play_sound_fine(pumpkin_data::sound::Sound::BlockSculkSensorClickingStop, pumpkin_data::sound::SoundCategory::Blocks, &args.position.to_centered_f64(), 1.0, rand::random::<f32>() * 0.2 + 0.8);
                    }
                }
                SculkSensorPhase::Inactive => {}
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
