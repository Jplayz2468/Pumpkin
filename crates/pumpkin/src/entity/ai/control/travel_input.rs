//! Java Entity.getInputVector and LivingEntity.getFrictionInfluencedSpeed.
pub fn velocity(mut input: [f64; 3], speed: f32, yaw: f32) -> [f64; 3] {
    let length = input[0] * input[0] + input[1] * input[1] + input[2] * input[2];
    if length < 1.0e-7 {
        return [0.0; 3];
    }
    if length > 1.0 {
        let length = length.sqrt();
        input = input.map(|v| v / length);
    }
    input = input.map(|v| v * f64::from(speed));
    let angle = yaw * (std::f32::consts::PI / 180.0_f32);
    let sin = f64::from(pumpkin_util::math::sin(angle));
    let cos = f64::from(pumpkin_util::math::cos(angle));
    [
        input[0] * cos - input[2] * sin,
        input[1],
        input[2] * cos + input[0] * sin,
    ]
}
pub fn air_speed(speed: f32, ground: bool, slipperiness: f32, player_passenger: bool) -> f32 {
    if ground {
        if f64::from(slipperiness) > 0.6 {
            speed * (0.21600002_f32 / (slipperiness * slipperiness * slipperiness))
        } else {
            speed
        }
    } else if player_passenger {
        speed * 0.1_f32
    } else {
        0.02_f32
    }
}

pub fn modified_friction(base: f32, modifier: f32) -> f32 {
    (1.0_f32 - (1.0_f32 - base) * modifier).clamp(0.0, 1.0)
}
