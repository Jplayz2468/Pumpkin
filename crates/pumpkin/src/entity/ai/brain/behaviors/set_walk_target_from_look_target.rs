use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `SetWalkTargetFromLookTarget`.
///
/// Walks toward whatever the mob is currently looking at, unless it is already close
/// enough. Pairs with a sensor that writes `look_target`.
pub struct SetWalkTargetFromLookTarget {
    speed: f32,
    close_enough: i32,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl SetWalkTargetFromLookTarget {
    #[must_use]
    pub fn new(speed: f32, close_enough: i32) -> Self {
        Self {
            speed,
            close_enough,
            conditions: [
                (MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::LookTarget, MemoryStatus::ValuePresent),
            ],
        }
    }
}

impl Behavior<MobActor> for SetWalkTargetFromLookTarget {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let target = match ctx.memories.get(MemoryModuleType::LookTarget) {
            Some(MemoryValue::Vec3(pos)) => *pos,
            Some(MemoryValue::Position(pos)) => pos.to_f64(),
            _ => return,
        };
        let close_enough = f64::from(self.close_enough * self.close_enough);
        if ctx
            .actor
            .position
            .squared_distance_to(target.x, target.y, target.z)
            <= close_enough
        {
            return;
        }
        set_walk_target(ctx.memories, target, self.speed);
    }

    fn debug_name(&self) -> &'static str {
        "set_walk_target_from_look_target"
    }
}
