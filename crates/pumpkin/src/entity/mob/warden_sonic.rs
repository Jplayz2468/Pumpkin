//! SonicBoom behavior phases and beam mathematics, checked against the actual Java behavior.
#[derive(Debug, Default, Clone)]
pub struct SonicBoom {
    pub sound_delay: Option<i64>,
    pub sound_cooldown: Option<i64>,
    pub melee_cooldown: Option<i64>,
    pub end_timestamp: Option<i64>,
}
#[derive(Debug, Default)]
pub struct Transition {
    pub start: bool,
    pub look: bool,
    pub fire: bool,
    pub stop: bool,
}
pub struct Facts {
    pub active: bool,
    pub attack: bool,
    pub eligible: bool,
    pub in_range: bool,
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
impl SonicBoom {
    /// The Brain's memory pass runs before any behavior. The shared sonic cooldown
    /// is owned by the combat state and is ticked once by its caller.
    pub fn tick_memories(&mut self) {
        tick_memory(&mut self.sound_delay);
        tick_memory(&mut self.sound_cooldown);
        tick_memory(&mut self.melee_cooldown);
    }
    #[allow(dead_code)] // Standalone direct-Java contract entry point.
    pub fn tick(
        &mut self,
        time: i64,
        no_ai: bool,
        cooldown: &mut Option<i64>,
        facts: &Facts,
        next_int: impl FnMut(i32),
    ) -> Transition {
        if no_ai {
            return Transition::default();
        }
        tick_memory(cooldown);
        self.tick_memories();
        self.tick_behavior(time, cooldown, facts, next_int)
    }
    pub fn tick_behavior(
        &mut self,
        time: i64,
        cooldown: &mut Option<i64>,
        facts: &Facts,
        mut next_int: impl FnMut(i32),
    ) -> Transition {
        let mut change = Transition::default();
        if facts.active
            && facts.attack
            && cooldown.is_none()
            && facts.in_range
            && self.end_timestamp.is_none()
        {
            next_int(1);
            self.end_timestamp = Some(time.wrapping_add(60));
            self.melee_cooldown = Some(60);
            self.sound_delay = Some(34);
            change.start = true;
        }
        if let Some(end) = self.end_timestamp {
            if time > end {
                self.end_timestamp = None;
                *cooldown = Some(40);
                change.stop = true;
            } else {
                change.look = facts.attack;
                if self.sound_delay.is_none() && self.sound_cooldown.is_none() {
                    self.sound_cooldown = Some(26);
                    change.fire = facts.attack && facts.eligible && facts.in_range;
                }
            }
        }
        change
    }
}
pub fn in_range(origin: [f64; 3], target: [f64; 3]) -> bool {
    let x = target[0] - origin[0];
    let y = target[1] - origin[1];
    let z = target[2] - origin[2];
    x * x + z * z < 15.0 * 15.0 && y * y < 20.0 * 20.0
}
#[derive(Debug)]
pub struct Beam {
    pub particles: Vec<[f64; 3]>,
    pub direction: [f64; 3],
}
impl Beam {
    pub fn new(origin: [f64; 3], eye: [f64; 3]) -> Self {
        let delta = [eye[0] - origin[0], eye[1] - origin[1], eye[2] - origin[2]];
        let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        let direction = if length < 1.0e-5 {
            [0.0; 3]
        } else {
            [delta[0] / length, delta[1] / length, delta[2] / length]
        };
        let mut particles = vec![];
        for i in 1..(length.floor() as i32).wrapping_add(7) {
            let d = i as f64;
            particles.push([
                origin[0] + direction[0] * d,
                origin[1] + direction[1] * d,
                origin[2] + direction[2] * d,
            ]);
        }
        Self {
            particles,
            direction,
        }
    }
    /// Applied only after the target's health-damage operation accepts the hit.
    pub fn push(&self, resistance: f64) -> [f64; 3] {
        let vertical = 0.5 * (1.0 - resistance);
        let horizontal = 2.5 * (1.0 - resistance);
        [
            self.direction[0] * horizontal,
            self.direction[1] * vertical,
            self.direction[2] * horizontal,
        ]
    }
}
