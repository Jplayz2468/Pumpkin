//! Java BlockGetter traversal and VoxelShape clipping.
use pumpkin_data::BlockDirection;
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

/// Fluid selection is independent of the block shape. A solid in front of water
/// still stops an item ray, and a fluid can win over a later block hit in its cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayFluidHandling {
    None,
    Source,
    Any,
    Water,
}

impl RayFluidHandling {
    pub(crate) fn can_pick(
        self,
        fluid: &pumpkin_data::fluid::Fluid,
        state: &pumpkin_data::fluid::FluidState,
    ) -> bool {
        !state.is_empty
            && match self {
                Self::None => false,
                Self::Source => state.is_source,
                Self::Any => true,
                Self::Water => fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER),
            }
    }
}

/// FlowingFluid stores shapes by FluidState identity. In Java 26.2 the ordinary
/// states have amounts 1..8; their first queried world height is retained. The
/// explicit amount-9/same-above branch bypasses the cache in the original method.
#[derive(Default)]
pub(crate) struct FluidShapeCache {
    heights: std::collections::HashMap<(u16, bool, bool, i16), f32>,
}

impl FluidShapeCache {
    pub fn height(
        &mut self,
        fluid: &pumpkin_data::fluid::Fluid,
        state: &pumpkin_data::fluid::FluidState,
        height: f32,
    ) -> f32 {
        use pumpkin_data::fluid::Fluid;
        if state.is_empty {
            return 0.0;
        }
        if state.level == 9 && height == 1.0 {
            return 1.0;
        }
        let family = if fluid.matches_type(&Fluid::WATER) {
            Fluid::WATER.id
        } else {
            Fluid::LAVA.id
        };
        let key = (
            family,
            state.is_source,
            state.falling,
            if state.is_source { 8 } else { state.level },
        );
        *self.heights.entry(key).or_insert(height)
    }
}

static FLUID_SHAPES: std::sync::LazyLock<std::sync::Mutex<FluidShapeCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(FluidShapeCache::default()));

pub(crate) fn fluid_shape_height(
    fluid: &pumpkin_data::fluid::Fluid,
    state: &pumpkin_data::fluid::FluidState,
    height: f32,
) -> f32 {
    FLUID_SHAPES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .height(fluid, state, height)
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub direction: BlockDirection,
    pub position: Vector3<f64>,
    pub inside: bool,
}

/// Clip a complete voxel shape. All component boxes share the closest fraction;
/// the short inside probe and entering-face tie order match VoxelShape/AABB.clip.
pub(crate) fn clip(from: Vector3<f64>, to: Vector3<f64>, boxes: &[BoundingBox]) -> Option<RayHit> {
    let diff = to - from;
    if boxes.is_empty() || diff.length_squared() < 1.0e-7 {
        return None;
    }
    let test = from + diff * 0.001;
    if boxes.iter().any(|bounds| {
        test.x >= bounds.min.x
            && test.x < bounds.max.x
            && test.y >= bounds.min.y
            && test.y < bounds.max.y
            && test.z >= bounds.min.z
            && test.z < bounds.max.z
    }) {
        return Some(RayHit {
            direction: approximate_nearest(diff).opposite(),
            position: test,
            inside: true,
        });
    }
    clip_surfaces(from, to, boxes).map(|(_, hit)| hit)
}

/// AABB.clip tests entering surfaces only: starting inside or ending exactly on
/// the surface does not manufacture a hit, unlike VoxelShape's inside probe.
pub(crate) fn clip_aabb(
    from: Vector3<f64>,
    to: Vector3<f64>,
    bounds: BoundingBox,
) -> Option<(f64, RayHit)> {
    clip_surfaces(from, to, &[bounds])
}

fn clip_surfaces(
    from: Vector3<f64>,
    to: Vector3<f64>,
    boxes: &[BoundingBox],
) -> Option<(f64, RayHit)> {
    let diff = to - from;
    let origin = [from.x, from.y, from.z];
    let delta = [diff.x, diff.y, diff.z];
    let mut closest = 1.0;
    let mut direction = None;
    for bounds in boxes {
        let min = [bounds.min.x, bounds.min.y, bounds.min.z];
        let max = [bounds.max.x, bounds.max.y, bounds.max.z];
        for axis in 0..3 {
            if delta[axis].abs() <= 1.0e-7 {
                continue;
            }
            let positive = delta[axis] > 0.0;
            let edge = if positive { min[axis] } else { max[axis] };
            let t = (edge - origin[axis]) / delta[axis];
            if t > 0.0
                && t < closest
                && (0..3).filter(|&i| i != axis).all(|i| {
                    let value = origin[i] + t * delta[i];
                    value > min[i] - 1.0e-7 && value < max[i] + 1.0e-7
                })
            {
                closest = t;
                direction = Some(match (axis, positive) {
                    (0, true) => BlockDirection::West,
                    (0, false) => BlockDirection::East,
                    (1, true) => BlockDirection::Down,
                    (1, false) => BlockDirection::Up,
                    (2, true) => BlockDirection::North,
                    _ => BlockDirection::South,
                });
            }
        }
    }
    direction.map(|direction| {
        (
            closest,
            RayHit {
                direction,
                position: from + diff * closest,
                inside: false,
            },
        )
    })
}

