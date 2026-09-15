//! LivingEntity's additional melee knockback, after a successful damage attempt.
//!
//! Ports vanilla `LivingEntity.causeExtraKnockback` (`LivingEntity.java:2745-2759`)
//! together with the `LivingEntity.knockback` formula it calls into
//! (`LivingEntity.java:1641-1659`):
//!
//! ```java
//! public void causeExtraKnockback(target, knockback, oldMovement, damageSource, damage, comesFromEffect) {
//!     if (knockback > 0.0F && target instanceof LivingEntity livingTarget) {
//!         livingTarget.knockback(knockback, sin(yRot), -cos(yRot), damageSource, damage, comesFromEffect);
//!         this.setDeltaMovement(this.getDeltaMovement().multiply(0.6, 1.0, 0.6));
//!     }
//! }
//!
//! public void knockback(double power, double xd, double zd, source, damage, comesFromEffect) {
//!     power *= 1.0 - this.getAttributeValue(Attributes.KNOCKBACK_RESISTANCE);
//!     if (!(power <= 0.0)) {
//!         this.needsSync = true;
//!         Vec3 deltaMovement = this.getDeltaMovement();
//!         Vec3 deltaVector = new Vec3(xd, 0.0, zd).normalize().scale(power);
//!         this.setDeltaMovement(
//!             deltaMovement.x / 2.0 - deltaVector.x,
//!             this.onGround() ? Math.min(0.4, deltaMovement.y / 2.0 + power) : deltaMovement.y,
//!             deltaMovement.z / 2.0 - deltaVector.z
//!         );
//!     }
//! }
//! ```
//!
//! `strength` here corresponds to vanilla's `knockback` argument - the caller-side
//! `LivingEntity.getKnockback` result (`attribute / 2.0F`, `LivingEntity.java:1540-1544`,
//! enchantment scaling not modelled here since callers of this pure function do not yet
//! thread enchantments through). The outer `strength > 0.0 && living` gate mirrors
//! `knockback > 0.0F && target instanceof LivingEntity`, which gates *both* whether the
//! target is pushed and whether the attacker recoils by `*0.6` - the recoil happens even
//! when the target's own push is fully absorbed by resistance, because vanilla only
//! re-checks `power <= 0.0` *inside* `knockback()`, after the recoil already ran.
//! `sync` mirrors `this.needsSync = true`, which vanilla only sets once resistance has
//! been applied and the post-resistance `power` is still positive - callers use it to
//! decide whether to mark the victim entity's velocity dirty for resync to the client
//! (a missing resync here makes the knockback invisible on the client even though the
//! server applied it).
//!
//! The direction vector fed to `knockback()` is always `(sin(yRot), -cos(yRot))`, which
//! is already unit-length, so vanilla's near-zero-vector jitter loop in `knockback()`
//! (only needed for direct position-delta callers such as `dealDefaultKnockback`) can
//! never trigger here and is intentionally not ported into this function.
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

#[cfg(test)]
mod tests {
    use super::*;

    // Pins the vector for `attribute = 1.0` (-> `strength = attribute / 2.0 = 0.5`,
    // `LivingEntity.java:1541`), `resistance = 0.25` (-> `scaled = 0.5 * 0.75 = 0.375`,
    // `LivingEntity.java:1642`) and `yaw = 0.0` against the real production function,
    // rather than re-deriving the formula here. At yaw 0, `sin(yRot) == 0.0` and
    // `-cos(yRot) == -1.0` (LivingEntity.java:2751-2752), so the knockback direction is
    // straight down -Z and the resulting vector is easy to check by hand:
    //   target: x/2 - 0.0 = 0.0; on ground -> min(0.4, 0.0/2 + 0.375) = 0.375 (the two
    //     most commonly mis-ported parts of this formula: halving the existing velocity
    //     before adding, and clamping the vertical component to 0.4); z/2 - (-0.375) = 0.375
    //   actor (attacker recoil, `causeExtraKnockback`'s `*0.6, 1.0, 0.6`): [1.2, 3.0, 1.2]
    #[test]
    fn knockback_vector_matches_hand_worked_values() {
        let result = apply(
            1.0,
            0.0,
            true,
            0.25,
            true,
            [2.0, 3.0, 2.0],
            [0.0, 0.0, 0.0],
        );

        assert_eq!(result.strength, 0.5);
        assert!(result.sync);

        let eps = 1e-4;
        assert!(result.target_velocity[0].abs() < eps);
        assert!((result.target_velocity[1] - 0.375).abs() < eps);
        assert!((result.target_velocity[2] - 0.375).abs() < eps);

        assert!((result.actor_velocity[0] - 1.2).abs() < eps);
        assert!((result.actor_velocity[1] - 3.0).abs() < eps);
        assert!((result.actor_velocity[2] - 1.2).abs() < eps);
    }

    // `LivingEntity.knockback` scales by `1.0 - KNOCKBACK_RESISTANCE` and then early-outs
    // on `power <= 0.0` (LivingEntity.java:1642-1643) before ever setting `needsSync` -
    // full resistance (iron golem / warden: `KNOCKBACK_RESISTANCE == 1.0`) must leave the
    // target's velocity untouched and unsynced, even though the attacker still recoils
    // (the recoil in `causeExtraKnockback` is gated on the pre-resistance `knockback`
    // argument, not on whether the push actually landed).
    #[test]
    fn full_resistance_cancels_target_push_but_not_attacker_recoil() {
        let result = apply(
            1.0,
            0.0,
            true,
            1.0,
            true,
            [2.0, 3.0, 2.0],
            [0.0, 0.0, 0.0],
        );

        assert!(!result.sync);
        assert_eq!(result.target_velocity, [0.0, 0.0, 0.0]);
        assert_eq!(result.actor_velocity, [1.2, 3.0, 1.2]);
    }
}
