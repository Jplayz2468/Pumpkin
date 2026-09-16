//! Java axis-ordered movement against a list of box collision shapes.
#[derive(Clone, Copy)]
pub struct Box3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
#[derive(Clone)]
pub struct VoxelShape {
    pub boxes: Vec<Box3>,
    pub coordinates: [Vec<f64>; 3],
}
impl VoxelShape {
    pub fn from_box(bounds: Box3) -> Self {
        let coordinates = box_coordinates(bounds);
        let boxes = if coordinates.iter().any(Vec::is_empty) {
            Vec::new()
        } else {
            vec![snap_box(bounds, &coordinates)]
        };
        Self { boxes, coordinates }
    }
    pub fn from_boxes(boxes: Vec<Box3>) -> Self {
        // Shapes.or(...).optimize() enumerates occupied boxes in Y/X/Z order,
        // merging Z strips first, then X, then Y. Rebuilding the optimized grid
        // from the original component boxes retains planes Java removes.
        let mut boxes = merged_boxes(boxes);
        let mut coordinates: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
        for bounds in &mut boxes {
            let grid = box_coordinates(*bounds);
            *bounds = snap_box(*bounds, &grid);
            for (axis, values) in grid.into_iter().enumerate() {
                coordinates[axis] = merge_coordinates(&coordinates[axis], &values);
            }
        }
        boxes = merged_boxes(
            boxes
                .into_iter()
                .map(|b| snap_box(b, &coordinates))
                .collect(),
        );
        if boxes.is_empty() {
            coordinates = std::array::from_fn(|_| vec![0.0]);
        }
        Self { boxes, coordinates }
    }
    /// Shapes.or joins the original grids before optimizing their occupied union.
    /// In particular the stationary piston base wins epsilon-close grid planes.
    pub fn union(self, other: Self) -> Self {
        if self.boxes.is_empty() {
            return Self::from_boxes(other.boxes);
        }
        if other.boxes.is_empty() {
            return Self::from_boxes(self.boxes);
        }
        let coordinates = std::array::from_fn(|axis| {
            merge_coordinates(&self.coordinates[axis], &other.coordinates[axis])
        });
        Self::from_boxes(
            self.boxes
                .into_iter()
                .chain(other.boxes)
                .map(|b| snap_box(b, &coordinates))
                .collect(),
        )
    }

    pub fn translated(mut self, delta: [f64; 3]) -> Self {
        for shape in &mut self.boxes {
            *shape = shape.translated(delta);
        }
        for axis in 0..3 {
            for value in &mut self.coordinates[axis] {
                *value += delta[axis];
            }
        }
        self
    }
    fn clip(&self, axis: usize, bounds: Box3, distance: f64) -> f64 {
        if self.boxes.is_empty() {
            return distance;
        }
        if distance.abs() < 1.0e-7 {
            return 0.0;
        }
        let coords = &self.coordinates[axis];
        let positive = distance > 0.0;
        let edge = if positive {
            bounds.max[axis] - 1.0e-7
        } else {
            bounds.min[axis] + 1.0e-7
        };
        let next = coords.partition_point(|&coordinate| coordinate <= edge);
        let mut contact: Option<f64> = None;
        for shape in &self.boxes {
            if (0..3).any(|i| {
                i != axis
                    && (bounds.min[i] + 1.0e-7 >= shape.max[i]
                        || bounds.max[i] - 1.0e-7 < shape.min[i])
            }) {
                continue;
            }
            let coordinate = if positive {
                let first = coords.partition_point(|&value| value < shape.min[axis]);
                let index = first.max(next);
                coords
                    .get(index)
                    .copied()
                    .filter(|value| *value < shape.max[axis])
            } else {
                let last = coords.partition_point(|&value| value <= shape.max[axis]);
                last.min(next)
                    .checked_sub(1)
                    .and_then(|index| coords.get(index).copied())
                    .filter(|value| *value > shape.min[axis])
            };
            if let Some(value) = coordinate {
                contact = Some(contact.map_or(value, |old| {
                    if positive {
                        old.min(value)
                    } else {
                        old.max(value)
                    }
                }));
            }
        }
        if let Some(coordinate) = contact {
            let gap = coordinate
                - if positive {
                    bounds.max[axis]
                } else {
                    bounds.min[axis]
                };
            if positive && gap >= -1.0e-7 {
                return distance.min(gap);
            }
            if !positive && gap <= 1.0e-7 {
                return distance.max(gap);
            }
        }
        distance
    }
}

