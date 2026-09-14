//! Digging and forced dismount behavior; cooldown memory is owned by the Brain.
#[derive(Debug, Default, Clone)]
pub struct Digging {
    pub active: bool,
    pub end_timestamp: Option<i64>,
}
pub struct Facts {
    pub no_ai: bool,
    pub ground: bool,
    pub water: bool,
    pub lava: bool,
    pub passenger: bool,
    pub removed: bool,
    pub attack: bool,
    pub walk: bool,
    pub cooldown: bool,
    pub roar: bool,
    pub emerging: bool,
}
#[derive(Debug, Default)]
pub struct Transition {
    pub unride: bool,
    pub start: bool,
    pub agitated: bool,
    pub remove: bool,
}
impl Digging {
    pub fn tick(
        &mut self,
        time: i64,
        facts: &Facts,
        mut next_int: impl FnMut(i32),
        mut unride: impl FnMut() -> (bool, bool, bool),
    ) -> Transition {
        if facts.no_ai {
            return Transition::default();
        }
        let mut change = self.start_behavior(time, facts, &mut next_int, &mut unride);
        change.remove |= self.run_behavior(time, facts.removed || change.remove);
        self.active = !facts.emerging && !facts.roar && !facts.cooldown;
        change
    }
    pub fn start_behavior(
        &mut self,
        time: i64,
        facts: &Facts,
        mut next_int: impl FnMut(i32),
        mut unride: impl FnMut() -> (bool, bool, bool),
    ) -> Transition {
        let mut change = Transition::default();
        let (mut ground, mut water, mut lava) = (facts.ground, facts.water, facts.lava);
        if self.active && facts.passenger {
            next_int(1); // ForceUnmount's fixed duration still consumes RNG.
            change.unride = true;
            (ground, water, lava) = unride();
        }
        let removed = facts.removed;
        if self.active
            && self.end_timestamp.is_none()
            && !facts.attack
            && !facts.walk
            && (ground || water || lava)
        {
            next_int(1);
            self.end_timestamp = Some(time.wrapping_add(100));
            if ground {
                change.start = true;
            } else {
                change.agitated = true;
                change.remove = !removed;
            }
        }
        change
    }
    pub fn run_behavior(&mut self, time: i64, removed: bool) -> bool {
        if self.end_timestamp.is_some_and(|end| time > end || removed) {
            self.end_timestamp = None;
            !removed
        } else {
            false
        }
    }
}
