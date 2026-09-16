use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::start_attacking::attack_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use std::sync::atomic::Ordering::Relaxed;

/// `StopAttackingIfTargetInvalid.TIMEOUT_TO_GET_WITHIN_ATTACK_RANGE`.
const TIMEOUT_TO_REACH_TARGET: i64 = 200;

/// Port of `StopAttackingIfTargetInvalid`.
///
/// Forgets the attack target once it dies, leaves the world, or the mob has spent too
/// long failing to reach it. Without this a brain mob chases a corpse forever.
pub struct StopAttackingIfTargetInvalid {
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl Default for StopAttackingIfTargetInvalid {
    fn default() -> Self {
        Self {
            conditions: [
                (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
                (
                    MemoryModuleType::CantReachWalkTargetSince,
                    MemoryStatus::Registered,
                ),
            ],
        }
    }
}

impl StopAttackingIfTargetInvalid {
    /// Vanilla's `isTiredOfTryingToReachTarget`.
    fn tired_of_trying(ctx: &BehaviorContext<'_, MobActor>) -> bool {
        match ctx.memories.get(MemoryModuleType::CantReachWalkTargetSince) {
            Some(MemoryValue::Long(since)) => ctx.time - since > TIMEOUT_TO_REACH_TARGET,
            _ => false,
        }
    }
}

impl Behavior<MobActor> for StopAttackingIfTargetInvalid {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let alive = ctx
            .actor
            .world
            .get_entity_by_id(target_id)
            .is_some_and(|target| {
                !target.get_entity().removed.load(Relaxed)
                    && target
                        .get_living_entity()
                        .is_none_or(|living| !living.dead.load(Relaxed))
            });

        if !alive || Self::tired_of_trying(ctx) {
            ctx.memories.erase(MemoryModuleType::AttackTarget);
            ctx.memories.erase(MemoryModuleType::CantReachWalkTargetSince);
        }
    }

    fn debug_name(&self) -> &'static str {
        "stop_attacking_if_target_invalid"
    }
}
