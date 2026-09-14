//! TryToSniff, Sniffing and disturbance memory transitions verified against Java.
#[derive(Debug, Default, Clone)]
pub struct Sniffing {
    pub sniffing: Option<i64>,
    pub cooldown: Option<i64>,
    pub disturbance: Option<([i32; 3], i64)>,
    pub look: Option<([i32; 3], i64)>,
    pub end_timestamp: Option<i64>,
}
#[derive(Debug, Default)]
pub struct Transition {
    pub pose: bool,
    pub sound: bool,
    pub stop: bool,
    pub forget_walk: bool,
}
pub struct Facts {
    pub idle_active: bool,
    pub sniff_active: bool,
    pub nearest: bool,
    pub attack: bool,
    pub walk: bool,
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
fn tick_location(memory: &mut Option<([i32; 3], i64)>) {
    if let Some((_, ttl)) = memory {
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
impl Sniffing {
    pub fn tick_memories(&mut self) {
        tick_memory(&mut self.sniffing);
        tick_memory(&mut self.cooldown);
        tick_location(&mut self.disturbance);
        tick_location(&mut self.look);
    }
    pub fn tick_behavior(
        &mut self,
        time: i64,
        facts: &Facts,
        mut next_int: impl FnMut(i32) -> i32,
    ) -> Transition {
        let mut change = Transition::default();
        if facts.idle_active
            && self.cooldown.is_none()
            && facts.nearest
            && self.disturbance.is_none()
        {
            self.sniffing = Some(i64::MAX);
            self.cooldown = Some(i64::from(100 + next_int(101)));
            change.pose = true;
            change.forget_walk = true;
        }
        if facts.sniff_active
            && self.end_timestamp.is_none()
            && self.sniffing.is_some()
            && !facts.attack
            && !facts.walk
        {
            next_int(1);
            self.end_timestamp = Some(time.wrapping_add(84));
            change.sound = true;
        }
        if self.end_timestamp.is_some_and(|end| time > end) {
            self.end_timestamp = None;
            self.sniffing = None;
            change.stop = true;
        }
        change
    }
    pub fn disturb(
        &mut self,
        position: [i32; 3],
        inside_border: bool,
        angry: bool,
        attack: bool,
        dig: &mut Option<i64>,
    ) -> bool {
        if !inside_border || angry || attack {
            return false;
        }
        if dig.is_some() {
            *dig = Some(1200);
        }
        self.cooldown = Some(100);
        self.look = Some((position, 100));
        self.disturbance = Some((position, 100));
        true
    }
}
pub fn in_range(origin: [f64; 3], target: [f64; 3]) -> bool {
    let x = target[0] - origin[0];
    let y = target[1] - origin[1];
    let z = target[2] - origin[2];
    x * x + z * z < 6.0 * 6.0 && y * y < 20.0 * 20.0
}
