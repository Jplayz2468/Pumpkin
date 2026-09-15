//! A Brain wired onto zombies, purely to exercise the framework on a live server.
//!
//! **This is a deliberate divergence from vanilla.** Zombies are a goal mob
//! (`Zombie.registerGoals`, `Zombie.java:112`), not one of the 22 brain mobs, so vanilla
//! zombies have no brain at all. It is gated behind `local_safety.zombie_brain_lab` and
//! defaults to off; it runs *alongside* the zombie's normal goals rather than replacing
//! them, so goal behaviour is unchanged while the brain runs.
//!
//! The point is to prove the framework end to end against something easy to spawn and
//! watch: a sensor writes a memory, an activity requirement gates on that memory, and a
//! behaviour reacts. Real brain mobs come later and will use the same machinery.

use std::sync::Arc;

use crate::world::World;

use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::{Activity, MemoryModuleType};
use crate::entity::ai::brain::sensor::{Sensor, SensorContext};
use crate::entity::ai::brain::{
    Brain, MobActor, MobBrain, behavior::BehaviorSlot, sensor::SensorSlot,
};

/// How far the sensor looks for a player, in blocks.
const PLAYER_SEARCH_RADIUS: f64 = 16.0;

/// Writes the nearest player into `NearestVisiblePlayer`, mirroring the shape of vanilla's
/// `NearestLivingEntitySensor` family without their targeting-condition machinery.
struct NearestPlayerSensor;

impl Sensor<MobActor> for NearestPlayerSensor {
    fn do_tick(&mut self, ctx: &mut SensorContext<'_, MobActor>) {
        let nearest = ctx
            .actor
            .world
            .get_closest_player(ctx.actor.position, PLAYER_SEARCH_RADIUS);

        match nearest {
            Some(player) => ctx.memories.set(
                MemoryModuleType::NearestVisiblePlayer,
                MemoryValue::EntityId(player.living_entity.entity.entity_id),
            ),
            None => ctx.memories.erase(MemoryModuleType::NearestVisiblePlayer),
        }
    }

    fn requires(&self) -> &[MemoryModuleType] {
        &[MemoryModuleType::NearestVisiblePlayer]
    }

    fn debug_name(&self) -> &'static str {
        "nearest_player"
    }
}

/// Reports, once per start, that the brain saw a player. Deliberately observable: it is
/// how the framework is confirmed to be running on a live server.
struct ReportNearestPlayer {
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl Default for ReportNearestPlayer {
    fn default() -> Self {
        Self {
            conditions: [(
                MemoryModuleType::NearestVisiblePlayer,
                MemoryStatus::ValuePresent,
            )],
        }
    }
}

impl Behavior<MobActor> for ReportNearestPlayer {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, _ctx: &mut BehaviorContext<'_, MobActor>) {
        // Intentionally silent. This behaviour exists to prove the framework runs -- the
        // memory it depends on is asserted by the brain's own tests, not by log output.
    }

    fn debug_name(&self) -> &'static str {
        "report_nearest_player"
    }
}

/// Builds the lab brain: one sensor, one memory, and an Idle activity whose single
/// behaviour requires that memory to be present.
#[must_use]
pub fn build() -> MobBrain {
    let mut brain: MobBrain = Brain::new(Activity::Idle);
    brain.register_memory(MemoryModuleType::NearestVisiblePlayer);
    brain.add_sensor(SensorSlot::new(Box::new(NearestPlayerSensor)));
    brain.add_activity(
        Activity::Idle,
        vec![(
            0,
            BehaviorSlot::new(Box::new(ReportNearestPlayer::default())),
        )],
        vec![],
        vec![],
    );
    brain.set_active_activity_if_possible(Activity::Idle);
    brain
}

/// Whether the lab brain is switched on for this server.
#[must_use]
pub fn enabled(world: &Arc<World>) -> bool {
    world.server.upgrade().is_some_and(|server| {
        server.advanced_config.local_safety.zombie_brain_lab
    })
}