fn snap_box(mut bounds: Box3, coordinates: &[Vec<f64>; 3]) -> Box3 {
    for axis in 0..3 {
        for edge in [&mut bounds.min[axis], &mut bounds.max[axis]] {
            if let Some(&value) = coordinates[axis]
                .iter()
                .find(|&&value| value == *edge || (value - *edge).abs() < 1.0e-7)
            {
                *edge = value;
            }
        }
    }
    bounds
}

/// OR's IndirectMerger keeps the first list's coordinate when planes are within
/// Java's shape epsilon; sorting and deduplicating changes that choice.
fn merge_coordinates(first: &[f64], second: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(first.len() + second.len());
    let (mut a, mut b) = (0, 0);
    while a < first.len() || b < second.len() {
        let value = if a < first.len() && (b == second.len() || first[a] < second[b] + 1.0e-7) {
            a += 1;
            first[a - 1]
        } else {
            b += 1;
            second[b - 1]
        };
        if result.last().is_none_or(|last| !(*last >= value - 1.0e-7)) {
            result.push(value);
        }
    }
    result
}

fn merged_boxes(mut boxes: Vec<Box3>) -> Vec<Box3> {
    boxes.retain(|b| (0..3).all(|axis| b.max[axis] - b.min[axis] >= 1.0e-7));
    if boxes.len() <= 1 {
        return boxes;
    }
    let coordinates: [Vec<f64>; 3] = std::array::from_fn(|axis| {
        let mut values: Vec<_> = boxes
            .iter()
            .flat_map(|b| [b.min[axis], b.max[axis]])
            .collect();
        values.sort_unstable_by(f64::total_cmp);
        values.dedup();
        values
    });
    let [nx, ny, nz] = std::array::from_fn(|axis| coordinates[axis].len() - 1);
    let index = |x, y, z| (x * ny + y) * nz + z;
    let mut full = vec![false; nx * ny * nz];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let cell = [x, y, z];
                full[index(x, y, z)] = boxes.iter().any(|b| {
                    (0..3).all(|axis| {
                        coordinates[axis][cell[axis]] >= b.min[axis]
                            && coordinates[axis][cell[axis] + 1] <= b.max[axis]
                    })
                });
            }
        }
    }
    let mut result = Vec::new();
    for y in 0..ny {
        for x in 0..nx {
            let mut z = 0;
            while z < nz {
                if !full[index(x, y, z)] {
                    z += 1;
                    continue;
                }
                let start = z;
                while z < nz && full[index(x, y, z)] {
                    z += 1;
                }
                for cz in start..z {
                    full[index(x, y, cz)] = false;
                }
                let mut end_x = x + 1;
                while end_x < nx && (start..z).all(|cz| full[index(end_x, y, cz)]) {
                    for cz in start..z {
                        full[index(end_x, y, cz)] = false;
                    }
                    end_x += 1;
                }
                let mut end_y = y + 1;
                while end_y < ny
                    && (x..end_x).all(|cx| (start..z).all(|cz| full[index(cx, end_y, cz)]))
                {
                    for cx in x..end_x {
                        for cz in start..z {
                            full[index(cx, end_y, cz)] = false;
                        }
                    }
                    end_y += 1;
                }
                result.push(Box3 {
                    min: [coordinates[0][x], coordinates[1][y], coordinates[2][start]],
                    max: [
                        coordinates[0][end_x],
                        coordinates[1][end_y],
                        coordinates[2][z],
                    ],
                });
            }
        }
    }
    result
}

pub fn collide(motion: [f64; 3], bounds: Box3, boxes: &[Box3]) -> ([f64; 3], Option<usize>) {
    let shapes: Vec<_> = boxes.iter().copied().map(VoxelShape::from_box).collect();
    collide_voxels(motion, bounds, &shapes)
}

