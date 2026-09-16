//! Port of `LongJumpUtil`: the ballistic maths behind a frog's or goat's long jump.

use crate::entity::Entity;
use pumpkin_util::math::vector3::Vector3;

/// Vanilla scales the final velocity by 0.95 to keep the landing inside the target block.
const VELOCITY_SCALE: f64 = 0.95;

/// `LongJumpUtil.calculateJumpVectorForAngle`.
///
/// Solves the projectile equation for a fixed launch angle: given where the mob is and
/// where it wants to land, find the launch velocity that gets it there under gravity.
/// Returns `None` when no solution exists, or when the required speed exceeds what the
/// mob can manage -- which is how vanilla rejects a jump that is too far or too high.
///
/// Known gap: vanilla also walks the trajectory and rejects a jump that would clip
/// scenery (`isClearTransition`). That collision sweep is not implemented here, so a frog
/// may attempt a jump whose arc passes through a block.
#[must_use]
pub fn calculate_jump_vector_for_angle(
    entity: &Entity,
    target: Vector3<f64>,
    max_jump_velocity: f32,
    angle_degrees: i32,
) -> Option<Vector3<f64>> {
    let mob_pos = entity.pos.load();

    // Aim half a block short, as vanilla does, so the mob lands on the block rather than
    // its far edge.
    let plane = Vector3::new(target.x - mob_pos.x, 0.0, target.z - mob_pos.z);
    let plane_len = (plane.x * plane.x + plane.z * plane.z).sqrt();
    if plane_len <= f64::EPSILON {
        return None;
    }
    let direction_plane = Vector3::new(plane.x / plane_len * 0.5, 0.0, plane.z / plane_len * 0.5);
    let aim = target - direction_plane;
    let to_aim = aim - mob_pos;

    let angle = f64::from(angle_degrees).to_radians();
    let xz_angle = to_aim.z.atan2(to_aim.x);
    let r2 = to_aim.x * to_aim.x + to_aim.z * to_aim.z;
    let r = r2.sqrt();
    let y = to_aim.y;
    let gravity = f64::from(entity.entity_type.attributes.iter().find_map(|(attribute, value)| {
        (attribute.id == pumpkin_data::attributes::Attributes::GRAVITY.id).then_some(*value)
    }).unwrap_or(0.08) as f32);

    let sin_2ang = (2.0 * angle).sin();
    let cos_ang_sqr = angle.cos().powi(2);
    let denominator = r * sin_2ang - 2.0 * y * cos_ang_sqr;
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let v0_sqr = r2 * gravity / denominator;
    if v0_sqr < 0.0 {
        return None;
    }
    let v0 = v0_sqr.sqrt();
    if v0 > f64::from(max_jump_velocity) {
        return None;
    }

    let v0_r = v0 * angle.cos();
    let v0_y = v0 * angle.sin();
    Some(Vector3::new(
        v0_r * xz_angle.cos() * VELOCITY_SCALE,
        v0_y * VELOCITY_SCALE,
        v0_r * xz_angle.sin() * VELOCITY_SCALE,
    ))
}
