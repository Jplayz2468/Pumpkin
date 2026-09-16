use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::memory::MemoryValue;
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::brain::sensor::{Sensor, SensorContext};

/// Port of `IsInWaterSensor`.
///
/// Publishes whether the mob is in water, which the frog's activity list gates on to
/// choose between its Swim and Idle activities.
pub struct IsInWaterSensor;

impl Sensor<MobActor> for IsInWaterSensor {
    /// Every tick: a frog entering water should switch activity immediately, not up to a
    /// second later.
    fn scan_rate(&self) -> i32 {
        1
    }

    fn do_tick(&mut self, ctx: &mut SensorContext<'_, MobActor>) {
        let in_water = ctx.actor.entity().is_some_and(|entity| {
            entity
                .get_entity()
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
        });
        if in_water {
            ctx.memories
                .set(MemoryModuleType::IsInWater, MemoryValue::Unit);
        } else {
            ctx.memories.erase(MemoryModuleType::IsInWater);
        }
    }

    fn requires(&self) -> &[MemoryModuleType] {
        &[MemoryModuleType::IsInWater]
    }

    fn debug_name(&self) -> &'static str {
        "is_in_water"
    }
}
