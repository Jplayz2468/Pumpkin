//! FlowingFluid.getFlow, including empty neighboring cells and Java float math.
use pumpkin_data::HorizontalFacingExt;
use pumpkin_data::{BlockDirection, BlockStateId, block_properties::blocks_movement, fluid::Fluid};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

pub struct Cell {
    pub affects_flow: bool,
    pub height: f32,
    pub blocks_motion: bool,
}

pub fn cell(id: BlockStateId, fluid: &Fluid) -> Cell {
    let (neighbor, state) = super::World::fluid_state_from_block_state(id);
    Cell {
        affects_flow: state.is_empty || neighbor.matches_type(fluid),
        height: state.height,
        blocks_motion: blocks_movement(id.to_state(), id.to_block_id()),
    }
}

fn normalize(value: Vector3<f64>) -> Vector3<f64> {
    let length = value.length();
    if length < f64::from(1.0e-5_f32) {
        Vector3::default()
    } else {
        Vector3::new(value.x / length, value.y / length, value.z / length)
    }
}

pub fn velocity(
    pos: BlockPos,
    own_height: f32,
    falling: bool,
    mut cell: impl FnMut(BlockPos) -> Cell,
    mut solid_face: impl FnMut(BlockPos, BlockDirection) -> bool,
) -> Vector3<f64> {
    let mut flow = Vector3::default();
    for direction in BlockDirection::horizontal_worldgen() {
        let offset = direction.to_offset();
        let neighbor_pos = pos.offset(offset);
        let neighbor = cell(neighbor_pos);
        if !neighbor.affects_flow {
            continue;
        }
        let mut distance = 0.0_f32;
        if neighbor.height == 0.0 {
            if !neighbor.blocks_motion {
                let below = cell(neighbor_pos.down());
                if below.affects_flow && below.height > 0.0 {
                    distance = own_height - (below.height - 0.888_888_9_f32);
                }
            }
        } else if neighbor.height > 0.0 {
            distance = own_height - neighbor.height;
        }
        if distance != 0.0 {
            flow.x += f64::from(offset.x as f32 * distance);
            flow.z += f64::from(offset.z as f32 * distance);
        }
    }
    if falling {
        for direction in BlockDirection::horizontal_worldgen() {
            let neighbor = pos.offset(direction.to_offset());
            let direction = direction.to_block_direction();
            if solid_face(neighbor, direction) || solid_face(neighbor.up(), direction) {
                flow = normalize(flow).add_raw(0.0, -6.0, 0.0);
                break;
            }
        }
    }
    normalize(flow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn java_world_fluid_currents() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fluid_flow_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let states: std::collections::HashMap<_, _> = case[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    (
                        BlockPos::new(
                            v[0].as_i64().unwrap() as i32,
                            v[1].as_i64().unwrap() as i32,
                            v[2].as_i64().unwrap() as i32,
                        ),
                        BlockStateId::new(v[3].as_u64().unwrap() as u16).unwrap(),
                    )
                })
                .collect();
            let state_at = |pos: BlockPos| {
                states
                    .get(&pos)
                    .copied()
                    .unwrap_or(Block::AIR.default_state.id)
            };
            let origin = BlockPos::new(0, 0, 0);
            let (fluid, state) =
                super::super::World::fluid_state_from_block_state(state_at(origin));
            let actual = velocity(
                origin,
                state.height,
                state.falling,
                |pos| cell(state_at(pos), fluid),
                |pos, direction| {
                    let id = state_at(pos);
                    let (other, _) = super::super::World::fluid_state_from_block_state(id);
                    !fluid.matches_type(other)
                        && id.to_block() != &Block::ICE
                        && id.to_block() != &Block::FROSTED_ICE
                        && id.to_state().is_side_solid(direction)
                },
            );
            assert_eq!(
                [actual.x.to_bits(), actual.y.to_bits(), actual.z.to_bits()],
                [
                    case[1][0].as_u64().unwrap(),
                    case[1][1].as_u64().unwrap(),
                    case[1][2].as_u64().unwrap()
                ],
                "case {index}"
            );
        }
    }
}