pub(crate) fn approximate_nearest(diff: Vector3<f64>) -> BlockDirection {
    // Direction.getApproximateNearest narrows to floats before its dot products.
    let [dx, dy, dz] = [diff.x as f32, diff.y as f32, diff.z as f32];
    let mut nearest = BlockDirection::North;
    let mut highest = f32::from_bits(1);
    for (direction, normal) in [
        (BlockDirection::Down, [0.0, -1.0, 0.0]),
        (BlockDirection::Up, [0.0, 1.0, 0.0]),
        (BlockDirection::North, [0.0, 0.0, -1.0]),
        (BlockDirection::South, [0.0, 0.0, 1.0]),
        (BlockDirection::West, [-1.0, 0.0, 0.0]),
        (BlockDirection::East, [1.0, 0.0, 0.0]),
    ] {
        let dot = dx * normal[0] + dy * normal[1] + dz * normal[2];
        if dot > highest {
            highest = dot;
            nearest = direction;
        }
    }
    nearest
}

/// CollisionGetter.clipIncludingBorder clamps the selected hit endpoint; this
/// deliberately is not the geometric intersection of the ray and border plane.
pub(crate) fn clip_border(
    from: Vector3<f64>,
    to: Vector3<f64>,
    bounds: [f64; 4],
) -> Option<RayHit> {
    let [min_x, min_z, max_x, max_z] = bounds;
    let contains = |p: Vector3<f64>| p.x >= min_x && p.x < max_x && p.z >= min_z && p.z < max_z;
    if !contains(from) || contains(to) {
        return None;
    }
    let epsilon = f64::from(1.0e-5_f32);
    // Mth.clamp's branch order matters for borders narrower than epsilon.
    let clamp = |value: f64, min: f64, max: f64| if value < min { min } else { value.min(max) };
    Some(RayHit {
        direction: approximate_nearest(to - from),
        position: Vector3::new(
            clamp(to.x, min_x, max_x - epsilon),
            to.y,
            clamp(to.z, min_z, max_z - epsilon),
        ),
        inside: false,
    })
}

/// Interaction volumes only override the face of an existing block hit.
pub(crate) fn with_interaction_override(
    from: Vector3<f64>,
    mut block: Option<RayHit>,
    interaction: Option<RayHit>,
) -> Option<RayHit> {
    if let (Some(hit), Some(override_hit)) = (block.as_mut(), interaction)
        && (override_hit.position - from).length_squared() < (hit.position - from).length_squared()
    {
        hit.direction = override_hit.direction;
    }
    block
}

pub(crate) fn nearest(
    from: Vector3<f64>,
    block: Option<RayHit>,
    fluid: Option<RayHit>,
) -> Option<RayHit> {
    match (block, fluid) {
        (Some(block), Some(fluid)) => Some(
            if (block.position - from).length_squared() <= (fluid.position - from).length_squared()
            {
                block
            } else {
                fluid
            },
        ),
        (block, fluid) => block.or(fluid),
    }
}

