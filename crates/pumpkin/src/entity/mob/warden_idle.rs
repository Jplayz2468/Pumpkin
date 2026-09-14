//! Warden RunOne(RandomStroll(0.5), weight 2; DoNothing(30,60), weight 1).
//! The shuffle owns an independent random source and retains its entry order.
pub struct Idle {
    pub running: bool,
    pub wait_until: Option<i64>,
    pub order: [bool; 2],
}
impl Default for Idle {
    fn default() -> Self {
        Self {
            running: false,
            wait_until: None,
            order: [true, false],
        }
    }
}
impl Idle {
    /// Returns true when RandomStroll requests a land destination. A failed
    /// search still completes that one-shot; it does not start DoNothing.
    pub fn start(
        &mut self,
        time: i64,
        sniffing: bool,
        walk: bool,
        water: bool,
        mut shuffle_float: impl FnMut() -> f32,
        mut world_int: impl FnMut(i32) -> i32,
    ) -> bool {
        if self.running || sniffing {
            return false;
        }
        self.running = true;
        let mut entries = self.order.map(|stroll| {
            let sample = f64::from(shuffle_float());
            (stroll, if stroll { -sample.sqrt() } else { -sample })
        });
        if entries[1].1 < entries[0].1 {
            entries.swap(0, 1);
        }
        self.order = entries.map(|(stroll, _)| stroll);
        for stroll in self.order {
            if stroll {
                if !walk && !water {
                    return true;
                }
            } else {
                self.wait_until = Some(time.wrapping_add(i64::from(30 + world_int(31))));
                return false;
            }
        }
        false
    }
    pub fn run(&mut self, time: i64) {
        if !self.running {
            return;
        }
        if self.wait_until.is_none_or(|end| time > end) {
            self.wait_until = None;
            self.running = false;
        }
    }
}
