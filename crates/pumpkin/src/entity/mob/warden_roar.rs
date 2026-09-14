//! Roar activity memory/behavior phases, checked against the Java Brain.
pub const DURATION: i64 = 84;
#[derive(Debug, Default, Clone)]
pub struct Roar {
    pub target: Option<i32>,
    pub attack_target: Option<i32>,
    pub sound_delay: Option<i64>,
    pub sound_cooldown: Option<i64>,
    pub sonic_cooldown: Option<i64>,
    pub look_target: Option<i32>,
    pub active: bool,
    pub end_timestamp: Option<i64>,
}
#[derive(Debug, Default)]
pub struct Transition {
    pub start: Option<i32>,
    pub stop: bool,
    pub sound: bool,
    pub attack: Option<i32>,
}
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
impl Roar {
    #[allow(dead_code)] // Standalone direct-Java contract entry point.
    pub fn tick(&mut self, time: i64, no_ai: bool, next_int: impl FnMut(i32)) -> Transition {
        if no_ai {
            return Transition::default();
        }
        self.tick_memories();
        self.tick_behavior(time, next_int)
    }

    pub fn tick_memories(&mut self) {
        tick_memory(&mut self.sound_delay);
        tick_memory(&mut self.sound_cooldown);
        tick_memory(&mut self.sonic_cooldown);
    }

    pub fn tick_behavior(&mut self, time: i64, mut next_int: impl FnMut(i32)) -> Transition {
        let mut change = Transition::default();
        if self.active && self.end_timestamp.is_none() && self.attack_target.is_none() {
            if let Some(target) = self.target {
                next_int(1);
                self.end_timestamp = Some(time.wrapping_add(DURATION));
                self.sound_delay = Some(25);
                self.look_target = Some(target);
                change.start = Some(target);
            }
        }
        if let Some(end) = self.end_timestamp {
            if time > end {
                self.end_timestamp = None;
                change.stop = true;
                if let Some(target) = self.target {
                    self.attack_target = Some(target);
                    self.sonic_cooldown = Some(200);
                    change.attack = Some(target);
                }
                self.target = None;
            } else if self.sound_delay.is_none() && self.sound_cooldown.is_none() {
                self.sound_cooldown = Some(DURATION - 25);
                change.sound = true;
            }
        }
        self.active = self.target.is_some();
        change
    }
}
