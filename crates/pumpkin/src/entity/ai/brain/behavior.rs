//! The behaviour contract.
//!
//! Port of vanilla `BehaviorControl` and the `Behavior` base class
//! (`net/minecraft/world/entity/ai/behavior/`).

use super::memory::{MemoryMap, MemoryStatus};
use super::registry::MemoryModuleType;

/// Vanilla `Behavior.Status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BehaviorStatus {
    #[default]
    Stopped,
    Running,
}

/// Everything a behaviour needs that is not the mob itself.
///
/// Vanilla passes `(ServerLevel, E body, long timestamp)` into every hook. The mob is
/// supplied separately by the caller because Rust cannot hand out a `&mut` to the owning
/// entity while the brain is borrowed from it.
pub struct BehaviorContext<'a> {
    pub memories: &'a mut MemoryMap,
    /// Vanilla's `timestamp` -- the level game time, not a per-behaviour counter.
    pub time: i64,
}

/// Port of vanilla `BehaviorControl`.
///
/// `try_start` / `tick_or_stop` / `do_stop` are deliberately provided rather than
/// overridable: vanilla marks them `final` and routes all customization through
/// `start` / `tick` / `stop` / `can_still_use` / `check_extra_start_conditions`. Keeping
/// that split is what guarantees the status flag and timeout can never drift out of sync
/// with whether the behaviour is actually running.
pub trait Behavior: Send + Sync {
    /// Memories that must hold the given status for this behaviour to start. Vanilla's
    /// `entryCondition`.
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &[]
    }

    /// Vanilla's `minDuration` / `maxDuration`. The default is a single tick, matching
    /// `Behavior`'s one-argument constructor.
    fn duration_range(&self) -> (i32, i32) {
        (60, 60)
    }

    fn check_extra_start_conditions(&mut self, _ctx: &mut BehaviorContext<'_>) -> bool {
        true
    }

    fn start(&mut self, _ctx: &mut BehaviorContext<'_>) {}

    fn tick(&mut self, _ctx: &mut BehaviorContext<'_>) {}

    fn stop(&mut self, _ctx: &mut BehaviorContext<'_>) {}

    /// Vanilla's default is `false`, i.e. a behaviour runs for exactly one tick unless it
    /// opts into continuing.
    fn can_still_use(&mut self, _ctx: &mut BehaviorContext<'_>) -> bool {
        false
    }

    fn debug_name(&self) -> &'static str;
}

/// Wraps a [`Behavior`] with the status and timeout bookkeeping vanilla keeps in the
/// `Behavior` base class, so implementations cannot get it wrong.
pub struct BehaviorSlot {
    behavior: Box<dyn Behavior>,
    status: BehaviorStatus,
    end_timestamp: i64,
}

impl BehaviorSlot {
    #[must_use]
    pub fn new(behavior: Box<dyn Behavior>) -> Self {
        Self {
            behavior,
            status: BehaviorStatus::Stopped,
            end_timestamp: 0,
        }
    }

    #[must_use]
    pub const fn status(&self) -> BehaviorStatus {
        self.status
    }

    #[must_use]
    pub fn debug_name(&self) -> &'static str {
        self.behavior.debug_name()
    }

    fn has_required_memories(&self, memories: &MemoryMap) -> bool {
        self.behavior
            .entry_conditions()
            .iter()
            .all(|&(memory, status)| memories.check(memory, status))
    }

    /// Vanilla `Behavior.tryStart`.
    ///
    /// `duration` is rolled once at start from `[min, max]` inclusive, and the behaviour
    /// is stopped when the timestamp passes it.
    pub fn try_start(&mut self, ctx: &mut BehaviorContext<'_>, duration: i32) -> bool {
        if !self.has_required_memories(ctx.memories) {
            return false;
        }
        if !self.behavior.check_extra_start_conditions(ctx) {
            return false;
        }
        self.status = BehaviorStatus::Running;
        self.end_timestamp = ctx.time + i64::from(duration);
        self.behavior.start(ctx);
        true
    }

    /// Vanilla `Behavior.tickOrStop`.
    pub fn tick_or_stop(&mut self, ctx: &mut BehaviorContext<'_>) {
        if !self.timed_out(ctx.time) && self.behavior.can_still_use(ctx) {
            self.behavior.tick(ctx);
        } else {
            self.do_stop(ctx);
        }
    }

    /// Vanilla `Behavior.doStop`.
    pub fn do_stop(&mut self, ctx: &mut BehaviorContext<'_>) {
        self.status = BehaviorStatus::Stopped;
        self.behavior.stop(ctx);
    }

    #[must_use]
    pub const fn timed_out(&self, time: i64) -> bool {
        time > self.end_timestamp
    }

    #[must_use]
    pub fn duration_range(&self) -> (i32, i32) {
        self.behavior.duration_range()
    }
}
