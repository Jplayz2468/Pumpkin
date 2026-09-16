//! Java 26.2 BlockGetter swept cell traversal and AABB.collidedAlongVector.
//! Keep ordering and step numbers: inside effects are grouped by traversal step.
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use rustc_hash::FxHashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Movement {
    pub from: Vector3<f64>,
    pub to: Vector3<f64>,
    pub original: Option<Vector3<f64>>,
}

/// Entity's bounded pending movement log and previous application paths.
#[derive(Default)]
pub(super) struct MovementHistory {
    pending: Vec<Movement>,
    previous: Vec<Movement>,
}
impl MovementHistory {
    pub fn push(&mut self, movement: Movement) {
        if self.pending.len() >= 100 {
            let first = self.pending.remove(0);
            let second = &mut self.pending[0];
            second.from = first.from;
            second.original = None;
        }
        self.pending.push(movement);
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.previous.clear();
    }
    pub fn finish(&mut self, old_position: Vector3<f64>, position: Vector3<f64>) -> Vec<Movement> {
        self.previous = std::mem::take(&mut self.pending);
        if self.previous.is_empty() {
            self.previous.push(Movement {
                from: old_position,
                to: position,
                original: None,
            });
        } else if let Some(last) = self.previous.last()
            && last.to.squared_distance_to_vec(&position) > f64::from(9.9999994e-11_f32)
        {
            self.previous.push(Movement {
                from: last.to,
                to: position,
                original: None,
            });
        }
        self.replay()
    }
    pub fn replay(&self) -> Vec<Movement> {
        self.previous.clone()
    }
}

pub const SKIN: f64 = 1.0e-5_f32 as f64;

fn array(v: Vector3<f64>) -> [f64; 3] {
    [v.x, v.y, v.z]
}
fn vector(v: [f64; 3]) -> Vector3<f64> {
    Vector3::new(v[0], v[1], v[2])
}
pub fn axis_order(delta: Vector3<f64>) -> [usize; 3] {
    if delta.x.abs() < delta.z.abs() {
        [1, 2, 0]
    } else {
        [1, 0, 2]
    }
}

fn between(
    first: [i32; 3],
    second: [i32; 3],
    direction: Vector3<f64>,
    mut visit: impl FnMut(BlockPos) -> bool,
) -> bool {
    let direction = array(direction);
    let min: [i32; 3] = std::array::from_fn(|i| first[i].min(second[i]));
    let max: [i32; 3] = std::array::from_fn(|i| first[i].max(second[i]));
    let start: [i32; 3] =
        std::array::from_fn(|i| if direction[i] >= 0.0 { min[i] } else { max[i] });
    let signs: [i32; 3] = std::array::from_fn(|i| if direction[i] >= 0.0 { 1 } else { -1 });
    let [a, b, c] = axis_order(vector(direction));
    for i in 0..=max[a] - min[a] {
        for j in 0..=max[b] - min[b] {
            for k in 0..=max[c] - min[c] {
                let mut pos = start;
                pos[a] += signs[a] * i;
                pos[b] += signs[b] * j;
                pos[c] += signs[c] * k;
                if !visit(BlockPos::new(pos[0], pos[1], pos[2])) {
                    return false;
                }
            }
        }
    }
    true
}

/// AABB.clip: only entering faces count; touching the ray's endpoints does not.
fn clip(bounds: BoundingBox, from: Vector3<f64>, to: Vector3<f64>) -> Option<Vector3<f64>> {
    let start = array(from);
    let delta = array(to - from);
    let min = array(bounds.min);
    let max = array(bounds.max);
    let mut scale = 1.0;
    let mut hit = false;
    for axis in 0..3 {
        if delta[axis].abs() <= 1.0e-7 {
            continue;
        }
        let face = if delta[axis] > 0.0 {
            min[axis]
        } else {
            max[axis]
        };
        let s = (face - start[axis]) / delta[axis];
        let b = (axis + 1) % 3;
        let c = (axis + 2) % 3;
        let pb = start[b] + s * delta[b];
        let pc = start[c] + s * delta[c];
        if s > 0.0
            && s < scale
            && min[b] - 1.0e-7 < pb
            && pb < max[b] + 1.0e-7
            && min[c] - 1.0e-7 < pc
            && pc < max[c] + 1.0e-7
        {
            scale = s;
            hit = true;
        }
    }
    hit.then(|| from + (to - from) * scale)
}

