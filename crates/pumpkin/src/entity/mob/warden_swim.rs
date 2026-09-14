//! Brain Swim(0.8), with separate world duration and entity chance randomness.
#[derive(Default)]
pub struct Swim {
    pub end_timestamp: Option<i64>,
}
pub fn should_swim(water: bool, height: f64, threshold: f64, lava: bool) -> bool {
    (water && height > threshold) || lava
}
impl Swim {
    pub fn start(&mut self, time: i64, eligible: bool, mut world_int: impl FnMut(i32) -> i32) {
        if self.end_timestamp.is_none() && eligible {
            self.end_timestamp = Some(time.wrapping_add(i64::from(60 + world_int(1))));
        }
    }
    pub fn run(
        &mut self,
        time: i64,
        eligible: bool,
        mut entity_float: impl FnMut() -> f32,
    ) -> bool {
        let Some(end) = self.end_timestamp else {
            return false;
        };
        if time > end || !eligible {
            self.end_timestamp = None;
            return false;
        }
        entity_float() < 0.8_f32
    }
}
