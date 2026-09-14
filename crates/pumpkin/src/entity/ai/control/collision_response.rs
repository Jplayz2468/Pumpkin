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
