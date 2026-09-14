//! Java water/lava acceleration and post-collision fluid movement.
pub fn water_parameters(
    sprint: bool,
    slowdown: f32,
    efficiency: f64,
    ground: bool,
    speed: f32,
    dolphin: bool,
) -> [f32; 2] {
    let mut friction = if sprint { 0.9_f32 } else { slowdown };
    let mut acceleration = 0.02_f32;
    let mut efficiency = efficiency as f32;
    if !ground {
        efficiency *= 0.5_f32;
    }
    if efficiency > 0.0 {
        friction += (0.546_000_06_f32 - friction) * efficiency;
        acceleration += (speed - acceleration) * efficiency;
    }
    if dolphin {
        friction = 0.96_f32;
    }
    [acceleration, friction]
}
pub fn falling_y(mut y: f64, gravity: f64, falling: bool, sprint: bool) -> f64 {
    if gravity != 0.0 && !sprint {
        if falling && (y - 0.005).abs() >= 0.003 && (y - gravity / 16.0).abs() < 0.003 {
            y = -0.003;
        } else {
            y -= gravity / 16.0;
        }
    }
    y
}
pub fn after_move(
    mut v: [f64; 3],
    water: bool,
    friction: f32,
    climbing_collision: bool,
    shallow: bool,
    gravity: f64,
    falling: bool,
    sprint: bool,
) -> [f64; 3] {
    if water {
        if climbing_collision {
            v[1] = 0.2;
        }
        v = [
            v[0] * f64::from(friction),
            v[1] * f64::from(0.8_f32),
            v[2] * f64::from(friction),
        ];
        v[1] = falling_y(v[1], gravity, falling, sprint);
    } else {
        if shallow {
            v = [v[0] * 0.5, v[1] * f64::from(0.8_f32), v[2] * 0.5];
            v[1] = falling_y(v[1], gravity, falling, sprint);
        } else {
            v = v.map(|n| n * 0.5);
        }
        if gravity != 0.0 {
            v[1] += -gravity / 4.0;
        }
    }
    v
}
pub fn escape_query(v: [f64; 3], y: f64, old_y: f64) -> [f64; 3] {
    [v[0], v[1] + f64::from(0.6_f32) - y + old_y, v[2]]
}
pub fn escape_velocity(mut v: [f64; 3]) -> [f64; 3] {
    v[1] = f64::from(0.3_f32);
    v
}
