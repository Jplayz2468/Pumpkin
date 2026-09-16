//! Server-side Entity.doWaterSplashEffect: sound and random consumption.
use pumpkin_util::{math::vector3::Vector3, random::RandomImpl};

pub fn sound_and_advance_random(
    movement: Vector3<f64>,
    controlling_passenger: bool,
    width: f32,
    random: &mut impl RandomImpl,
) -> (f32, f32) {
    let modifier = if controlling_passenger {
        0.9_f32
    } else {
        0.2_f32
    };
    let speed = ((movement.x * movement.x * f64::from(0.2_f32)
        + movement.y * movement.y
        + movement.z * movement.z * f64::from(0.2_f32))
    .sqrt() as f32
        * modifier)
        .min(1.0);
    let pitch = 1.0_f32 + (random.next_f32() - random.next_f32()) * 0.4_f32;
    // Level.addParticle is a no-op on the server, but Java still evaluates all
    // arguments. Preserve those draws so the next entity action sees the same RNG.
    let mut i = 0;
    while (i as f32) < 1.0_f32 + width * 20.0_f32 {
        random.next_f64();
        random.next_f64();
        random.next_f64();
        i += 1;
    }
    let mut i = 0;
    while (i as f32) < 1.0_f32 + width * 20.0_f32 {
        random.next_f64();
        random.next_f64();
        i += 1;
    }
    (speed, pitch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::random::legacy_rand::LegacyRand;

    #[test]
    fn java_splash_sound_and_following_random_draw() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("splash_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let double = |v: &serde_json::Value| f64::from_bits(v.as_u64().unwrap());
            let mut random = LegacyRand::from_seed(case[0].as_u64().unwrap());
            let (volume, pitch) = sound_and_advance_random(
                Vector3::new(
                    double(&case[1][0]),
                    double(&case[1][1]),
                    double(&case[1][2]),
                ),
                case[2].as_bool().unwrap(),
                f32::from_bits(case[3].as_u64().unwrap() as u32),
                &mut random,
            );
            assert_eq!(
                volume.to_bits(),
                case[4].as_u64().unwrap() as u32,
                "volume {index}"
            );
            assert_eq!(
                pitch.to_bits(),
                case[5].as_u64().unwrap() as u32,
                "pitch {index}"
            );
            assert_eq!(
                random.next_i64(),
                case[6].as_i64().unwrap(),
                "random {index}"
            );
            assert_eq!(case[7].as_u64().unwrap(), 1, "event {index}");
        }
    }
}
