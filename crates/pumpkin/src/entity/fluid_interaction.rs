//! Java EntityFluidInteraction scan and passenger-box geometry.
use pumpkin_util::math::{
    boundingbox::BoundingBox, position::BlockPos, vector2::Vector2, vector3::Vector3,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct Tracker {
    pub height: f64,
    pub eyes_inside: bool,
    pub current: Vector3<f64>,
    pub current_count: usize,
}

pub struct Sample<T> {
    pub kind: usize,
    pub height: f64,
    pub state: T,
}

pub fn interaction_box(bounds: BoundingBox) -> BoundingBox {
    // Java AABB normalizes crossed endpoints, including zero-sized entities.
    let a = bounds.min.add_raw(0.001, 0.001, 0.001);
    let b = bounds.max.add_raw(-0.001, -0.001, -0.001);
    BoundingBox::new(
        Vector3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
        Vector3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
    )
}

pub fn boat_passenger_box(
    boat: BoundingBox,
    passenger: BoundingBox,
    underwater: bool,
) -> Option<BoundingBox> {
    if underwater {
        Some(passenger)
    } else if boat.max.y >= passenger.max.y {
        None
    } else {
        Some(BoundingBox::new(
            Vector3::new(
                passenger.min.x,
                passenger.min.y.max(boat.max.y),
                passenger.min.z,
            ),
            passenger.max,
        ))
    }
}

pub fn scan<T>(
    bounds: Option<BoundingBox>,
    body_min_y: f64,
    eye: Vector3<f64>,
    ignore_current: bool,
    mut loaded: impl FnMut(Vector2<i32>) -> bool,
    mut sample: impl FnMut(BlockPos) -> Option<Sample<T>>,
    mut flow: impl FnMut(BlockPos, &T) -> Vector3<f64>,
) -> [Tracker; 2] {
    let mut result = [Tracker::default(); 2];
    let Some(bounds) = bounds else {
        return result;
    };
    let min = bounds.min_block_pos();
    let max = BlockPos::new(
        (bounds.max.x.ceil() as i32).saturating_sub(1),
        (bounds.max.y.ceil() as i32).saturating_sub(1),
        (bounds.max.z.ceil() as i32).saturating_sub(1),
    );
    for z in (min.0.z.wrapping_sub(1) >> 4)..=(max.0.z.wrapping_add(1) >> 4) {
        for x in (min.0.x.wrapping_sub(1) >> 4)..=(max.0.x.wrapping_add(1) >> 4) {
            if !loaded(Vector2::new(x, z)) {
                return result;
            }
        }
    }
    let eye_x = eye.x.floor() as i32;
    let eye_z = eye.z.floor() as i32;
    for x in min.0.x..=max.0.x {
        for y in min.0.y..=max.0.y {
            for z in min.0.z..=max.0.z {
                let pos = BlockPos::new(x, y, z);
                let Some(fluid) = sample(pos) else {
                    continue;
                };
                let top = f64::from(y) + fluid.height;
                if top < bounds.min.y {
                    continue;
                }
                let tracker = &mut result[fluid.kind];
                if x == eye_x && z == eye_z && eye.y >= f64::from(y) && eye.y <= top {
                    tracker.eyes_inside = true;
                }
                tracker.height = tracker.height.max(top - body_min_y);
                if !ignore_current {
                    let current = flow(pos, &fluid.state);
                    tracker.current += if tracker.height < 0.4 {
                        current * tracker.height
                    } else {
                        current
                    };
                    tracker.current_count += 1;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_fluid_scan_and_boat_passenger_boxes() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fluid_interaction_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let number = |v: &serde_json::Value| f64::from_bits(v.as_u64().unwrap());
            let vector =
                |v: &serde_json::Value| Vector3::new(number(&v[0]), number(&v[1]), number(&v[2]));
            let bounds = |v: &serde_json::Value| {
                BoundingBox::new(
                    vector(v),
                    Vector3::new(number(&v[3]), number(&v[4]), number(&v[5])),
                )
            };
            let original = bounds(&case[0]);
            let mut shape = Some(interaction_box(original));
            if case[2].as_bool().unwrap() {
                shape = boat_passenger_box(
                    bounds(&case[4]),
                    shape.unwrap(),
                    case[3].as_bool().unwrap(),
                );
            }
            assert_eq!(
                shape.map(|b| (b.min, b.max)),
                (!case[5].is_null()).then(|| {
                    let b = bounds(&case[5]);
                    (b.min, b.max)
                }),
                "box case {index}"
            );
            let mut samples = std::collections::HashMap::new();
            for cell in case[8].as_array().unwrap() {
                let p = &cell[0];
                samples.insert(
                    BlockPos::new(
                        p[0].as_i64().unwrap() as i32,
                        p[1].as_i64().unwrap() as i32,
                        p[2].as_i64().unwrap() as i32,
                    ),
                    (
                        cell[1].as_u64().unwrap() as usize,
                        number(&cell[2][0]),
                        vector(&cell[3]),
                    ),
                );
            }
            let mut chunks = Vec::new();
            let actual = scan(
                shape,
                original.min.y,
                vector(&case[1]),
                case[6].as_bool().unwrap(),
                |pos| {
                    chunks.push([pos.x, pos.y]);
                    !(case[7].as_bool().unwrap() && pos.x == -1)
                },
                |pos| {
                    samples.get(&pos).map(|&(kind, height, state)| Sample {
                        kind,
                        height,
                        state,
                    })
                },
                |_, flow| *flow,
            );
            assert_eq!(
                chunks,
                serde_json::from_value::<Vec<[i32; 2]>>(case[9].clone()).unwrap(),
                "chunks case {index}"
            );
            for (kind, tracker) in actual.iter().enumerate() {
                let expected = &case[10][kind];
                assert_eq!(
                    tracker.height.to_bits(),
                    expected[0][0].as_u64().unwrap(),
                    "height case {index}/{kind}"
                );
                assert_eq!(
                    tracker.eyes_inside,
                    expected[1].as_bool().unwrap(),
                    "eyes case {index}/{kind}"
                );
                let current = vector(&expected[2]);
                assert_eq!(
                    [
                        tracker.current.x.to_bits(),
                        tracker.current.y.to_bits(),
                        tracker.current.z.to_bits()
                    ],
                    [
                        current.x.to_bits(),
                        current.y.to_bits(),
                        current.z.to_bits()
                    ],
                    "current case {index}/{kind}"
                );
                assert_eq!(
                    tracker.current_count,
                    expected[3].as_u64().unwrap() as usize,
                    "count case {index}/{kind}"
                );
            }
        }
    }
}
