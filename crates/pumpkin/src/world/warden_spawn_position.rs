//! Collision-face and downward-search primitives for Java Warden spawn attempts.
//! The search is validated independently before wiring the full creation/brain path.
use pumpkin_util::math::boundingbox::BoundingBox;

/// Project the collision at Java's UP face slice and require exact unit coverage.
/// Registry shapes are exact binary fractions; arbitrary plugin shapes need a separate gate.
pub fn full_top_face(shapes: impl IntoIterator<Item = BoundingBox>) -> bool {
    let rectangles: Vec<_> = shapes
        .into_iter()
        .filter(|b| b.min.y <= 0.9999999 && b.max.y > 0.9999999)
        .collect();
    if rectangles.is_empty() {
        return false;
    }
    if rectangles
        .iter()
        .any(|b| b.min.x < 0.0 || b.max.x > 1.0 || b.min.z < 0.0 || b.max.z > 1.0)
    {
        return false;
    }
    let mut xs = vec![0.0, 1.0];
    let mut zs = vec![0.0, 1.0];
    for b in &rectangles {
        xs.extend([b.min.x, b.max.x]);
        zs.extend([b.min.z, b.max.z]);
    }
    xs.sort_unstable_by(f64::total_cmp);
    xs.dedup();
    zs.sort_unstable_by(f64::total_cmp);
    zs.dedup();
    xs.windows(2).all(|x| {
        zs.windows(2).all(|z| {
            rectangles
                .iter()
                .any(|b| b.min.x <= x[0] && b.max.x >= x[1] && b.min.z <= z[0] && b.max.z >= z[1])
        })
    })
}

pub fn move_to_possible_spawn_position<S>(
    y: &mut i32,
    range: i32,
    mut read: impl FnMut(i32) -> S,
    mut can_spawn: impl FnMut(&S, &S) -> bool,
) -> bool {
    let mut above = read(*y);
    for _ in -range..=range {
        *y -= 1;
        let floor = read(*y);
        if can_spawn(&floor, &above) {
            *y += 1;
            return true;
        }
        above = floor;
    }
    false
}
