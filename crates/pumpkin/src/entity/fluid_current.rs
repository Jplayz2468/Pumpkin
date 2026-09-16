//! EntityFluidInteraction.Tracker.applyCurrentTo (Java 26.2).
use pumpkin_util::math::vector3::Vector3;

fn normalize(value: Vector3<f64>) -> Vector3<f64> {
    let length = value.length();
    if length < f64::from(1.0e-5_f32) {
        Vector3::new(0.0, 0.0, 0.0)
    } else {
        Vector3::new(value.x / length, value.y / length, value.z / length)
    }
}

pub fn apply(
    old: Vector3<f64>,
    accumulated: Vector3<f64>,
    count: usize,
    player: bool,
    scale: f64,
) -> Vector3<f64> {
    if count == 0 || accumulated.length_squared() < f64::from(1.0e-5_f32) {
        return old;
    }
    let mut impulse = if player {
        accumulated * (1.0 / count as f64)
    } else {
        normalize(accumulated)
    } * scale;
    if old.x.abs() < 0.003 && old.z.abs() < 0.003 && impulse.length() < 0.004_500_000_000_000_000_5
    {
        impulse = normalize(impulse) * 0.004_500_000_000_000_000_5;
    }
    old + impulse
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_tracker_currents() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fluid_current_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let vector = |v: &serde_json::Value| {
                Vector3::new(
                    f64::from_bits(v[0].as_u64().unwrap()),
                    f64::from_bits(v[1].as_u64().unwrap()),
                    f64::from_bits(v[2].as_u64().unwrap()),
                )
            };
            let flows = case[3].as_array().unwrap();
            let accumulated = flows
                .iter()
                .fold(Vector3::new(0.0, 0.0, 0.0), |sum, v| sum + vector(v));
            let actual = apply(
                vector(&case[1]),
                accumulated,
                flows.len(),
                case[0].as_bool().unwrap(),
                f64::from_bits(case[2][0].as_u64().unwrap()),
            );
            let expected = vector(&case[4]);
            assert_eq!(
                [actual.x.to_bits(), actual.y.to_bits(), actual.z.to_bits()],
                [
                    expected.x.to_bits(),
                    expected.y.to_bits(),
                    expected.z.to_bits()
                ],
                "case {index}"
            );
        }
    }
}
