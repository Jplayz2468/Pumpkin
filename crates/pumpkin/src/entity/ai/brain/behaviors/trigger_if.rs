use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext, BehaviorSlot};
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;

/// A predicate on the mob, tested before the wrapped behaviour may start.
pub type MobPredicate = fn(&BehaviorContext<'_, MobActor>) -> bool;

/// Port of `BehaviorBuilder.triggerIf`.
///
/// Gates an existing behaviour behind a species predicate without rewriting it. Vanilla
/// uses this constantly -- the camel wraps three of its idle behaviours in
/// `triggerIf(not(Camel::refuseToMove), ..)`, which is what stops a sitting or ridden
/// camel strolling away.
pub struct TriggerIf {
    predicate: MobPredicate,
    child: BehaviorSlot<MobActor>,
}

impl TriggerIf {
    #[must_use]
    pub fn new(predicate: MobPredicate, child: Box<dyn Behavior<MobActor>>) -> Self {
        Self {
            predicate,
            child: BehaviorSlot::new(child),
        }
    }
}

impl Behavior<MobActor> for TriggerIf {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        // The child's own conditions still apply; they are checked when it starts.
        &[]
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        (self.predicate)(ctx)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        self.child.try_start(ctx, 1);
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        self.child.tick_or_stop(ctx);
        self.child.status()
            == crate::entity::ai::brain::behavior::BehaviorStatus::Running
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        self.child.do_stop(ctx);
    }

    fn debug_name(&self) -> &'static str {
        "trigger_if"
    }
}