pub fn collided_along(
    bounds_at_from: BoundingBox,
    travel: Vector3<f64>,
    shape: BoundingBox,
) -> bool {
    let half = (bounds_at_from.max - bounds_at_from.min) * 0.5;
    let from = bounds_at_from.min + half;
    let to = from + travel;
    let inflated = shape.expand(half.x - 1.0e-7, half.y - 1.0e-7, half.z - 1.0e-7);
    let contains = |p: Vector3<f64>| {
        p.x >= inflated.min.x
            && p.x < inflated.max.x
            && p.y >= inflated.min.y
            && p.y < inflated.max.y
            && p.z >= inflated.min.z
            && p.z < inflated.max.z
    };
    contains(from) || contains(to) || clip(inflated, from, to).is_some()
}

/// The visitor may stop traversal at the engine's movement-iteration budget.
/// Cell deduplication here is local to one segment; callers deduplicate across
/// multiple axis segments and movement records as Entity.checkInsideBlocks does.
pub fn visit_swept(
    from: Vector3<f64>,
    to: Vector3<f64>,
    target: BoundingBox,
    mut visit: impl FnMut(BlockPos, i32) -> bool,
) -> bool {
    let travel = to - from;
    let floor = |v: Vector3<f64>| array(v).map(|v| v.floor() as i32);
    if travel.length_squared() < SKIN * SKIN {
        // BlockPos.betweenClosed is X-fastest, unlike the directional iterator.
        return BlockPos::iterate(
            BlockPos::floored_v(target.min),
            BlockPos::floored_v(target.max),
        )
        .all(|pos| visit(pos, 0));
    }
    let mut visited = FxHashSet::default();
    if !between(
        floor(target.min - travel),
        floor(target.max - travel),
        travel,
        |pos| {
            if !visit(pos, 0) {
                return false;
            }
            visited.insert(pos);
            true
        },
    ) {
        return false;
    }
    let delta = array(travel);
    let size = array(target.max - target.min);
    let center = array(target.min + (target.max - target.min) * 0.5);
    let signs: [i32; 3] = std::array::from_fn(|i| if delta[i] >= 0.0 { 1 } else { -1 });
    // BlockGetter.getFurthestCorner (including its axis/sign permutation).
    let corner_dir = if delta[0].abs() <= delta[1].abs() && delta[0].abs() <= delta[2].abs() {
        [-signs[0], -signs[2], signs[1]]
    } else if delta[1].abs() <= delta[2].abs() {
        [signs[2], -signs[1], -signs[0]]
    } else {
        [-signs[1], signs[0], -signs[2]]
    };
    let end_corner: [f64; 3] =
        std::array::from_fn(|i| center[i] + size[i] * 0.5 * f64::from(corner_dir[i]));
    let start_corner: [f64; 3] = std::array::from_fn(|i| end_corner[i] - delta[i]);
    let mut cell = start_corner.map(|v| v.floor() as i32);
    let sign: [i32; 3] = delta.map(|v| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    });
    let t_delta: [f64; 3] = std::array::from_fn(|i| {
        if sign[i] == 0 {
            f64::MAX
        } else {
            f64::from(sign[i]) / delta[i]
        }
    });
    let mut t: [f64; 3] = std::array::from_fn(|i| {
        let fraction = start_corner[i] - start_corner[i].floor();
        t_delta[i]
            * if sign[i] > 0 {
                1.0 - fraction
            } else {
                fraction
            }
    });
    let mut step = 0;
    while t.iter().any(|v| *v <= 1.0) {
        let axis = if t[0] < t[1] {
            if t[0] < t[2] { 0 } else { 2 }
        } else if t[1] < t[2] {
            1
        } else {
            2
        };
        cell[axis] += sign[axis];
        t[axis] += t_delta[axis];
        let bounds = BoundingBox::from_block(&BlockPos::new(cell[0], cell[1], cell[2]));
        if let Some(hit) = clip(bounds, vector(start_corner), vector(end_corner)) {
            step += 1;
            let hit = array(hit);
            let opposite: [i32; 3] = std::array::from_fn(|i| {
                (hit[i].clamp(f64::from(cell[i]) + SKIN, f64::from(cell[i]) + 1.0 - SKIN)
                    - size[i] * f64::from(corner_dir[i]))
                .floor() as i32
            });
            if !between(cell, opposite, travel, |pos| {
                !visited.insert(pos) || visit(pos, step)
            }) {
                return false;
            }
        }
    }
    between(floor(target.min), floor(target.max), travel, |pos| {
        !visited.insert(pos) || visit(pos, step + 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Case {
        from: [u64; 3],
        to: [u64; 3],
        min: [u64; 3],
        max: [u64; 3],
        hits: Vec<[i32; 4]>,
    }
    #[test]
    fn movement_history_bounds_packets_and_preserves_replay_paths() {
        let point = |x: usize| Vector3::new(x as f64, 0.0, 0.0);
        let mut history = MovementHistory::default();
        for i in 0..102 {
            history.push(Movement {
                from: point(i),
                to: point(i + 1),
                original: Some(point(1)),
            });
        }
        let paths = history.finish(point(0), point(102));
        assert_eq!(paths.len(), 100);
        assert_eq!(
            paths[0],
            Movement {
                from: point(0),
                to: point(3),
                original: None
            }
        );
        assert_eq!(paths[1].original, Some(point(1)));
        assert!(history.is_empty());
        assert_eq!(history.replay(), paths);
        history.push(Movement {
            from: point(102),
            to: point(103),
            original: None,
        });
        assert_eq!(
            history.replay(),
            paths,
            "pending movement must not alter replay"
        );
        let paths = history.finish(point(102), point(105));
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[1],
            Movement {
                from: point(103),
                to: point(105),
                original: None
            }
        );
        history.clear();
        assert!(history.replay().is_empty());
        assert!(history.is_empty());
        assert_eq!(
            history.finish(point(8), point(9)),
            vec![Movement {
                from: point(8),
                to: point(9),
                original: None
            }]
        );
    }

    #[test]
    fn exact_cell_order_and_steps_match_unmodified_java_26_2() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("inside_blocks_cases.json")).unwrap();
        for (index, case) in cases.into_iter().enumerate() {
            let mut hits = Vec::new();
            assert!(visit_swept(
                vector(case.from.map(f64::from_bits)),
                vector(case.to.map(f64::from_bits)),
                BoundingBox::new(
                    vector(case.min.map(f64::from_bits)),
                    vector(case.max.map(f64::from_bits))
                ),
                |pos, step| {
                    hits.push([pos.0.x, pos.0.y, pos.0.z, step]);
                    true
                }
            ));
            assert_eq!(hits, case.hits, "oracle case {index}");
        }
    }
    #[test]
    fn swept_shape_hits_thin_crossed_shapes_without_hitting_diagonal_near_misses() {
        let body = BoundingBox::new_array([0.0, 0.0, 0.0], [0.6, 1.8, 0.6]);
        assert!(collided_along(
            body,
            Vector3::new(4.0, 0.0, 0.0),
            BoundingBox::new_array([2.0, 0.0, 0.0], [2.1, 1.0, 1.0])
        ));
        assert!(!collided_along(
            body,
            Vector3::new(4.0, 0.0, 4.0),
            BoundingBox::new_array([0.0, 0.0, 3.0], [1.0, 1.0, 4.0])
        ));
    }
}
