//! LivingEntity auto-spin lifetime and Player/Trident launch state.
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::math::{cos, sin, vector3::Vector3};

#[derive(Default)]
pub struct AutoSpin {
    pub ticks: i32,
    pub damage: f32,
    pub item: Option<ItemStack>,
}

impl AutoSpin {
    pub fn start(&mut self, ticks: i32, damage: f32, item: ItemStack) {
        self.ticks = ticks;
        self.damage = damage;
        self.item = Some(item);
    }

    pub fn begin_tick(&mut self) -> bool {
        if self.ticks <= 0 {
            return false;
        }
        self.ticks -= 1;
        true
    }

    /// Non-living entities in the query deliberately prevent the wall-only stop.
    pub fn finish_tick(
        &mut self,
        any_entities: bool,
        hit_living: bool,
        horizontal_collision: bool,
    ) -> bool {
        if hit_living || (!any_entities && horizontal_collision) {
            self.ticks = 0;
        }
        if self.ticks <= 0 {
            self.damage = 0.0;
            self.item = None;
            true
        } else {
            false
        }
    }
}

pub fn launch(yaw: f32, pitch: f32, strength: f32) -> Vector3<f64> {
    let yaw = yaw * (std::f64::consts::PI / 180.0) as f32;
    let pitch = pitch * (std::f64::consts::PI / 180.0) as f32;
    let x = -sin(yaw) * cos(pitch);
    let y = -sin(pitch);
    let z = cos(yaw) * cos(pitch);
    let distance = (x * x + y * y + z * z).sqrt();
    let scale = strength / distance;
    Vector3::new(
        f64::from(x * scale),
        f64::from(y * scale),
        f64::from(z * scale),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::item::Item;

    #[test]
    fn java_auto_spin_lifetime_contacts_and_rebound() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("auto_spin_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let mut spin = AutoSpin::default();
            spin.start(
                case[0].as_i64().unwrap() as i32,
                8.0,
                ItemStack::new(1, &Item::TRIDENT),
            );
            assert!(spin.begin_tick());
            let kind = case[1].as_u64().unwrap();
            let stopped = spin.finish_tick(kind != 0, kind >= 2, case[2].as_bool().unwrap());
            let expected = &case[4];
            assert_eq!(
                spin.ticks,
                expected[0].as_i64().unwrap() as i32,
                "ticks {index}"
            );
            assert_eq!(
                spin.damage,
                expected[1].as_f64().unwrap() as f32,
                "damage {index}"
            );
            assert_eq!(
                spin.item.is_some(),
                expected[2].as_bool().unwrap(),
                "weapon {index}"
            );
            assert_eq!(!stopped, expected[3].as_bool().unwrap(), "flag {index}");
            let motion = &case[3];
            for axis in 0..3 {
                let speed = f64::from_bits(motion[axis].as_u64().unwrap());
                let actual = if kind >= 2 { speed * -0.2 } else { speed };
                assert_eq!(
                    actual.to_bits(),
                    expected[5][axis].as_u64().unwrap(),
                    "velocity {index}/{axis}"
                );
            }
        }
    }

    #[test]
    fn java_trident_launch_float_arithmetic() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("trident_launch_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let value = |i| f32::from_bits(case[i].as_u64().unwrap() as u32);
            let actual = launch(value(0), value(1), value(2));
            assert_eq!(
                [actual.x.to_bits(), actual.y.to_bits(), actual.z.to_bits()],
                [
                    case[3][0].as_u64().unwrap(),
                    case[3][1].as_u64().unwrap(),
                    case[3][2].as_u64().unwrap()
                ],
                "launch {index}"
            );
        }
    }
}