pub fn collide_voxels(
    motion: [f64; 3],
    bounds: Box3,
    shapes: &[VoxelShape],
) -> ([f64; 3], Option<usize>) {
    if shapes.is_empty() {
        return (motion, None);
    }
    let order = if motion[0].abs() < motion[2].abs() {
        [1, 2, 0]
    } else {
        [1, 0, 2]
    };
    let mut result = [0.0; 3];
    let mut support = None;
    for axis in order {
        if motion[axis] == 0.0 {
            continue;
        }
        let moved = bounds.translated(result);
        let mut movement = motion[axis];
        for (index, shape) in shapes.iter().enumerate() {
            if movement.abs() < 1.0e-7 {
                movement = 0.0;
                break;
            }
            let before = movement;
            movement = shape.clip(axis, moved, movement);
            if axis == 1 && motion[1] < 0.0 && before != movement {
                support = Some(index);
            }
        }
        result[axis] = movement;
    }
    (result, support)
}

impl Box3 {
    pub fn translated(self, delta: [f64; 3]) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i] + delta[i]),
            max: std::array::from_fn(|i| self.max[i] + delta[i]),
        }
    }
}

/// Search area for Entity.collide's second collision query, if stepping is allowed.
pub fn step_query(
    motion: [f64; 3],
    clipped: [f64; 3],
    bounds: Box3,
    grounded: bool,
    max_step: f32,
) -> Option<(Box3, Box3)> {
    let landed = motion[1] != clipped[1] && motion[1] < 0.0;
    if max_step <= 0.0
        || !(landed || grounded)
        || (motion[0] == clipped[0] && motion[2] == clipped[2])
    {
        return None;
    }
    let feet = if landed {
        bounds.translated([0.0, clipped[1], 0.0])
    } else {
        bounds
    };
    let stretch = [motion[0], f64::from(max_step), motion[2]];
    let mut query = Box3 {
        min: std::array::from_fn(|i| feet.min[i] + stretch[i].min(0.0)),
        max: std::array::from_fn(|i| feet.max[i] + stretch[i].max(0.0)),
    };
    if !landed {
        query.min[1] -= f64::from(1.0e-5_f32);
    }
    Some((feet, query))
}

/// Y coordinates of Shapes.create(AABB), including a CubeVoxelShape's internal grid.
pub fn box_coordinates(shape: Box3) -> [Vec<f64>; 3] {
    if (0..3).any(|axis| shape.max[axis] - shape.min[axis] < 1.0e-7) {
        return std::array::from_fn(|_| Vec::new());
    }
    let intervals = |min: f64, max: f64| {
        if min < -1.0e-7 || max > 1.0000001 {
            return None;
        }
        (0..=3).map(|bits| 1 << bits).find(|&n| {
            let n = f64::from(n);
            let a = min * n;
            let b = max * n;
            (a - (a + 0.5).floor()).abs() < 1.0e-7 * n && (b - (b + 0.5).floor()).abs() < 1.0e-7 * n
        })
    };
    let grids: [Option<i32>; 3] =
        std::array::from_fn(|axis| intervals(shape.min[axis], shape.max[axis]));
    if grids.iter().all(Option::is_some) {
        std::array::from_fn(|axis| {
            let n = grids[axis].unwrap();
            (0..=n).map(|i| f64::from(i) / f64::from(n)).collect()
        })
    } else {
        std::array::from_fn(|axis| vec![shape.min[axis], shape.max[axis]])
    }
}

pub fn step_candidates(bounds: Box3, coordinates: &[f64], max_step: f32, skip: f32) -> Vec<f32> {
    let mut candidates = Vec::new();
    for &y in coordinates {
        let height = (y - bounds.min[1]) as f32;
        if height >= 0.0 && height != skip && height <= max_step && !candidates.contains(&height) {
            candidates.push(height);
        }
    }
    candidates.sort_unstable_by(f32::total_cmp);
    candidates
}

