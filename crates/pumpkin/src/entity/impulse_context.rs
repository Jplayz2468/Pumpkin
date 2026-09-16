//! LivingEntity impulse fall-damage context, including the grace-period lifecycle.
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::vector3::Vector3;

#[derive(Default)]
pub(crate) struct ImpulseContext {
    pub impact: Option<Vector3<f64>>,
    pub grace: i32,
}
impl ImpulseContext {
    pub fn set_ignore(&mut self, ignore: bool, position: Vector3<f64>) {
        if ignore {
            self.apply_grace(40);
            self.impact = Some(position);
        } else {
            self.grace = 0;
        }
    }
    pub fn apply_grace(&mut self, ticks: i32) {
        self.grace = self.grace.max(ticks);
    }
    pub fn tick(&mut self) {
        if self.grace > 0 {
            self.grace -= 1;
        }
    }
    pub fn reset(&mut self) {
        self.grace = 0;
        self.impact = None;
    }
    pub fn try_reset(&mut self) {
        if self.grace == 0 {
            self.reset();
        }
    }
    pub fn fall_distance(&mut self, distance: f64, y: f64) -> f64 {
        let Some(impact) = self.impact else {
            return distance;
        };
        let effective = distance.min(impact.y - y);
        if effective <= 0.0 {
            self.reset();
        } else {
            self.try_reset();
        }
        effective
    }
    pub fn mace_impact(&self, position: Vector3<f64>) -> Vector3<f64> {
        self.impact
            .filter(|old| old.y <= position.y)
            .unwrap_or(position)
    }
    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("current_impulse_context_reset_grace_time", self.grace);
        if let Some(pos) = self.impact {
            nbt.put(
                "current_explosion_impact_pos",
                NbtTag::List(vec![pos.x.into(), pos.y.into(), pos.z.into()]),
            );
        }
    }
    pub fn read_nbt(&mut self, nbt: &NbtCompound) {
        self.grace = nbt
            .get_int("current_impulse_context_reset_grace_time")
            .unwrap_or(0);
        self.impact = nbt.get_list("current_explosion_impact_pos").and_then(|v| {
            if v.len() != 3 {
                return None;
            }
            Some(Vector3::new(
                v[0].extract_double()?,
                v[1].extract_double()?,
                v[2].extract_double()?,
            ))
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_impulse_transitions_and_fall_clamping() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("impulse_context_cases.json")).unwrap();
        let mut context = ImpulseContext::default();
        for (index, case) in cases.iter().enumerate() {
            let vector = |v: &serde_json::Value| {
                Vector3::new(
                    f64::from_bits(v[0].as_u64().unwrap()),
                    f64::from_bits(v[1].as_u64().unwrap()),
                    f64::from_bits(v[2].as_u64().unwrap()),
                )
            };
            let pos = vector(&case[2]);
            match case[0].as_u64().unwrap() {
                0 => context.set_ignore(true, pos),
                1 => context.set_ignore(false, pos),
                2 => context.apply_grace(case[1].as_i64().unwrap() as i32),
                3 => context.try_reset(),
                4 => context.reset(),
                5 => {
                    let value =
                        context.fall_distance(f64::from_bits(case[3][0].as_u64().unwrap()), pos.y);
                    assert_eq!(
                        value.to_bits(),
                        case[4][0].as_u64().unwrap(),
                        "fall case {index}"
                    );
                }
                _ => unreachable!(),
            }
            assert_eq!(
                context.grace,
                case[5].as_i64().unwrap() as i32,
                "grace case {index}"
            );
            assert_eq!(
                context.impact,
                (!case[6].is_null()).then(|| vector(&case[6])),
                "impact case {index}"
            );
            assert_eq!(
                context.mace_impact(pos),
                vector(&case[7]),
                "mace case {index}"
            );
            let mut nbt = NbtCompound::new();
            context.write_nbt(&mut nbt);
            let mut loaded = ImpulseContext::default();
            loaded.read_nbt(&nbt);
            assert_eq!(context.impact, loaded.impact);
            assert_eq!(context.grace, loaded.grace);
        }
    }
}
