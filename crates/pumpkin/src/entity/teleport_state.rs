//! PositionMoveRotation and passenger transition arithmetic.
use pumpkin_util::math::{cos, sin, vector3::Vector3};

pub const X: u16 = 1;
pub const Y: u16 = 2;
pub const Z: u16 = 4;
pub const Y_ROT: u16 = 8;
pub const X_ROT: u16 = 16;
pub const DELTA_X: u16 = 32;
pub const DELTA_Y: u16 = 64;
pub const DELTA_Z: u16 = 128;
pub const ROTATE_DELTA: u16 = 256;
pub const DELTA: u16 = DELTA_X | DELTA_Y | DELTA_Z | ROTATE_DELTA;
pub const ROTATION: u16 = Y_ROT | X_ROT;

#[derive(Clone, Copy, Debug)]
pub struct TeleportState {
    pub position: Vector3<f64>,
    pub velocity: Vector3<f64>,
    pub yaw: f32,
    pub pitch: f32,
}

impl TeleportState {
    pub fn of(entity: &super::Entity) -> Self {
        Self {
            position: entity.pos.load(),
            velocity: entity.velocity.load(),
            yaw: entity.yaw.load(),
            pitch: entity.pitch.load(),
        }
    }

    pub fn absolute(self, source: Self, relatives: u16) -> Self {
        let has = |flag| relatives & flag != 0;
        let add = |old: f64, new: f64, flag| if has(flag) { old + new } else { 0.0 + new };
        let yaw = (if has(Y_ROT) { source.yaw } else { 0.0 }) + self.yaw;
        let pitch = ((if has(X_ROT) { source.pitch } else { 0.0 }) + self.pitch).clamp(-90.0, 90.0);
        let mut velocity = source.velocity;
        if has(ROTATE_DELTA) {
            let x = f64::from(source.pitch - pitch).to_radians() as f32;
            let y = f64::from(source.yaw - yaw).to_radians() as f32;
            let (cx, sx, cy, sy) = (
                f64::from(cos(x)),
                f64::from(sin(x)),
                f64::from(cos(y)),
                f64::from(sin(y)),
            );
            velocity = Vector3::new(
                velocity.x,
                velocity.y * cx + velocity.z * sx,
                velocity.z * cx - velocity.y * sx,
            );
            velocity = Vector3::new(
                velocity.x * cy + velocity.z * sy,
                velocity.y,
                velocity.z * cy - velocity.x * sy,
            );
        }
        Self {
            position: Vector3::new(
                add(source.position.x, self.position.x, X),
                add(source.position.y, self.position.y, Y),
                add(source.position.z, self.position.z, Z),
            ),
            // Unlike positions, an absolute delta does not add positive zero.
            velocity: Vector3::new(
                if has(DELTA_X) {
                    velocity.x + self.velocity.x
                } else {
                    self.velocity.x
                },
                if has(DELTA_Y) {
                    velocity.y + self.velocity.y
                } else {
                    self.velocity.y
                },
                if has(DELTA_Z) {
                    velocity.z + self.velocity.z
                } else {
                    self.velocity.z
                },
            ),
            yaw,
            pitch,
        }
    }

    pub fn passenger(self, vehicle: Self, passenger: Self, relatives: u16) -> Self {
        let offset = passenger.position - vehicle.position;
        Self {
            position: self.position
                + Vector3::new(
                    if relatives & X != 0 { 0.0 } else { offset.x },
                    if relatives & Y != 0 { 0.0 } else { offset.y },
                    if relatives & Z != 0 { 0.0 } else { offset.z },
                ),
            velocity: self.velocity,
            yaw: self.yaw
                + if relatives & Y_ROT != 0 {
                    0.0
                } else {
                    passenger.yaw - vehicle.yaw
                },
            pitch: self.pitch
                + if relatives & X_ROT != 0 {
                    0.0
                } else {
                    passenger.pitch - vehicle.pitch
                },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(value: &serde_json::Value) -> TeleportState {
        let n = |i| f64::from_bits(value[i].as_u64().unwrap());
        TeleportState {
            position: Vector3::new(n(0), n(1), n(2)),
            velocity: Vector3::new(n(3), n(4), n(5)),
            yaw: n(6) as f32,
            pitch: n(7) as f32,
        }
    }
    fn bits(value: TeleportState) -> [u64; 8] {
        [
            value.position.x,
            value.position.y,
            value.position.z,
            value.velocity.x,
            value.velocity.y,
            value.velocity.z,
            f64::from(value.yaw),
            f64::from(value.pitch),
        ]
        .map(f64::to_bits)
    }
    #[test]
    fn java_relative_motion_and_passenger_transitions() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("teleport_state_cases.json")).unwrap();
        for (i, case) in cases.iter().enumerate() {
            let flags = case[0].as_u64().unwrap() as u16;
            let (source, change, rider) = (state(&case[1]), state(&case[2]), state(&case[3]));
            assert_eq!(
                bits(change.absolute(source, flags)),
                bits(state(&case[4])),
                "absolute {i}"
            );
            assert_eq!(
                bits(change.passenger(source, rider, flags)),
                bits(state(&case[5])),
                "passenger {i}"
            );
        }
    }
}

