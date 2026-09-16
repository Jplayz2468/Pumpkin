use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext, BehaviorSlot, BehaviorStatus};
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Vanilla `GateBehavior.OrderPolicy`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OrderPolicy {
    Ordered,
    Shuffled,
}

/// Vanilla `GateBehavior.RunningPolicy`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunningPolicy {
    /// Stop at the first child that starts.
    RunOne,
    /// Start every child that will.
    TryAll,
}

/// Port of `GateBehavior`.
///
/// Groups child behaviours behind one set of entry conditions and one policy pair. This
/// is how vanilla expresses "pick one of these ways to wander", so a brain's idle
/// activity is usually a single gate rather than a flat list.
pub struct GateBehavior<A: ?Sized> {
    conditions: Vec<(MemoryModuleType, MemoryStatus)>,
    order: OrderPolicy,
    running: RunningPolicy,
    children: Vec<BehaviorSlot<A>>,
}

impl<A: ?Sized> GateBehavior<A> {
    #[must_use]
    pub fn new(
        conditions: Vec<(MemoryModuleType, MemoryStatus)>,
        order: OrderPolicy,
        running: RunningPolicy,
        children: Vec<BehaviorSlot<A>>,
    ) -> Self {
        Self {
            conditions,
            order,
            running,
            children,
        }
    }
}

impl<A: ?Sized> Behavior<A> for GateBehavior<A> {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, A>) {
        // `Shuffled` is not reproduced faithfully: vanilla shuffles with the mob's own
        // random, which a behaviour has no handle on here. Ordered children are exact.
        for child in &mut self.children {
            let started = child.try_start(ctx, 1);
            if started && self.running == RunningPolicy::RunOne {
                break;
            }
        }
        let _ = self.order;
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, A>) -> bool {
        self.children.iter_mut().fold(false, |running, child| {
            child.tick_or_stop(ctx);
            running || child.status() == BehaviorStatus::Running
        })
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, A>) {
        for child in &mut self.children {
            child.do_stop(ctx);
        }
    }

    fn debug_name(&self) -> &'static str {
        "gate"
    }
}
