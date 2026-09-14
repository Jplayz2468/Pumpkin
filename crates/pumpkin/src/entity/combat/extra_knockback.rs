//! LivingEntity's additional melee knockback, after a successful damage attempt.
use pumpkin_util::math::{cos, sin};
#[derive(Debug)]
pub struct ExtraKnockback {
    pub strength: f32,
    pub sync: bool,
    pub actor_velocity: [f64; 3],
    pub target_velocity: [f64; 3],
}
pub fn apply(
    attribute: f64,
    yaw: f32,
    living: bool,
    resistance: f64,
    ground: bool,
    mut actor: [f64; 3],
    mut target: [f64; 3],
) -> ExtraKnockback {
    let strength = (attribute as f32) / 2.0;
    let mut sync = false;
    if strength > 0.0 && living {
        let radians = yaw * 0.017453292_f32;
        let x = f64::from(sin(radians));
        let z = f64::from(-cos(radians));
        let scaled = f64::from(strength) * (1.0 - resistance);
        if scaled > 0.0 {
            sync = true;
            let length = (x * x + z * z).sqrt();
            let nx = x / length * scaled;
            let nz = z / length * scaled;
            target = [
                target[0] / 2.0 - nx,
                if ground {
                    (target[1] / 2.0 + scaled).min(0.4)
                } else {
                    target[1]
                },
                target[2] / 2.0 - nz,
            ];
        }
        actor = [actor[0] * 0.6, actor[1], actor[2] * 0.6];
    }
    ExtraKnockback {
        strength,
        sync,
        actor_velocity: actor,
        target_velocity: target,
    }
}
