use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;

/// A predicate on the mob, deciding whether the memory should go.
pub type MobPredicate = fn(&BehaviorContext<'_, MobActor>) -> bool;

/// Port of `EraseMemoryIf`.
///
/// Drops a memory once some condition holds. Vanilla uses it to clear state that no
/// behaviour owns outright, such as an axolotl forgetting it played dead.
pub struct EraseMemoryIf {
    predicate: MobPredicate,
    memory: MemoryModuleType,
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl EraseMemoryIf {
    #[must_use]
    pub fn new(predicate: MobPredicate, memory: MemoryModuleType) -> Self {
        Self {
            predicate,
            memory,
            conditions: [(memory, MemoryStatus::ValuePresent)],
        }
    }
}

impl Behavior<MobActor> for EraseMemoryIf {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        (self.predicate)(ctx)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(self.memory);
    }

    fn debug_name(&self) -> &'static str {
        "erase_memory_if"
    }
}
