//! Java LookControl's angle calculation, including its approximate atan2.
include!("look_math_table.rs");
pub fn atan2(mut y: f64, mut x: f64) -> f64 {
    let square = x * x + y * y;
    if square.is_nan() {
        return f64::NAN;
    }
    let negative_y = y < 0.0;
    if negative_y {
        y = -y;
    }
    let negative_x = x < 0.0;
    if negative_x {
        x = -x;
    }
    let swap = y > x;
    if swap {
        std::mem::swap(&mut x, &mut y);
    }
    let half = 0.5 * square;
    let guess = f64::from_bits(
        (6910469410427058090_i64.wrapping_sub((square.to_bits() as i64) >> 1)) as u64,
    );
    let inverse = guess * (1.5 - half * guess * guess);
    x *= inverse;
    y *= inverse;
    let bias = f64::from_bits(4805340802404319232);
    let rounded = bias + y;
    let index = rounded.to_bits() as u32 as usize;
    let angle = f64::from_bits(TABLE[index][0]);
    let cosine = f64::from_bits(TABLE[index][1]);
    let residual = y * cosine - x * (rounded - bias);
    let mut result = angle + (6.0 + residual * residual) * residual * (1.0 / 6.0);
    if swap {
        result = std::f64::consts::FRAC_PI_2 - result;
    }
    if negative_x {
        result = std::f64::consts::PI - result;
    }
    if negative_y {
        result = -result;
    }
    result
}
pub fn pitch(dx: f64, dy: f64, dz: f64) -> Option<f32> {
    let horizontal = (dx * dx + dz * dz).sqrt();
    if dy.abs() <= f64::from(1.0e-5_f32) && horizontal.abs() <= f64::from(1.0e-5_f32) {
        None
    } else {
        Some((-atan2(dy, horizontal) * f64::from(180.0_f32 / std::f32::consts::PI)) as f32)
    }
}
pub fn yaw(dx: f64, dz: f64) -> Option<f32> {
    if dz.abs() <= f64::from(1.0e-5_f32) && dx.abs() <= f64::from(1.0e-5_f32) {
        None
    } else {
        Some((atan2(dz, dx) * f64::from(180.0_f32 / std::f32::consts::PI)) as f32 - 90.0)
    }
}
