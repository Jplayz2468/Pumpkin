//! Java 26.2 velocity restitution after clipping movement against collision shapes.
pub struct Facts {
    pub velocity: [f64; 3],
    pub actual: [f64; 3],
    pub collision_x: bool,
    pub collision_z: bool,
    pub vertical: bool,
    pub below: bool,
    pub suppress: bool,
    pub block_suppresses: bool,
    pub bounce: f64,
    pub block_bounce: f32,
    pub gravity: f64,
    pub drag: f32,
}
pub fn applies_position(requested_squared: f64, actual_squared: f64) -> bool {
    actual_squared > 1.0e-7 || requested_squared - actual_squared < 1.0e-7
}

pub fn clipped(requested: f64, actual: f64) -> bool {
    !((actual - requested).abs() < f64::from(1.0e-5_f32))
}
pub fn restitute(f: Facts) -> ([f64; 3], bool) {
    let mut bounce = if f.suppress { 0.0 } else { f.bounce };
    let mut velocity = f.velocity;
    if f.collision_x {
        velocity[0] = -f.velocity[0] * bounce;
    }
    if f.collision_z {
        velocity[2] = -f.velocity[2] * bounce;
    }
    let mut bounced = bounce > 0.0 && (f.collision_x || f.collision_z);
    if f.vertical {
        if f.below {
            bounce = if -f.velocity[1] < f.gravity || f.suppress || f.block_suppresses {
                0.0
            } else {
                bounce.max(f64::from(f.block_bounce))
            };
        }
        let (gravity, drag) = if bounce > 0.0 {
            let fraction = f.actual[1] / f.velocity[1];
            bounced = true;
            (
                fraction * f.gravity,
                1.0 + fraction * (f64::from(f.drag) - 1.0),
            )
        } else {
            (0.0, 1.0)
        };
        velocity[1] = (gravity - f.velocity[1]) * drag * bounce;
    }
    (velocity, bounced)
}

/// Complete nonzero vanilla26.2 block restitution table, verified against every block.
pub fn block_bounce(name: &str) -> f32 {
    if name == "slime_block" {
        1.0
    } else if name.ends_with("_bed") {
        0.75
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::tag::{self, Taggable};
    #[test]
    fn blocked_motion_omits_tiny_position_changes_but_free_motion_keeps_them() {
        assert!(applies_position(1.0e-10, 1.0e-10));
        assert!(!applies_position(1.0, 1.0e-10));
        assert!(!applies_position(1.0, 0.0));
        assert!(applies_position(1.0, 1.0e-6));
    }

    #[test]
    fn java_restitution_for_external_motion_and_nonliving_blocks() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("collision_restitution_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let vector = |i: usize| -> [f64; 3] {
                std::array::from_fn(|axis| f64::from_bits(case[i][axis].as_u64().unwrap()))
            };
            let state = pumpkin_data::BlockStateId::new(case[0].as_u64().unwrap() as u16)
                .unwrap()
                .to_state();
            let block = state.id.to_block();
            let flag = |i: usize| case[i].as_bool().unwrap();
            let parameters = vector(8);
            let (actual, bounced) = restitute(Facts {
                velocity: vector(1),
                actual: vector(2),
                collision_x: flag(3),
                collision_z: flag(4),
                vertical: flag(5),
                below: flag(6),
                suppress: flag(7),
                block_suppresses: block.has_tag(&tag::Block::MINECRAFT_SUPPRESSES_BOUNCE),
                bounce: parameters[0],
                gravity: parameters[1],
                drag: parameters[2] as f32,
                block_bounce: block_bounce(block.name) * 0.8_f32,
            });
            for (axis, expected) in vector(9).into_iter().enumerate() {
                if expected.is_nan() {
                    assert!(actual[axis].is_nan(), "case {index}, axis {axis}");
                } else {
                    assert_eq!(
                        actual[axis].to_bits(),
                        expected.to_bits(),
                        "case {index}, axis {axis}"
                    );
                }
            }
            assert_eq!(bounced, flag(10), "case {index}");
        }
    }
}
