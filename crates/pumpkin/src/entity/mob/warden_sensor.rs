//! Warden nearest-entity scan and lazily cached visibility; checked against Java's sensor.
use std::collections::HashMap;
#[derive(Clone)]
pub struct Target {
    pub id: i32,
    pub player: bool,
    pub position: [f64; 3],
    pub bounds: [f64; 6],
    pub alive: bool,
    pub spectator: bool,
    pub eligible: bool,
    pub loaded: bool,
    pub visibility: f64,
}
pub struct Sensor {
    pub delay: i64,
    pub nearest: Vec<i32>,
    pub attackable: Option<i32>,
    pub present: bool,
    pub range: f64,
    visible: HashMap<i32, bool>,
    rays: HashMap<i32, bool>,
}
fn intersects(a: [f64; 6], b: [f64; 6]) -> bool {
    a[0] < b[3] && a[3] > b[0] && a[1] < b[4] && a[4] > b[1] && a[2] < b[5] && a[5] > b[2]
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    let z = a[2] - b[2];
    x * x + y * y + z * z
}
impl Sensor {
    pub fn new(mut next_int: impl FnMut(i32) -> i32) -> Self {
        Self {
            delay: next_int(20) as i64,
            nearest: vec![],
            attackable: None,
            present: false,
            range: 16.0,
            visible: HashMap::new(),
            rays: HashMap::new(),
        }
    }
    pub fn tick(
        &mut self,
        no_ai: bool,
        origin: [f64; 3],
        mut bounds: [f64; 6],
        range: f64,
        targets: &[Target],
    ) -> Option<[f64; 6]> {
        if no_ai {
            return None;
        }
        self.rays.clear();
        self.delay = self.delay.wrapping_sub(1);
        if self.delay > 0 {
            return None;
        }
        self.delay = 20;
        self.range = range;
        for i in 0..3 {
            bounds[i] -= range;
            bounds[i + 3] += range;
        }
        let mut found: Vec<_> = targets
            .iter()
            .filter(|t| t.loaded && t.alive && intersects(bounds, t.bounds))
            .collect();
        found.sort_by(|a, b| distance(origin, a.position).total_cmp(&distance(origin, b.position)));
        self.nearest = found.iter().map(|t| t.id).collect();
        self.visible.clear();
        self.present = true;
        self.attackable = found
            .iter()
            .find(|t| t.player && t.eligible)
            .or_else(|| found.iter().find(|t| !t.player && t.eligible))
            .map(|t| t.id);
        Some(bounds)
    }
    pub fn contains(
        &mut self,
        origin: [f64; 3],
        attack: Option<i32>,
        target: &Target,
        mut ray: impl FnMut(i32) -> bool,
    ) -> bool {
        if !self.nearest.contains(&target.id) {
            return false;
        }
        if let Some(visible) = self.visible.get(&target.id) {
            return *visible;
        }
        let mut visible = target.alive && !target.spectator;
        if visible && self.range > 0.0 {
            let multiplier = if attack == Some(target.id) {
                1.0
            } else {
                target.visibility
            };
            let range = (self.range * multiplier).max(2.0);
            visible = distance(origin, target.position) <= range * range;
        }
        if visible {
            visible = *self.rays.entry(target.id).or_insert_with(|| ray(target.id));
        }
        self.visible.insert(target.id, visible);
        visible
    }
}
