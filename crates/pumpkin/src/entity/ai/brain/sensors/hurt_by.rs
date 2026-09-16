use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::memory::MemoryValue;
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::brain::sensor::{Sensor, SensorContext};
use std::sync::atomic::Ordering::Relaxed;

/// Vanilla's `getLastDamageSource` clears itself after 40 ticks, and `HurtBySensor`
/// republishes whatever survives that window every scan.
const DAMAGE_MEMORY_TICKS: i32 = 40;

/// Port of `HurtBySensor`.
///
/// Publishes the damage the mob last took, and who dealt it, so `AnimalPanic` and the
/// target-selection behaviours can react without reaching into the entity themselves.
pub struct HurtBySensor;

impl Sensor<MobActor> for HurtBySensor {
    /// Vanilla scans this one every tick, not on the default rate: a mob must react to
    /// being hit immediately, not up to a second later.
    fn scan_rate(&self) -> i32 {
        1
    }

    fn do_tick(&mut self, ctx: &mut SensorContext<'_, MobActor>) {
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let Some(living) = entity.get_living_entity() else {
            return;
        };

        let age = living.entity.tick_count.load(Relaxed);
        let last = living.last_damage_time.load(Relaxed);
        let fresh = last != 0 && age - last < DAMAGE_MEMORY_TICKS;

        match (fresh, living.last_damage_type.load()) {
            (true, Some(damage_type)) => {
                ctx.memories
                    .set(MemoryModuleType::HurtBy, MemoryValue::DamageType(damage_type));
                let attacker = living.last_attacker_id.load(Relaxed);
                if attacker != 0 {
                    ctx.memories
                        .set(MemoryModuleType::HurtByEntity, MemoryValue::EntityId(attacker));
                }
            }
            _ => {
                ctx.memories.erase(MemoryModuleType::HurtBy);
                ctx.memories.erase(MemoryModuleType::HurtByEntity);
            }
        }
    }

    fn requires(&self) -> &[MemoryModuleType] {
        &[MemoryModuleType::HurtBy, MemoryModuleType::HurtByEntity]
    }

    fn debug_name(&self) -> &'static str {
        "hurt_by"
    }
}
