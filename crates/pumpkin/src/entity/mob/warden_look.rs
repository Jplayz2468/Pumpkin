//! LookAtTargetSink lifecycle. Duration is 45..90 ticks, not head-angle limits.
#[derive(Debug, Default)]
pub struct LookSink {
    pub end_timestamp: Option<i64>,
}
impl LookSink {
    pub fn start(&mut self, time: i64, look: bool, mut next_int: impl FnMut(i32) -> i32) {
        if self.end_timestamp.is_none() && look {
            self.end_timestamp = Some(time.wrapping_add(i64::from(45 + next_int(46))));
        }
    }
    /// Returns (call look control, erase LOOK_TARGET). This reads the memory after
    /// every behavior has attempted to start, including non-core memory writers.
    pub fn run(&mut self, time: i64, look: bool, visible: bool) -> (bool, bool) {
        if let Some(end) = self.end_timestamp {
            if time > end || !look || !visible {
                self.end_timestamp = None;
                return (false, true);
            }
            return (true, false);
        }
        (false, false)
    }
    #[allow(dead_code)] // Isolated actual-Java lifecycle contract.
    pub fn tick(
        &mut self,
        time: i64,
        no_ai: bool,
        look: bool,
        visible: bool,
        next_int: impl FnMut(i32) -> i32,
    ) -> (bool, bool) {
        if no_ai {
            return (false, false);
        }
        self.start(time, look, next_int);
        self.run(time, look, visible)
    }
}
