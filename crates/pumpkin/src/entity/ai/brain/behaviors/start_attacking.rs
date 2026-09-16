use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryMap, MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Finds the entity this mob should attack, if any. Vanilla's `TargetFinder`.
pub type TargetFinder = fn(&BehaviorContext<'_, MobActor>) -> Option<i32>;

/// Port of `StartAttacking`.
///
/// Sets `attack_target`, which is what a Fight activity gates on. Species supply their own
/// finder, the way `NautilusAi::findNearestValidAttackTarget` does.
pub struct StartAttacking {
    find_target: TargetFinder,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl StartAttacking {
    #[must_use]
    pub fn new(find_target: TargetFinder) -> Self {
        Self {
            find_target,
            conditions: [
                (MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent),
                (
                    MemoryModuleType::CantReachWalkTargetSince,
                    MemoryStatus::Registered,
                ),
            ],
        }
    }
}

impl Behavior<MobActor> for StartAttacking {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        (self.find_target)(ctx).is_some()
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target) = (self.find_target)(ctx) else {
            return;
        };
        ctx.memories
            .set(MemoryModuleType::AttackTarget, MemoryValue::EntityId(target));
        ctx.memories.erase(MemoryModuleType::CantReachWalkTargetSince);
    }

    fn debug_name(&self) -> &'static str {
        "start_attacking"
    }
}

/// Reads the `attack_target` memory, for behaviours that act on it.
#[must_use]
pub fn attack_target(memories: &MemoryMap) -> Option<i32> {
    match memories.get(MemoryModuleType::AttackTarget) {
        Some(MemoryValue::EntityId(id)) => Some(*id),
        _ => None,
    }
}