pub fn intersects_box(from: Vector3<f64>, to: Vector3<f64>, bounds: BoundingBox) -> bool {
    clip(from, to, &[bounds]).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_border_ray_clamping() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("border_ray_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let value =
                |json: &serde_json::Value, n: usize| f64::from_bits(json[n].as_u64().unwrap());
            let vector = |json: &serde_json::Value| {
                Vector3::new(value(json, 0), value(json, 1), value(json, 2))
            };
            let bounds = std::array::from_fn(|n| value(&case[0], n));
            let hit = clip_border(vector(&case[1]), vector(&case[2]), bounds);
            assert_eq!(hit.is_some(), !case[3].is_null(), "presence {index}");
            if let Some(hit) = hit {
                assert_eq!(
                    hit.direction as u64,
                    case[3][0].as_u64().unwrap(),
                    "face {index}"
                );
                assert_eq!(hit.inside, case[3][1].as_bool().unwrap(), "inside {index}");
                let expected = vector(&case[3][2]);
                assert_eq!(
                    [
                        hit.position.x.to_bits(),
                        hit.position.y.to_bits(),
                        hit.position.z.to_bits()
                    ],
                    [
                        expected.x.to_bits(),
                        expected.y.to_bits(),
                        expected.z.to_bits()
                    ],
                    "position {index}"
                );
            }
        }
    }

    #[test]
    fn java_block_and_fluid_selection() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("block_fluid_clip_cases.json")).unwrap();
        let mut fluid_shapes = FluidShapeCache::default();
        for (index, case) in cases.iter().enumerate() {
            let vector = |json: &serde_json::Value| {
                Vector3::new(
                    f64::from_bits(json[0].as_u64().unwrap()),
                    f64::from_bits(json[1].as_u64().unwrap()),
                    f64::from_bits(json[2].as_u64().unwrap()),
                )
            };
            let state = pumpkin_data::BlockStateId::new(case[0].as_u64().unwrap() as u16).unwrap();
            let from = vector(&case[3]);
            let to = vector(&case[4]);
            let mode = [
                RayFluidHandling::None,
                RayFluidHandling::Source,
                RayFluidHandling::Any,
                RayFluidHandling::Water,
            ][case[2].as_u64().unwrap() as usize];
            let hit = traverse(from, to, |pos| {
                if pos != BlockPos::new(0, 0, 0) {
                    return None;
                }
                let boxes: Vec<_> = state.to_state().get_block_outline_shapes_at(&pos).collect();
                let interaction: Vec<_> =
                    super::super::block_interaction_shapes::boxes(state.as_u16())
                        .iter()
                        .map(|b| BoundingBox::new_array([b[0], b[1], b[2]], [b[3], b[4], b[5]]))
                        .collect();
                let block = with_interaction_override(
                    from,
                    clip(from, to, &boxes),
                    clip(from, to, &interaction),
                );
                let (fluid, fluid_state) = super::super::World::fluid_state_from_block_state(state);
                // Start with the scene's uncached height, then preserve Java's per-state
                // shape cache across the complete sequence of queries.
                let height = f64::from_bits(case[5][0].as_u64().unwrap());
                let fluid = if mode.can_pick(fluid, &fluid_state) {
                    let height = f64::from(fluid_shapes.height(fluid, &fluid_state, height as f32));
                    clip(
                        from,
                        to,
                        &[BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, height, 1.0])],
                    )
                } else {
                    None
                };
                nearest(from, block, fluid)
            });
            assert_eq!(hit.is_some(), !case[6].is_null(), "case {index}");
            if let Some(hit) = hit {
                assert_eq!(
                    hit.direction as u64,
                    case[6][0].as_u64().unwrap(),
                    "face case {index}"
                );
                assert_eq!(
                    hit.inside,
                    case[6][1].as_bool().unwrap(),
                    "inside case {index}"
                );
                let expected = vector(&case[6][2]);
                assert_eq!(
                    [
                        hit.position.x.to_bits(),
                        hit.position.y.to_bits(),
                        hit.position.z.to_bits()
                    ],
                    [
                        expected.x.to_bits(),
                        expected.y.to_bits(),
                        expected.z.to_bits()
                    ],
                    "point case {index}"
                );
            }
        }
    }

    #[test]
    fn java_block_clip_and_interaction_faces() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("block_clip_cases.json")).unwrap();
        let mut override_count = 0;
        for (index, case) in cases.iter().enumerate() {
            let values = |json: &serde_json::Value| -> Vec<f64> {
                json.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| f64::from_bits(v.as_u64().unwrap()))
                    .collect()
            };
            let vector = |json: &serde_json::Value| {
                let v = values(json);
                Vector3::new(v[0], v[1], v[2])
            };
            let state = pumpkin_data::BlockStateId::new(case[0].as_u64().unwrap() as u16).unwrap();
            let pos = BlockPos::new(
                case[1][0].as_i64().unwrap() as i32,
                case[1][1].as_i64().unwrap() as i32,
                case[1][2].as_i64().unwrap() as i32,
            );
            let from = vector(&case[2]);
            let to = vector(&case[3]);
            // Use production block outlines, not fixture geometry, so data/order/offset
            // differences also fail. The fixture retains the original Java boxes for diagnosis.
            let boxes: Vec<_> = state
                .to_state()
                .get_block_outline_shapes_at(&pos)
                .map(|b| b.shift(pos.0.to_f64()))
                .collect();
            let check = |actual: Option<RayHit>, expected: &serde_json::Value| {
                assert_eq!(
                    actual.is_some(),
                    !expected.is_null(),
                    "case {index} state {}",
                    state.as_u16()
                );
                if let Some(hit) = actual {
                    assert_eq!(
                        hit.direction as u64,
                        expected[0].as_u64().unwrap(),
                        "face case {index}"
                    );
                    assert_eq!(
                        hit.inside,
                        expected[1].as_bool().unwrap(),
                        "inside case {index}"
                    );
                    let expected_position = vector(&expected[2]);
                    assert_eq!(
                        [
                            hit.position.x.to_bits(),
                            hit.position.y.to_bits(),
                            hit.position.z.to_bits()
                        ],
                        [
                            expected_position.x.to_bits(),
                            expected_position.y.to_bits(),
                            expected_position.z.to_bits()
                        ],
                        "point case {index}"
                    );
                }
            };
            let raw = clip(from, to, &boxes);
            check(raw, &case[5]);
            let interaction: Vec<_> = super::super::block_interaction_shapes::boxes(state.as_u16())
                .iter()
                .map(|b| {
                    BoundingBox::new_array([b[0], b[1], b[2]], [b[3], b[4], b[5]])
                        .shift(pos.0.to_f64())
                })
                .collect();
            let result = with_interaction_override(from, raw, clip(from, to, &interaction));
            check(result, &case[6]);
            if case[5] != case[6] {
                override_count += 1;
            }
        }
        assert!(override_count > 20, "fixture must exercise face overrides");
    }

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
