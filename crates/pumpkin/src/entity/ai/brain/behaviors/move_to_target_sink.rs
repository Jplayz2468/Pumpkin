use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryMap, MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::pathfinder::NavigatorGoal;
use pumpkin_util::math::vector3::Vector3;

/// Reads the `walk_target` memory, if it holds one.
#[must_use]
pub fn walk_target(memories: &MemoryMap) -> Option<(Vector3<f64>, f32, i32)> {
    match memories.get(MemoryModuleType::WalkTarget) {
        Some(MemoryValue::WalkTarget {
            destination,
            speed,
            close_enough,
        }) => Some((*destination, *speed, *close_enough)),
        _ => None,
    }
}

/// Writes the `walk_target` memory. Vanilla's `WalkTarget` default close-enough distance
/// is one block.
pub fn set_walk_target(memories: &mut MemoryMap, destination: Vector3<f64>, speed: f32) {
    memories.set(
        MemoryModuleType::WalkTarget,
        MemoryValue::WalkTarget {
            destination,
            speed,
            close_enough: 1,
        },
    );
}

/// Port of `MoveToTargetSink`.
///
/// The one behaviour that actually moves a brain mob: it reads `walk_target` and hands it
/// to the navigator, then forgets it on arrival or when the path dies. Every other
/// behaviour "moves" the mob only by writing `walk_target` for this one to consume, which
/// is why a brain with no `MoveToTargetSink` stands still no matter what else it runs.
pub struct MoveToTargetSink {
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl Default for MoveToTargetSink {
    fn default() -> Self {
        Self {
            conditions: [(MemoryModuleType::WalkTarget, MemoryStatus::ValuePresent)],
        }
    }
}

impl MoveToTargetSink {
    fn start_path(ctx: &BehaviorContext<'_, MobActor>) {
        let Some((destination, speed, _)) = walk_target(ctx.memories) else {
            return;
        };
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let Some(mob) = entity.get_mob() else {
            return;
        };
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.set_progress(NavigatorGoal::new(
            ctx.actor.position,
            destination,
            f64::from(speed),
        ));
    }
}

impl Behavior<MobActor> for MoveToTargetSink {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    /// Vanilla keeps this running for as long as the mob still has somewhere to be.
    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        walk_target(ctx.memories).is_some()
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        Self::start_path(ctx);
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some((destination, _, close_enough)) = walk_target(ctx.memories) else {
            return;
        };
        let reached = ctx.actor.position.squared_distance_to(
            destination.x,
            destination.y,
            destination.z,
        ) <= f64::from(close_enough * close_enough);
        if reached {
            ctx.memories.erase(MemoryModuleType::WalkTarget);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(MemoryModuleType::WalkTarget);
    }

    fn debug_name(&self) -> &'static str {
        "move_to_target_sink"
    }
}
