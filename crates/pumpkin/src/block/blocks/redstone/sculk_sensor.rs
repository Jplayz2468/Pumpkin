use std::sync::Arc;

use crate::block::blocks::sculk::vibration::{resonance_event_for_frequency, vibration_frequency};
use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, OnEntityStepArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType,
    PlacedArgs,
};
use crate::world::World;
use pumpkin_data::block_properties::{
    CalibratedSculkSensorLikeProperties, HorizontalFacing, SculkSensorLikeProperties,
    SculkSensorPhase,
};
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::RandomImpl;
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
    pub fn can_activate(world: &World, pos: &BlockPos) -> bool {
        let (block, state) = world.get_block_and_state(pos);
        matches!(
            block.id,
            BlockId::SCULK_SENSOR | BlockId::CALIBRATED_SCULK_SENSOR
        ) && sculk_sensor_phase(block, state.id) == SculkSensorPhase::Inactive
    }

    pub fn can_receive(world: &World, pos: &BlockPos, origin: &BlockPos, event: GameEvent) -> bool {
        if (pos == origin && matches!(event, GameEvent::BlockPlace | GameEvent::BlockDestroy))
            || vibration_frequency(event) == 0
            || !Self::can_activate(world, pos)
        {
            return false;
        }
        let (block, state) = world.get_block_and_state(pos);
        if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            let back = horizontal_facing_to_dir(props.facing).opposite();
            let back_pos = pos.offset(back.to_offset());
            let (back_block, back_state) = world.get_block_and_state(&back_pos);
            // CalibratedSculkSensorBlockEntity.java:38: filter the frequency at
            // reception, before it competes with other candidates, not the power.
            let signal = super::get_redstone_power(back_block, back_state, world, &back_pos, back);
            if signal != 0 && signal != vibration_frequency(event) {
                return false;
            }
        }
        true
    }

    fn update_neighbors(world: &Arc<World>, pos: &BlockPos) {
        world.update_neighbors(pos, None);
        world.update_neighbors(&pos.offset(BlockDirection::Down.to_offset()), None);
    }

    fn clicking_sound(world: &World, pos: &BlockPos, sound: Sound) {
        let pitch = world
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f32()
            * 0.2
            + 0.8;
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            pitch,
        );
    }

    pub fn trigger(
        world: &Arc<World>,
        pos: &BlockPos,
        power: u8,
        frequency: u8,
        source_entity: Option<i32>,
    ) {
        if !Self::can_activate(world, pos) {
            return;
        }
        let (block, state) = world.get_block_and_state(pos);
        let (state_id, waterlogged, active_ticks) = if block.id == BlockId::CALIBRATED_SCULK_SENSOR
        {
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            props.sculk_sensor_phase = SculkSensorPhase::Active;
            props.power = power;
            // CalibratedSculkSensorBlock.getActiveTicks, unlike the ordinary sensor.
            (props.to_state_id(block), props.waterlogged, 10)
        } else {
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            props.sculk_sensor_phase = SculkSensorPhase::Active;
            props.power = power;
            (props.to_state_id(block), props.waterlogged, 30)
        };
        world.set_block_state(pos, state_id, BlockFlags::NOTIFY_ALL);
        world.schedule_block_tick(block, *pos, active_ticks, TickPriority::Normal);
        Self::update_neighbors(world, pos);
        // SculkSensorBlock.tryResonateVibration: each adjacent resonator emits the
        // original frequency. A comparator's frequency is independent of power.
        if let Some(resonance) = resonance_event_for_frequency(frequency) {
            const TONES: [i32; 16] = [0, 0, 2, 4, 6, 7, 9, 10, 12, 14, 15, 18, 19, 21, 22, 24];
            let pitch = 2.0_f32.powf((TONES[usize::from(frequency)] - 12) as f32 / 12.0);
            for direction in BlockDirection::all() {
                let adjacent = pos.offset(direction.to_offset());
                if world
                    .get_block(&adjacent)
                    .has_tag(&pumpkin_data::tag::Block::MINECRAFT_VIBRATION_RESONATORS)
                {
                    world.emit_game_event_with_source(
                        resonance.name(),
                        adjacent.to_centered_f64(),
                        source_entity,
                    );
                    world.play_sound_fine(
                        Sound::BlockAmethystBlockResonate,
                        SoundCategory::Blocks,
                        &adjacent.to_centered_f64(),
                        1.0,
                        pitch,
                    );
                }
            }
        }
        world.emit_game_event_with_source(
            GameEvent::SculkSensorTendrilsClicking.name(),
            pos.to_centered_f64(),
            source_entity,
        );
        if !waterlogged {
            Self::clicking_sound(world, pos, Sound::BlockSculkSensorClicking);
        }
    }
}

impl BlockBehaviour for SculkSensorBlock {
    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let waterlogged = args.block.is_waterlogged(args.state_id);
        crate::block::blocks::schedule_waterlogged_tick(args.world, args.position, waterlogged);
        args.state_id
    }

    fn on_state_replaced(&self, args: crate::block::OnStateReplacedArgs<'_>) {
        if sculk_sensor_phase(args.block, args.old_state_id) == SculkSensorPhase::Active {
            Self::update_neighbors(args.world, args.position);
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::default(args.block);
            props.facing = args.player.living_entity.entity.get_horizontal_facing();
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        } else {
            let mut props = SculkSensorLikeProperties::default(args.block);
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.world.get_block_entity(args.position).is_some() {
            return;
        }
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
            if props.sculk_sensor_phase == SculkSensorPhase::Active
                && args.direction != horizontal_facing_to_dir(props.facing)
            {
                props.power
            } else {
                0
            }
        } else {
            0
        }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.direction == BlockDirection::Up {
            self.get_weak_redstone_power(args)
        } else {
            0
        }
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        // SculkSensorBlock.stepOn deliberately bypasses sneaking and occlusion.
        if args.entity.get_entity().entity_type == &pumpkin_data::entity::EntityType::WARDEN
            || !Self::can_receive(args.world, args.position, args.position, GameEvent::Step)
        {
            return;
        }
        if let Some(be) = args.world.get_block_entity(args.position)
            && let Some(listener) = be.as_vibration_listener()
        {
            let time = args
                .world
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .world_age as u64;
            listener.force_vibration(
                args.world,
                GameEvent::Step,
                args.entity.get_entity().pos.load(),
                Some(args.entity.get_entity().entity_id),
                time,
            );
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
                    Self::update_neighbors(args.world, args.position);
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    if !props.waterlogged {
                        Self::clicking_sound(
                            args.world,
                            args.position,
                            Sound::BlockSculkSensorClickingStop,
                        );
                    }
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
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
                    Self::update_neighbors(args.world, args.position);
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    if !props.waterlogged {
                        Self::clicking_sound(
                            args.world,
                            args.position,
                            Sound::BlockSculkSensorClickingStop,
                        );
                    }
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                SculkSensorPhase::Inactive => {}
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
