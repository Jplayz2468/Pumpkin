//! Warden emergence's Brain memory/behavior phases, checked against Java 26.2.
//! Other Warden activities are implemented separately; this is not a generic AI replacement.

pub const EMERGE_DURATION: i64 = 134;
pub const DIG_COOLDOWN: i64 = 1200;

#[derive(Debug, Default, Clone)]
pub struct Emergence {
    pub emerging_memory: Option<i64>,
    pub dig_cooldown: Option<i64>,
    pub active: bool,
    pub end_timestamp: Option<i64>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Transition {
    pub start: bool,
    pub stop: bool,
}

/// MemorySlot.tick expires before decrementing; MAX_VALUE means never expire.
fn tick_memory(memory: &mut Option<i64>) {
    if let Some(ttl) = memory {
        if *ttl == i64::MAX {
            return;
        }
        if *ttl <= 0 {
            *memory = None;
        } else {
            *ttl -= 1;
        }
    }
}

impl Emergence {
    pub fn finalize_spawn(&mut self, triggered: bool) {
        self.dig_cooldown = Some(DIG_COOLDOWN);
        if triggered {
            self.emerging_memory = Some(EMERGE_DURATION);
        }
    }

    /// NoAI skips the entire Brain step, including memory expiry and activity selection.
    /// Fixed-duration Behavior.tryStart still consumes nextInt(1).
    #[allow(dead_code)] // Standalone direct-Java contract entry point.
    pub fn tick(&mut self, time: i64, no_ai: bool, next_int: impl FnMut(i32)) -> Transition {
        if no_ai {
            return Transition::default();
        }
        self.tick_memories();
        self.tick_behavior(time, next_int)
    }

    pub fn tick_memories(&mut self) {
        tick_memory(&mut self.emerging_memory);
        tick_memory(&mut self.dig_cooldown);
    }

    pub fn tick_behavior(&mut self, time: i64, next_int: impl FnMut(i32)) -> Transition {
        let mut change = self.start_behavior(time, next_int);
        change.stop = self.run_behavior(time);
        self.active = self.emerging_memory.is_some();
        change
    }
    pub fn start_behavior(&mut self, time: i64, mut next_int: impl FnMut(i32)) -> Transition {
        let mut change = Transition::default();
        if self.active && self.end_timestamp.is_none() && self.emerging_memory.is_some() {
            next_int(1);
            self.end_timestamp = Some(time.wrapping_add(EMERGE_DURATION));
            change.start = true;
        }
        change
    }
    /// Running behaviors continue after their originating activity changes.
    pub fn run_behavior(&mut self, time: i64) -> bool {
        if self.end_timestamp.is_some_and(|end| time > end) {
            self.end_timestamp = None;
            true
        } else {
            false
        }
    }
}
