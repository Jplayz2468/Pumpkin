use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use uuid::Uuid;

use crate::world::World;

use super::BlockEntity;

pub struct CreakingHeartBlockEntity {
    pub position: BlockPos,
    pub creaking_uuid: AtomicCell<Option<Uuid>>,
    /// Vanilla `CreakingHeartBlockEntity.outputSignal` (`:65`): recomputed each tick and
    /// only pushed to neighbours when it changes, so a comparator is notified as the
    /// bound creaking moves instead of having to re-read on its own.
    output_signal: AtomicCell<u8>,
}

impl BlockEntity for CreakingHeartBlockEntity {
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
        let creaking_uuid = nbt
            .get_uuid("creaking")
            .or_else(|| {
                nbt.get_string("creaking")
                    .and_then(|uuid_str| Uuid::parse_str(uuid_str).ok())
            })
            .or_else(|| {
                nbt.get_string("creaking_uuid")
                    .and_then(|uuid_str| Uuid::parse_str(uuid_str).ok())
            });
        Self {
            position,
            creaking_uuid: AtomicCell::new(creaking_uuid),
            output_signal: AtomicCell::new(0),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(uuid) = self.creaking_uuid.load() {
            nbt.put_uuid("creaking", uuid);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Some(uuid) = self.creaking_uuid.load() {
            nbt.put_uuid("creaking", uuid);
        }
        Some(nbt)
    }

    /// Vanilla `CreakingHeartBlockEntity.serverTick` (`:71`) recomputes the signal every
    /// tick and notifies neighbours only on a change.
    fn tick(&self, world: &Arc<World>) {
        let computed = self.compute_analog_output_signal(world);
        if self.output_signal.load() != computed {
            self.output_signal.store(computed);
            world.update_neighbour_for_output_signal(
                &self.position,
                &pumpkin_data::Block::CREAKING_HEART,
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl CreakingHeartBlockEntity {
    pub const ID: &'static str = "minecraft:creaking_heart";

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            creaking_uuid: AtomicCell::new(None),
            output_signal: AtomicCell::new(0),
        }
    }

    /// Vanilla `CreakingHeartBlockEntity.getAnalogOutputSignal` (`:334`) -- the cached
    /// value, not a fresh computation.
    #[must_use]
    pub fn analog_output_signal(&self) -> u8 {
        self.output_signal.load()
    }

    /// Vanilla `computeAnalogOutputSignal` (`:338`):
    ///
    /// ```java
    /// double distance = this.distanceToCreaking();
    /// double scaled = Math.clamp(distance, 0.0, 32.0) / 32.0;
    /// return 15 - (int)Math.floor(scaled * 15.0);
    /// ```
    ///
    /// Zero when no creaking is bound. `distanceToCreaking` measures from
    /// `Vec3.atBottomCenterOf(pos)` (`:150`), i.e. the block's bottom face centre.
    #[must_use]
    pub fn compute_analog_output_signal(&self, world: &Arc<World>) -> u8 {
        let Some(uuid) = self.creaking_uuid.load() else {
            return 0;
        };
        let Some(creaking) = world.get_entity_by_uuid(uuid) else {
            return 0;
        };

        let heart = self.position.to_f64();
        let bottom_centre = pumpkin_util::math::vector3::Vector3::new(
            heart.x + 0.5,
            self.position.0.y as f64,
            heart.z + 0.5,
        );
        let creaking_pos = creaking.get_entity().pos.load();
        let distance = creaking_pos.sub(&bottom_centre).length();

        Self::signal_for_distance(distance)
    }

    /// The distance-to-signal curve on its own, so it can be checked without a world.
    #[must_use]
    pub fn signal_for_distance(distance: f64) -> u8 {
        let scaled = distance.clamp(0.0, 32.0) / 32.0;
        15 - (scaled * 15.0).floor() as u8
    }

    pub fn is_protector(&self, creaking_uuid: Uuid) -> bool {
        self.creaking_uuid
            .load()
            .is_none_or(|uuid| uuid == creaking_uuid)
    }

    pub fn set_creaking_uuid(&self, uuid: Option<Uuid>) {
        self.creaking_uuid.store(uuid);
    }

    pub fn creaking_hurt(&self, world: &Arc<World>) {
        world.play_sound(
            Sound::BlockCreakingHeartHurt,
            SoundCategory::Blocks,
            &self.position.to_f64(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vanilla `CreakingHeartBlockEntity.computeAnalogOutputSignal` (`:338`):
    /// `15 - floor(clamp(distance, 0, 32) / 32 * 15)`.
    #[test]
    fn signal_matches_vanilla_distance_curve() {
        // Touching the heart is full strength.
        assert_eq!(CreakingHeartBlockEntity::signal_for_distance(0.0), 15);
        // At and beyond the 32-block cap the term is exactly 15, leaving zero.
        assert_eq!(CreakingHeartBlockEntity::signal_for_distance(32.0), 0);
        assert_eq!(CreakingHeartBlockEntity::signal_for_distance(100.0), 0);
        // Halfway: floor(0.5 * 15) = 7, so 15 - 7 = 8.
        assert_eq!(CreakingHeartBlockEntity::signal_for_distance(16.0), 8);
        // The curve never rises with distance.
        let mut previous = 16u8;
        for step in 0..=32 {
            let signal = CreakingHeartBlockEntity::signal_for_distance(f64::from(step));
            assert!(signal <= previous, "signal rose at distance {step}");
            previous = signal;
        }
    }
}
