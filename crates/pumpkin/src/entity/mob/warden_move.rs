//! MoveToTargetSink lifecycle; path creation and navigation are explicit boundaries.
#[derive(Clone, Copy, Debug)]
pub struct WalkTarget {
    pub position: [i32; 3],
    pub speed: f32,
    pub distance: i32,
    pub spectator: bool,
}
#[derive(Default, Debug)]
pub struct Memories {
    pub walk: Option<WalkTarget>,
    pub path: Option<u64>,
    pub cannot_reach_since: Option<i64>,
}
#[derive(Default, Debug)]
pub struct MoveSink {
    pub path: Option<u64>,
    pub last_target: Option<[i32; 3]>,
    pub speed: f32,
    pub cooldown: i32,
    pub end_timestamp: Option<i64>,
}
pub trait Navigation {
    fn create_path(&mut self, target: [i32; 3]) -> Option<(u64, bool)>;
    fn fallback_path(&mut self, target: [i32; 3]) -> Option<u64>;
    fn move_to(&mut self, path: Option<u64>, speed: f32);
    fn path(&self) -> Option<u64>;
    fn done(&self) -> bool;
    fn stuck(&self) -> bool;
    fn stop(&mut self);
    fn next_world_int(&mut self, bound: i32) -> i32;
}
pub fn reached(origin: [i32; 3], walk: WalkTarget) -> bool {
    // Vec3i.distManhattan uses Java int subtraction, abs and addition.
    let distance = walk.position[0]
        .wrapping_sub(origin[0])
        .wrapping_abs()
        .wrapping_add(walk.position[1].wrapping_sub(origin[1]).wrapping_abs())
        .wrapping_add(walk.position[2].wrapping_sub(origin[2]).wrapping_abs());
    distance <= walk.distance
}
impl MoveSink {
    fn compute(
        &mut self,
        time: i64,
        origin: [i32; 3],
        memories: &mut Memories,
        walk: WalkTarget,
        nav: &mut impl Navigation,
    ) -> bool {
        let result = nav.create_path(walk.position);
        self.path = result.map(|(id, _)| id);
        self.speed = walk.speed;
        if reached(origin, walk) {
            memories.cannot_reach_since = None;
            return false;
        }
        if result.is_some_and(|(_, reachable)| reachable) {
            memories.cannot_reach_since = None;
        } else if memories.cannot_reach_since.is_none() {
            memories.cannot_reach_since = Some(time);
        }
        if self.path.is_some() {
            return true;
        }
        self.path = nav.fallback_path(walk.position);
        self.path.is_some()
    }
    fn begin_navigation(&self, memories: &mut Memories, nav: &mut impl Navigation) {
        memories.path = self.path;
        nav.move_to(self.path, self.speed);
    }
    pub fn start(
        &mut self,
        time: i64,
        origin: [i32; 3],
        memories: &mut Memories,
        nav: &mut impl Navigation,
    ) {
        if self.end_timestamp.is_some() || memories.path.is_some() {
            return;
        }
        let Some(walk) = memories.walk else {
            return;
        };
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return;
        }
        let arrived = reached(origin, walk);
        if !arrived && self.compute(time, origin, memories, walk, nav) {
            self.last_target = Some(walk.position);
            self.end_timestamp = Some(time.wrapping_add(i64::from(150 + nav.next_world_int(101))));
            self.begin_navigation(memories, nav);
        } else {
            memories.walk = None;
            if arrived {
                memories.cannot_reach_since = None;
            }
        }
    }
    pub fn run(
        &mut self,
        time: i64,
        origin: [i32; 3],
        memories: &mut Memories,
        nav: &mut impl Navigation,
    ) {
        let Some(end) = self.end_timestamp else {
            return;
        };
        if time > end
            || self.path.is_none()
            || self.last_target.is_none()
            || nav.done()
            || memories
                .walk
                .is_none_or(|walk| reached(origin, walk) || walk.spectator)
        {
            if memories.walk.is_some_and(|walk| !reached(origin, walk)) && nav.stuck() {
                self.cooldown = nav.next_world_int(40);
            }
            nav.stop();
            memories.walk = None;
            memories.path = None;
            self.path = None;
            self.end_timestamp = None;
            return;
        }
        let current = nav.path();
        if self.path != current {
            self.path = current;
            memories.path = current;
        }
        if current.is_none() || self.last_target.is_none() {
            return;
        }
        let walk = memories.walk.unwrap();
        let last = self.last_target.unwrap();
        let delta = [
            f64::from(walk.position[0]) - f64::from(last[0]),
            f64::from(walk.position[1]) - f64::from(last[1]),
            f64::from(walk.position[2]) - f64::from(last[2]),
        ];
        if delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2] > 4.0
            && self.compute(time, origin, memories, walk, nav)
        {
            self.last_target = Some(walk.position);
            self.begin_navigation(memories, nav);
        }
    }
}
