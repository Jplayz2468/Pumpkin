use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_util::math::vector3::Vector3;

/// Port of `RandomLookAround`.
///
/// Picks a random direction to look and sets a gaze cooldown, which is what stops a mob's
/// head twitching every tick: the cooldown memory blocks this behaviour until it expires.
pub struct RandomLookAround {
    interval: (i32, i32),
    max_yaw: f32,
    min_pitch: f32,
    pitch_range: f32,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl RandomLookAround {
    #[must_use]
    pub fn new(interval: (i32, i32), max_yaw: f32, min_pitch: f32, max_pitch: f32) -> Self {
        assert!(min_pitch <= max_pitch, "minimum pitch exceeds maximum pitch");
        Self {
            interval,
            max_yaw,
            min_pitch,
            pitch_range: max_pitch - min_pitch,
            conditions: [
                (MemoryModuleType::LookTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::GazeCooldownTicks, MemoryStatus::ValueAbsent),
            ],
        }
    }
}

impl Behavior<MobActor> for RandomLookAround {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let entity = entity.get_entity();

        let pitch =
            (rand::random::<f32>() * self.pitch_range + self.min_pitch).clamp(-90.0, 90.0);
        let yaw = entity.yaw.load() + 2.0 * rand::random::<f32>() * self.max_yaw - self.max_yaw;
        let yaw = pumpkin_util::math::wrap_degrees(yaw);

        // Vec3.directionFromRotation.
        let (pitch_rad, yaw_rad) = (pitch.to_radians(), yaw.to_radians());
        let direction = Vector3::new(
            f64::from(-yaw_rad.sin() * pitch_rad.cos()),
            f64::from(-pitch_rad.sin()),
            f64::from(yaw_rad.cos() * pitch_rad.cos()),
        );

        let eye = entity.pos.load() + Vector3::new(0.0, f64::from(entity.height()), 0.0);
        ctx.memories
            .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(eye + direction));

        let (min, max) = self.interval;
        let cooldown = min + rand::random_range(0..=(max - min).max(0));
        ctx.memories
            .set(MemoryModuleType::GazeCooldownTicks, MemoryValue::Int(cooldown));
    }

    fn debug_name(&self) -> &'static str {
        "random_look_around"
    }
}
