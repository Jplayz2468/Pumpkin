//! Java RandomPos direction sampler used by DefaultRandomPos fallback.
use crate::entity::ai::control::look_math;
use pumpkin_util::random::RandomImpl;
pub fn direction(
    random: &mut impl RandomImpl,
    minimum: f64,
    maximum: f64,
    vertical: i32,
    vertical_offset: i32,
    dx: f64,
    dz: f64,
    spread: f64,
) -> Option<[i32; 3]> {
    let base = look_math::atan2(dz, dx) - f64::from(std::f32::consts::FRAC_PI_2);
    let angle = base + f64::from(2.0_f32 * random.next_f32() - 1.0_f32) * spread;
    let radius =
        (minimum + random.next_f64().sqrt() * (maximum - minimum)) * f64::from(2.0_f32.sqrt());
    let x = -radius * angle.sin();
    let z = radius * angle.cos();
    if x.abs() > maximum || z.abs() > maximum {
        return None;
    }
    let y = random.next_bounded_i32(2 * vertical + 1) - vertical + vertical_offset;
    Some([x.floor() as i32, y, z.floor() as i32])
}
#[derive(Clone, Copy)]
pub struct Search {
    pub origin: [f64; 3],
    pub horizontal: i32,
    pub vertical: i32,
    pub target: Option<[f64; 3]>,
    pub spread: f64,
    pub home: Option<([i32; 3], i32)>,
    pub min_y: i32,
    pub max_y: i32,
}
pub fn choose(
    random: &mut impl RandomImpl,
    facts: Search,
    mut score: impl FnMut([i32; 3]) -> Option<f64>,
) -> Option<[f64; 3]> {
    choose_transformed(random, facts, |p| score(p).map(|value| (p, value)))
}
/// Land destinations may move upward before scoring; restriction and stability
/// checks still use the original sampled candidate.
pub fn choose_transformed(
    random: &mut impl RandomImpl,
    facts: Search,
    mut evaluate: impl FnMut([i32; 3]) -> Option<([i32; 3], f64)>,
) -> Option<[f64; 3]> {
    let horizontal = f64::from(facts.horizontal);
    let restricted = facts.home.is_some_and(|(p, radius)| {
        let delta = [
            f64::from(p[0]) + 0.5 - facts.origin[0],
            f64::from(p[1]) + 0.5 - facts.origin[1],
            f64::from(p[2]) + 0.5 - facts.origin[2],
        ];
        let range = f64::from(radius) + horizontal + 1.0;
        delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2] < range * range
    });
    let mut best = None;
    let mut best_score = f64::NEG_INFINITY;
    for _ in 0..10 {
        let offset = if let Some(target) = facts.target {
            let Some(offset) = direction(
                random,
                0.0,
                horizontal,
                facts.vertical,
                0,
                target[0] - facts.origin[0],
                target[2] - facts.origin[2],
                facts.spread,
            ) else {
                continue;
            };
            offset
        } else {
            [
                random.next_bounded_i32(2 * facts.horizontal + 1) - facts.horizontal,
                random.next_bounded_i32(2 * facts.vertical + 1) - facts.vertical,
                random.next_bounded_i32(2 * facts.horizontal + 1) - facts.horizontal,
            ]
        };
        let mut x = f64::from(offset[0]);
        let mut z = f64::from(offset[2]);
        if let Some((home, _)) = facts.home
            && horizontal > 1.0
        {
            let bias = random.next_f64() * horizontal / 2.0;
            if facts.origin[0] > f64::from(home[0]) {
                x -= bias;
            } else {
                x += bias;
            }
            let bias = random.next_f64() * horizontal / 2.0;
            if facts.origin[2] > f64::from(home[2]) {
                z -= bias;
            } else {
                z += bias;
            }
        }
        let p = [
            (x + facts.origin[0]).floor() as i32,
            (f64::from(offset[1]) + facts.origin[1]).floor() as i32,
            (z + facts.origin[2]).floor() as i32,
        ];
        if p[1] < facts.min_y || p[1] >= facts.max_y {
            continue;
        }
        if restricted {
            let (home, radius) = facts.home.unwrap();
            let d = [
                f64::from(home[0]) - f64::from(p[0]),
                f64::from(home[1]) - f64::from(p[1]),
                f64::from(home[2]) - f64::from(p[2]),
            ];
            if !(d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < f64::from(radius.wrapping_mul(radius))) {
                continue;
            }
        }
        if let Some((p, value)) = evaluate(p)
            && value > best_score
        {
            best = Some(p);
            best_score = value;
        }
    }
    best.map(|p| {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]),
            f64::from(p[2]) + 0.5,
        ]
    })
}

/// LandRandomPos moves out of solid blocks, then rejects water or nonzero malus.
/// `max_y` is Java's inclusive maximum build Y.
pub fn land_candidate(
    mut position: [i32; 3],
    max_y: i32,
    mut solid: impl FnMut([i32; 3]) -> bool,
    mut water: impl FnMut([i32; 3]) -> bool,
    mut malus: impl FnMut([i32; 3]) -> f32,
) -> Option<[i32; 3]> {
    if solid(position) {
        position[1] = position[1].wrapping_add(1);
        while position[1] <= max_y && solid(position) {
            position[1] = position[1].wrapping_add(1);
        }
    }
    if water(position) || malus(position) != 0.0 {
        None
    } else {
        Some(position)
    }
}
