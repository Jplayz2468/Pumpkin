use super::BlockEntity;
use crate::block::blocks::sculk::vibration::{VibrationData, VibrationListener, VibrationUser};
use crate::world::World;
use pumpkin_data::game_event::GameEvent;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;
use std::sync::Mutex;

pub struct CalibratedSculkSensorBlockEntity {
    pub position: BlockPos,
    pub listener: Mutex<VibrationData>,
    pub last_vibration_frequency: Mutex<i32>,
}

impl BlockEntity for CalibratedSculkSensorBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let last_vibration_frequency = nbt.get_int("last_vibration_frequency").unwrap_or(0);
        Self {
            position,
            listener: Mutex::new(VibrationData::from_nbt(nbt)),
            last_vibration_frequency: Mutex::new(last_vibration_frequency),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.listener
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);
        if let Ok(freq) = self.last_vibration_frequency.lock() {
            nbt.put_int("last_vibration_frequency", *freq);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_int(
            "last_vibration_frequency",
            *self.last_vibration_frequency.try_lock().ok()?,
        );
        Some(nbt)
    }

    fn tick(&self, world: &Arc<World>) {
        let time = world
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age as u64;
        self.tick_vibration(world, time);
    }

    fn as_vibration_listener(&self) -> Option<&dyn VibrationListener> {
        Some(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CalibratedSculkSensorBlockEntity {
    pub const ID: &'static str = "minecraft:calibrated_sculk_sensor";
    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            listener: Mutex::new(VibrationData::default()),
            last_vibration_frequency: Mutex::new(0),
        }
    }
}

impl VibrationListener for CalibratedSculkSensorBlockEntity {
    fn vibration_data(&self) -> &Mutex<VibrationData> {
        &self.listener
    }
    fn as_vibration_user(&self) -> &dyn VibrationUser {
        self
    }
}

impl VibrationUser for CalibratedSculkSensorBlockEntity {
    fn listener_radius(&self) -> i32 {
        16
    }
    fn listener_pos(&self) -> BlockPos {
        self.position
    }
    fn can_receive_vibration(
        &self,
        world: &Arc<World>,
        origin: &BlockPos,
        event: GameEvent,
        _source_entity: Option<i32>,
    ) -> bool {
        crate::block::blocks::redstone::sculk_sensor::SculkSensorBlock::can_receive(
            world,
            &self.position,
            origin,
            event,
        )
    }
    fn on_receive_vibration(
        &self,
        world: &Arc<World>,
        _origin: &BlockPos,
        event: GameEvent,
        source_entity: Option<i32>,
        distance: f32,
    ) {
        use crate::block::blocks::redstone::sculk_sensor::SculkSensorBlock;
        use crate::block::blocks::sculk::vibration::{
            redstone_strength_for_distance, vibration_frequency,
        };
        // SculkSensorBlockEntity.VibrationUser.onReceiveVibration: recheck phase,
        // but do not recheck a calibrated input that changed during travel.
        if !SculkSensorBlock::can_activate(world, &self.position) {
            return;
        }
        let frequency = vibration_frequency(event);
        *self
            .last_vibration_frequency
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = i32::from(frequency);
        SculkSensorBlock::trigger(
            world,
            &self.position,
            redstone_strength_for_distance(distance, self.listener_radius()),
            frequency,
            source_entity,
        );
    }
}
