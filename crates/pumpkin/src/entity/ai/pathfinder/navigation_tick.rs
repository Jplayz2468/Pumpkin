//! Java PathNavigation tick/follow/stuck phases, with path geometry and world queries supplied.
#[derive(Clone)]
pub struct Route {
    pub nodes: Vec<[i32; 3]>,
    pub index: usize,
}
#[derive(Default)]
pub struct NavigationTick {
    pub tick: i32,
    pub last_stuck_check: i32,
    pub last_stuck_position: [f64; 3],
    pub cached_node: [i32; 3],
    pub timeout_timer: i64,
    pub timeout_limit: f64,
    pub last_timeout_check: i64,
    pub stuck: bool,
}
pub struct Facts {
    pub time: i64,
    pub position: [f64; 3],
    pub temporary_position: [f64; 3],
    pub width: f32,
    pub speed: f32,
    pub can_update: bool,
    pub ground: bool,
    pub cut_corner: bool,
    pub direct: bool,
}
fn square(v: [f64; 3]) -> f64 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}
fn subtract(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn center(p: [i32; 3]) -> [f64; 3] {
    [
        f64::from(p[0]) + 0.5,
        f64::from(p[1]),
        f64::from(p[2]) + 0.5,
    ]
}
fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = square(v).sqrt();
    if length < f64::from(1.0e-5_f32) {
        [0.0; 3]
    } else {
        [v[0] / length, v[1] / length, v[2] / length]
    }
}
fn entity_node(p: [i32; 3], width: f32) -> [f64; 3] {
    let offset = f64::from((width + 1.0) as i32) * 0.5;
    [
        f64::from(p[0]) + offset,
        f64::from(p[1]),
        f64::from(p[2]) + offset,
    ]
}
fn corner(route: &Route, position: [f64; 3], direct: bool) -> bool {
    if route.index + 1 >= route.nodes.len() {
        return false;
    }
    let first = subtract(center(route.nodes[route.index]), position);
    let distance = square(first);
    if !(distance < 4.0) {
        return false;
    }
    if direct {
        return true;
    }
    let second = subtract(center(route.nodes[route.index + 1]), position);
    if square(second) < distance || distance < 0.5 {
        let a = normalize(first);
        let b = normalize(second);
        return b[0] * a[0] + b[1] * a[1] + b[2] * a[2] < 0.0;
    }
    false
}
impl NavigationTick {
    fn stuck_check(&mut self, route: &mut Option<Route>, facts: &Facts) {
        if self.tick.wrapping_sub(self.last_stuck_check) > 100 {
            let speed = if facts.speed >= 1.0 {
                facts.speed
            } else {
                facts.speed * facts.speed
            };
            let reach = speed * 100.0_f32 * 0.25_f32;
            if square(subtract(facts.temporary_position, self.last_stuck_position))
                < f64::from(reach * reach)
            {
                self.stuck = true;
                *route = None;
            } else {
                self.stuck = false;
            }
            self.last_stuck_check = self.tick;
            self.last_stuck_position = facts.temporary_position;
        }
        if let Some(path) = route
            && path.index < path.nodes.len()
        {
            let node = path.nodes[path.index];
            if node == self.cached_node {
                self.timeout_timer = self
                    .timeout_timer
                    .wrapping_add(facts.time.wrapping_sub(self.last_timeout_check));
            } else {
                self.cached_node = node;
                let distance = square(subtract(facts.temporary_position, center(node))).sqrt();
                self.timeout_limit = if facts.speed > 0.0 {
                    distance / f64::from(facts.speed) * 20.0
                } else {
                    0.0
                };
            }
            if self.timeout_limit > 0.0 && self.timeout_timer as f64 > self.timeout_limit * 3.0 {
                self.cached_node = [0; 3];
                self.timeout_timer = 0;
                self.timeout_limit = 0.0;
                self.stuck = false;
                *route = None;
            }
            self.last_timeout_check = facts.time;
        }
    }
    /// Returns the node destination to send to MoveControl; floor-height adjustment
    /// is a separate world-collision boundary. Completed paths remain stored.
    pub fn tick(&mut self, route: &mut Option<Route>, facts: &Facts) -> Option<[f64; 3]> {
        self.tick = self.tick.wrapping_add(1);
        let path = route.as_mut()?;
        if path.index >= path.nodes.len() {
            return None;
        }
        if facts.can_update {
            let node = path.nodes[path.index];
            let threshold = if facts.width > 0.75 {
                facts.width / 2.0
            } else {
                0.75 - facts.width / 2.0
            };
            let close = (facts.position[0] - (f64::from(node[0]) + 0.5)).abs()
                < f64::from(threshold)
                && (facts.position[2] - (f64::from(node[2]) + 0.5)).abs() < f64::from(threshold)
                && (facts.position[1] - f64::from(node[1])).abs() < 1.0;
            if close || (facts.cut_corner && corner(path, facts.temporary_position, facts.direct)) {
                path.index += 1;
            }
            self.stuck_check(route, facts);
        } else {
            let next = entity_node(path.nodes[path.index], facts.width);
            if facts.temporary_position[1] > next[1]
                && !facts.ground
                && facts.temporary_position[0].floor() == next[0].floor()
                && facts.temporary_position[2].floor() == next[2].floor()
            {
                path.index += 1;
            }
        }
        let path = route.as_ref()?;
        path.nodes
            .get(path.index)
            .map(|p| entity_node(*p, facts.width))
    }
}
