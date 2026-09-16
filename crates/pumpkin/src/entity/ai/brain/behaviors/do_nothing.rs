use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};

/// Port of `DoNothing`.
///
/// Occupies a slot for a spell without acting, which is how vanilla makes a mob pause
/// between other behaviours rather than always having something to do.
pub struct DoNothing {
    duration: (i32, i32),
}

impl DoNothing {
    #[must_use]
    pub const fn new(min_duration: i32, max_duration: i32) -> Self {
        Self {
            duration: (min_duration, max_duration),
        }
    }
}

impl<A: ?Sized> Behavior<A> for DoNothing {
    fn duration_range(&self) -> (i32, i32) {
        self.duration
    }

    /// Runs until its timeout, doing nothing meanwhile.
    fn can_still_use(&mut self, _ctx: &mut BehaviorContext<'_, A>) -> bool {
        true
    }

    fn debug_name(&self) -> &'static str {
        "do_nothing"
    }
}