pub fn step_up(
    motion: [f64; 3],
    clipped: [f64; 3],
    bounds: Box3,
    feet: Box3,
    shapes: &[VoxelShape],
    coordinates: &[f64],
    max_step: f32,
) -> [f64; 3] {
    let original_horizontal = clipped[0] * clipped[0] + clipped[2] * clipped[2];
    for height in step_candidates(feet, coordinates, max_step, clipped[1] as f32) {
        let (mut step, _) = collide_voxels([motion[0], f64::from(height), motion[2]], feet, shapes);
        if step[0] * step[0] + step[2] * step[2] > original_horizontal {
            step[1] -= bounds.min[1] - feet.min[1];
            return step;
        }
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_static_block_shape_matches_java_clipping() {
        let expected: Vec<u64> =
            serde_json::from_str(include_str!("static_collision_hashes.json")).unwrap();
        assert_eq!(
            expected.len(),
            usize::from(pumpkin_data::BlockStateId::COUNT)
        );
        for (id, expected) in expected.into_iter().enumerate() {
            let state = pumpkin_data::BlockStateId::new(id as u16)
                .unwrap()
                .to_state();
            let boxes: Vec<_> = state
                .get_block_collision_shapes()
                .map(|b| Box3 {
                    min: [b.min.x, b.min.y, b.min.z],
                    max: [b.max.x, b.max.y, b.max.z],
                })
                .collect();
            let shapes = if boxes.is_empty() {
                Vec::new()
            } else {
                vec![VoxelShape {
                    boxes,
                    coordinates: state.collision_coordinates().map(<[f64]>::to_vec),
                }]
            };
            let mut hash = 0xcbf29ce484222325_u64;
            for pose in 0..3 {
                let p = f64::from(pose) * 0.5;
                let y = f64::from(pose) * 0.25;
                let bounds = Box3 {
                    min: [p - 0.3, y, p - 0.3],
                    max: [p + 0.3, y + 1.8, p + 0.3],
                };
                for x in -1..=1 {
                    for v in -1..=1 {
                        for z in -1..=1 {
                            let (result, _) = collide_voxels(
                                [
                                    f64::from(x) * 1.125,
                                    f64::from(v) * 0.875,
                                    f64::from(z) * 1.125,
                                ],
                                bounds,
                                &shapes,
                            );
                            for value in result {
                                hash = (hash ^ if value == 0.0 { 0 } else { value.to_bits() })
                                    .wrapping_mul(0x100000001b3);
                            }
                        }
                    }
                }
            }
            assert_eq!(hash, expected, "Java static collision state {id}");
        }
    }

    #[test]
    fn java_shape_clipping_and_step_candidates() {
        #[derive(serde::Deserialize)]
        struct Case {
            bounds: [[u64; 3]; 2],
            motion: [u64; 3],
            grounded: bool,
            max: f32,
            shapes: Vec<[[u64; 3]; 2]>,
            initial: [u64; 3],
            candidates: Vec<u32>,
            result: [u64; 3],
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("step_collision_cases.json")).unwrap();
        let box_from = |b: [[u64; 3]; 2]| Box3 {
            min: b[0].map(f64::from_bits),
            max: b[1].map(f64::from_bits),
        };
        for (index, case) in cases.into_iter().enumerate() {
            let bounds = box_from(case.bounds);
            let motion = case.motion.map(f64::from_bits);
            let shapes: Vec<_> = case.shapes.into_iter().map(box_from).collect();
            let coordinates: Vec<_> = shapes
                .iter()
                .copied()
                .flat_map(|shape| box_coordinates(shape)[1].clone())
                .collect();
            let (mut result, _) = collide(motion, bounds, &shapes);
            assert_eq!(
                result,
                case.initial.map(f64::from_bits),
                "Java clip case {index}"
            );
            if let Some((feet, _)) = step_query(motion, result, bounds, case.grounded, case.max) {
                assert_eq!(
                    step_candidates(feet, &coordinates, case.max, result[1] as f32),
                    case.candidates
                        .into_iter()
                        .map(f32::from_bits)
                        .collect::<Vec<_>>(),
                    "Java candidate case {index}"
                );
                let voxels: Vec<_> = shapes.iter().copied().map(VoxelShape::from_box).collect();
                result = step_up(
                    motion,
                    result,
                    bounds,
                    feet,
                    &voxels,
                    &coordinates,
                    case.max,
                );
            } else {
                assert!(case.candidates.is_empty());
            }
            assert_eq!(
                result,
                case.result.map(f64::from_bits),
                "Java step case {index}"
            );
        }
    }
}
