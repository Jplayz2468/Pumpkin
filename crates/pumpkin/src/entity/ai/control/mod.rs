use crate::entity::mob::Mob;
use pumpkin_util::math::subtract_angles;

pub mod body_rotation_control;
mod body_rotation_math;
pub(crate) mod collision_response;
pub(crate) mod collision_shapes;
pub(crate) mod fluid_travel;
pub mod flying_move_control;
pub mod jump_control;
pub mod look_control;
pub(crate) mod look_math;
pub mod move_control;
mod movement_command;
pub mod smooth_swimming_look_control;
pub mod smooth_swimming_move_control;
pub(crate) mod travel_input;

pub trait Control: Send + Sync {
    fn change_angle(&self, start: f32, end: f32, max_change: f32) -> f32 {
        let i = subtract_angles(start, end);
        let j = i.clamp(-max_change, max_change);
        start + j
    }
}

pub trait MoveControlTrait: Control {
    fn tick(&mut self, mob: &dyn Mob);

    fn set_wanted_position(&mut self, _x: f64, _y: f64, _z: f64, _speed_modifier: f64) {}

    fn strafe(&mut self, _forward: f32, _right: f32) {}

    fn has_wanted(&self) -> bool {
        false
    }
}
