//! Warden core look selection and investigation walk-target selection.
#[derive(Debug, Clone, Copy)]
pub struct WalkTarget {
    pub position: [i32; 3],
    pub speed: f32,
    pub distance: i32,
}
pub fn look_target(
    attack: bool,
    roar: Option<[i32; 3]>,
    disturbance: Option<[i32; 3]>,
) -> Option<[i32; 3]> {
    if attack { None } else { roar.or(disturbance) }
}
pub fn investigate(
    origin: [i32; 3],
    disturbance: Option<[i32; 3]>,
    attack: bool,
    walk: bool,
    mut next_int: impl FnMut(i32) -> i32,
) -> Option<WalkTarget> {
    let target = disturbance?;
    if attack || walk {
        return None;
    }
    let x = f64::from(target[0]) - f64::from(origin[0]);
    let y = f64::from(target[1]) - f64::from(origin[1]);
    let z = f64::from(target[2]) - f64::from(origin[2]);
    if x * x + y * y + z * z < 4.0 {
        return None;
    }
    Some(WalkTarget {
        position: [
            target[0].wrapping_add(next_int(3) - 1),
            target[1],
            target[2].wrapping_add(next_int(3) - 1),
        ],
        speed: 0.7,
        distance: 2,
    })
}
