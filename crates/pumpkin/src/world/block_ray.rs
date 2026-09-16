//! BlockGetter.traverseBlocks and the boolean part of VoxelShape.clip.
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

pub fn traverse<T>(
    from: Vector3<f64>,
    to: Vector3<f64>,
    mut visit: impl FnMut(BlockPos) -> Option<T>,
) -> Option<T> {
    if from == to {
        return None;
    }
    let original_from = [from.x, from.y, from.z];
    let original_to = [to.x, to.y, to.z];
    let from: [f64; 3] =
        std::array::from_fn(|i| original_from[i] + -1.0e-7 * (original_to[i] - original_from[i]));
    let to: [f64; 3] =
        std::array::from_fn(|i| original_to[i] + -1.0e-7 * (original_from[i] - original_to[i]));
    let mut cell = from.map(|v| v.floor() as i32);
    if let Some(hit) = visit(BlockPos::new(cell[0], cell[1], cell[2])) {
        return Some(hit);
    }
    let direction: [f64; 3] = std::array::from_fn(|i| to[i] - from[i]);
    let sign = direction.map(|v| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    });
    let step: [f64; 3] = std::array::from_fn(|i| {
        if sign[i] == 0 {
            f64::MAX
        } else {
            f64::from(sign[i]) / direction[i]
        }
    });
    let mut next: [f64; 3] = std::array::from_fn(|i| {
        let fraction = from[i] - from[i].floor();
        step[i]
            * if sign[i] > 0 {
                1.0 - fraction
            } else {
                fraction
            }
    });
    while next.iter().any(|&v| v <= 1.0) {
        let axis = if next[0] < next[1] && next[0] < next[2] {
            0
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        cell[axis] = cell[axis].wrapping_add(sign[axis]);
        next[axis] += step[axis];
        if let Some(hit) = visit(BlockPos::new(cell[0], cell[1], cell[2])) {
            return Some(hit);
        }
    }
    None
}

pub fn intersects_box(from: Vector3<f64>, to: Vector3<f64>, bounds: BoundingBox) -> bool {
    let diff = to - from;
    if diff.length_squared() < 1.0e-7 {
        return false;
    }
    let test = from + diff * 0.001;
    if test.x >= bounds.min.x
        && test.x < bounds.max.x
        && test.y >= bounds.min.y
        && test.y < bounds.max.y
        && test.z >= bounds.min.z
        && test.z < bounds.max.z
    {
        return true;
    }
    let from = [from.x, from.y, from.z];
    let diff = [diff.x, diff.y, diff.z];
    let min = [bounds.min.x, bounds.min.y, bounds.min.z];
    let max = [bounds.max.x, bounds.max.y, bounds.max.z];
    for axis in 0..3 {
        if diff[axis].abs() <= 1.0e-7 {
            continue;
        }
        let edge = if diff[axis] > 0.0 {
            min[axis]
        } else {
            max[axis]
        };
        let t = (edge - from[axis]) / diff[axis];
        if t > 0.0
            && t < 1.0
            && (0..3).filter(|&i| i != axis).all(|i| {
                let value = from[i] + t * diff[i];
                value > min[i] - 1.0e-7 && value < max[i] + 1.0e-7
            })
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_traversal_and_water_height_clip_cases() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("block_ray_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let vector = |i: usize| {
                Vector3::new(
                    f64::from_bits(case[i][0].as_u64().unwrap()),
                    f64::from_bits(case[i][1].as_u64().unwrap()),
                    f64::from_bits(case[i][2].as_u64().unwrap()),
                )
            };
            let from = vector(0);
            let to = vector(1);
            let height = f64::from_bits(case[2][0].as_u64().unwrap());
            let mut positions = Vec::new();
            traverse::<()>(from, to, |pos| {
                positions.push([pos.0.x, pos.0.y, pos.0.z]);
                None
            });
            let expected: Vec<[i32; 3]> = serde_json::from_value(case[3].clone()).unwrap();
            assert_eq!(positions, expected, "case {index}");
            assert_eq!(
                intersects_box(
                    from,
                    to,
                    BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, height, 1.0])
                ),
                case[4].as_bool().unwrap(),
                "clip case {index}"
            );
        }
    }
}
