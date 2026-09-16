use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `LookAtTargetSink`.
///
/// Turns the head toward the `look_target` memory. The counterpart of
/// `MoveToTargetSink`: other behaviours only write the memory, this one acts on it.
pub struct LookAtTargetSink {
    conditions: [(MemoryModuleType, MemoryStatus); 1],
    duration: (i32, i32),
}

impl LookAtTargetSink {
    #[must_use]
    pub fn new(min_duration: i32, max_duration: i32) -> Self {
        Self {
            conditions: [(MemoryModuleType::LookTarget, MemoryStatus::ValuePresent)],
            duration: (min_duration, max_duration),
        }
    }
}

impl Behavior<MobActor> for LookAtTargetSink {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        self.duration
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        matches!(
            ctx.memories.get(MemoryModuleType::LookTarget),
            Some(MemoryValue::Vec3(_) | MemoryValue::Position(_))
        )
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let target = match ctx.memories.get(MemoryModuleType::LookTarget) {
            Some(MemoryValue::Vec3(pos)) => *pos,
            Some(MemoryValue::Position(pos)) => pos.to_f64(),
            _ => return,
        };
        if let Some(entity) = ctx.actor.entity() {
            entity.get_entity().look_at(target);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(MemoryModuleType::LookTarget);
    }

    fn debug_name(&self) -> &'static str {
        "look_at_target_sink"
    }
}
