use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `CountDownCooldownTicks`.
///
/// Decrements a numeric memory every tick and erases it at zero, which is how every
/// cooldown in a brain expires.
pub struct CountDownCooldownTicks {
    memory: MemoryModuleType,
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl CountDownCooldownTicks {
    #[must_use]
    pub fn new(memory: MemoryModuleType) -> Self {
        Self {
            memory,
            conditions: [(memory, MemoryStatus::ValuePresent)],
        }
    }
}

impl<A: ?Sized> Behavior<A> for CountDownCooldownTicks {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    /// Vanilla keeps running while the counter is above zero.
    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, A>) -> bool {
        remaining(ctx, self.memory).is_some_and(|ticks| ticks > 0)
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, A>) {
        let Some(ticks) = remaining(ctx, self.memory) else {
            return;
        };
        ctx.memories
            .set(self.memory, MemoryValue::Int(ticks.saturating_sub(1)));
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, A>) {
        ctx.memories.erase(self.memory);
    }

    fn debug_name(&self) -> &'static str {
        "count_down_cooldown_ticks"
    }
}

fn remaining<A: ?Sized>(ctx: &BehaviorContext<'_, A>, memory: MemoryModuleType) -> Option<i32> {
    match ctx.memories.get(memory) {
        Some(MemoryValue::Int(ticks)) => Some(*ticks),
        Some(MemoryValue::Long(ticks)) => Some(*ticks as i32),
        _ => None,
    }
}
