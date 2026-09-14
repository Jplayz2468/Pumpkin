//! Java MoveControl MOVE_TO calculations; collision queries remain an explicit boundary.
#[derive(Debug)]
pub struct Command {
    pub stop_forward: bool,
    pub yaw: f32,
    pub speed: f32,
    pub jump: bool,
}
pub struct Facts {
    pub delta: [f64; 3],
    pub yaw: f32,
    pub attribute: f64,
    pub modifier: f64,
    pub step_height: f32,
    pub width: f32,
    pub feet_y: f64,
    pub collision_top: Option<f64>,
    pub door_or_fence: bool,
}
pub fn rotate(start: f32, end: f32) -> f32 {
    let mut delta = (end - start) % 360.0;
    if delta >= 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    let mut result = start + delta.clamp(-90.0, 90.0);
    if result < 0.0 {
        result += 360.0;
    } else if result > 360.0 {
        result -= 360.0;
    }
    result
}
pub fn command(facts: Facts) -> Command {
    let [dx, dy, dz] = facts.delta;
    if dx * dx + dy * dy + dz * dz < f64::from(2.5000003e-7_f32) {
        return Command {
            stop_forward: true,
            yaw: facts.yaw,
            speed: 0.0,
            jump: false,
        };
    }
    let target = (super::look_math::atan2(dz, dx) * f64::from(180.0_f32 / std::f32::consts::PI))
        as f32
        - 90.0;
    Command {
        stop_forward: false,
        yaw: rotate(facts.yaw, target),
        speed: (facts.modifier * facts.attribute) as f32,
        jump: (dy > f64::from(facts.step_height)
            && dx * dx + dz * dz < f64::from(1.0_f32.max(facts.width)))
            || (facts.collision_top.is_some_and(|top| facts.feet_y < top) && !facts.door_or_fence),
    }
}
