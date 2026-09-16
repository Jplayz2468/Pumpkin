use super::super::block_receives_redstone_power;
use super::RailProperties;
use crate::{block::OnPlaceArgs, entity::EntityBase, world::World};
use pumpkin_data::{
    Block, BlockDirection, BlockStateId,
    block_properties::{HorizontalFacing, RailShape},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

pub(super) fn rail_placement_is_valid(world: &World, block: &Block, pos: &BlockPos) -> bool {
    if !can_place_rail_at(world, pos) {
        return false;
    }

    let state_id = world.get_block_state_id(pos);
    let rail_props = RailProperties::new(state_id, block);
    let rail_leaning_direction = match rail_props.shape() {
        RailShape::AscendingNorth => Some(HorizontalFacing::North),
        RailShape::AscendingSouth => Some(HorizontalFacing::South),
        RailShape::AscendingEast => Some(HorizontalFacing::East),
        RailShape::AscendingWest => Some(HorizontalFacing::West),
        _ => None,
    };

    if let Some(direction) = rail_leaning_direction
        && !can_place_rail_at(world, &pos.offset(direction.to_offset()).up())
    {
        return false;
    }

    true
}

pub(super) fn can_place_rail_at(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let state = world.get_block_state(&pos.down());
    [
        [0.0, 0.125, 0.0, 1.0],
        [0.875, 1.0, 0.0, 1.0],
        [0.125, 0.875, 0.0, 0.125],
        [0.125, 0.875, 0.875, 1.0],
    ]
    .into_iter()
    .all(|region| state.collision_face_covers(pos.down(), BlockDirection::Up, region))
}

pub(super) fn source_water(world: &World, pos: &BlockPos) -> bool {
    let (fluid, state) = World::fluid_state_from_block_state(world.get_block_state_id(pos));
    fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) && state.is_source
}

pub(super) fn water_update(args: crate::block::GetStateForNeighborUpdateArgs<'_>) -> BlockStateId {
    if args.state_id.to_state().is_waterlogged() {
        args.world.schedule_fluid_tick(
            &pumpkin_data::fluid::Fluid::WATER,
            *args.position,
            pumpkin_data::fluid::Fluid::WATER.flow_speed as u32,
            pumpkin_world::tick::TickPriority::Normal,
        );
    }
    args.state_id
}

pub(super) fn removed(args: crate::block::OnStateReplacedArgs<'_>, straight: bool) {
    if args.moved {
        return;
    }
    if RailProperties::new(args.old_state_id, args.block)
        .shape()
        .is_ascending()
    {
        args.world
            .update_neighbors_at(&args.position.up(), args.block, None);
    }
    if straight {
        args.world
            .update_neighbors_at(args.position, args.block, None);
        args.world
            .update_neighbors_at(&args.position.down(), args.block, None);
    }
}

pub(super) fn placement(args: OnPlaceArgs<'_>) -> BlockStateId {
    let mut props = RailProperties::default(args.block);
    props.set_waterlogged(source_water(args.world, args.position));
    props.set_shape(match args.player.get_entity().get_horizontal_facing() {
        HorizontalFacing::East | HorizontalFacing::West => RailShape::EastWest,
        _ => RailShape::NorthSouth,
    });
    props.to_state_id(args.block)
}

pub(super) fn update_direction(
    world: &Arc<World>,
    pos: BlockPos,
    state: BlockStateId,
    first: bool,
) -> BlockStateId {
    let mut rail = RailState::new(world, pos, state);
    let default = RailProperties::new(state, state.to_block()).shape();
    rail.place(block_receives_redstone_power(world, &pos), first, default);
    rail.state
}

pub(super) fn potential_connections(world: &Arc<World>, pos: BlockPos) -> usize {
    [pos.north(), pos.south(), pos.west(), pos.east()]
        .into_iter()
        .filter(|pos| {
            [*pos, pos.up(), pos.down()]
                .into_iter()
                .any(|pos| is_rail(world.get_block(&pos)))
        })
        .count()
}

fn is_rail(block: &Block) -> bool {
    matches!(
        block.id,
        pumpkin_data::BlockId::RAIL
            | pumpkin_data::BlockId::POWERED_RAIL
            | pumpkin_data::BlockId::DETECTOR_RAIL
            | pumpkin_data::BlockId::ACTIVATOR_RAIL
    )
}

struct RailState<'a> {
    world: &'a Arc<World>,
    pos: BlockPos,
    block: &'static Block,
    state: BlockStateId,
    straight: bool,
    connections: Vec<BlockPos>,
}
impl<'a> RailState<'a> {
    fn new(world: &'a Arc<World>, pos: BlockPos, state: BlockStateId) -> Self {
        let block = state.to_block();
        let mut rail = Self {
            world,
            pos,
            block,
            state,
            straight: block != &Block::RAIL,
            connections: Vec::new(),
        };
        rail.update_connections(RailProperties::new(state, block).shape());
        rail
    }
    fn update_connections(&mut self, shape: RailShape) {
        let p = self.pos;
        self.connections = match shape {
            RailShape::NorthSouth => vec![p.north(), p.south()],
            RailShape::EastWest => vec![p.west(), p.east()],
            RailShape::AscendingEast => vec![p.west(), p.east().up()],
            RailShape::AscendingWest => vec![p.west().up(), p.east()],
            RailShape::AscendingNorth => vec![p.north().up(), p.south()],
            RailShape::AscendingSouth => vec![p.north(), p.south().up()],
            RailShape::SouthEast => vec![p.east(), p.south()],
            RailShape::SouthWest => vec![p.west(), p.south()],
            RailShape::NorthWest => vec![p.west(), p.north()],
            RailShape::NorthEast => vec![p.east(), p.north()],
        };
    }
    fn get_rail(&self, pos: BlockPos) -> Option<Self> {
        [pos, pos.up(), pos.down()].into_iter().find_map(|pos| {
            let state = self.world.get_block_state_id(&pos);
            is_rail(state.to_block()).then(|| Self::new(self.world, pos, state))
        })
    }
    fn has_connection(&self, pos: BlockPos) -> bool {
        self.connections
            .iter()
            .any(|other| other.0.x == pos.0.x && other.0.z == pos.0.z)
    }
    fn remove_soft_connections(&mut self) {
        let mut index = 0;
        while index < self.connections.len() {
            if let Some(rail) = self.get_rail(self.connections[index])
                && rail.has_connection(self.pos)
            {
                self.connections[index] = rail.pos;
                index += 1;
            } else {
                self.connections.remove(index);
            }
        }
    }
    fn can_connect_to(&self, rail: &Self) -> bool {
        self.has_connection(rail.pos) || self.connections.len() != 2
    }
    fn has_neighbor(&self, pos: BlockPos) -> bool {
        self.get_rail(pos).is_some_and(|mut neighbor| {
            neighbor.remove_soft_connections();
            neighbor.can_connect_to(self)
        })
    }
    fn slope(&self, mut shape: RailShape) -> RailShape {
        if shape == RailShape::NorthSouth {
            if is_rail(self.world.get_block(&self.pos.north().up())) {
                shape = RailShape::AscendingNorth;
            }
            if is_rail(self.world.get_block(&self.pos.south().up())) {
                shape = RailShape::AscendingSouth;
            }
        }
        if shape == RailShape::EastWest {
            if is_rail(self.world.get_block(&self.pos.east().up())) {
                shape = RailShape::AscendingEast;
            }
            if is_rail(self.world.get_block(&self.pos.west().up())) {
                shape = RailShape::AscendingWest;
            }
        }
        shape
    }
    fn exclusive_curve(n: bool, s: bool, w: bool, e: bool) -> Option<RailShape> {
        if s && e && !n && !w {
            Some(RailShape::SouthEast)
        } else if s && w && !n && !e {
            Some(RailShape::SouthWest)
        } else if n && w && !s && !e {
            Some(RailShape::NorthWest)
        } else if n && e && !s && !w {
            Some(RailShape::NorthEast)
        } else {
            None
        }
    }
    fn set_shape(&mut self, shape: RailShape) {
        let mut props = RailProperties::new(self.state, self.block);
        props.set_shape(shape);
        self.state = props.to_state_id(self.block);
    }
    fn connect_to(&mut self, rail: &Self) {
        self.connections.push(rail.pos);
        let n = self.has_connection(self.pos.north());
        let s = self.has_connection(self.pos.south());
        let w = self.has_connection(self.pos.west());
        let e = self.has_connection(self.pos.east());
        let mut shape = if w || e {
            RailShape::EastWest
        } else {
            RailShape::NorthSouth
        };
        if !self.straight
            && let Some(curve) = Self::exclusive_curve(n, s, w, e)
        {
            shape = curve;
        }
        self.set_shape(self.slope(shape));
        self.world
            .set_block_state(&self.pos, self.state, BlockFlags::NOTIFY_ALL);
    }
    fn place(&mut self, powered: bool, first: bool, default: RailShape) {
        let n = self.has_neighbor(self.pos.north());
        let s = self.has_neighbor(self.pos.south());
        let w = self.has_neighbor(self.pos.west());
        let e = self.has_neighbor(self.pos.east());
        let ns = n || s;
        let ew = w || e;
        let mut shape = if ns && !ew {
            Some(RailShape::NorthSouth)
        } else if ew && !ns {
            Some(RailShape::EastWest)
        } else {
            None
        };
        if !self.straight
            && let Some(curve) = Self::exclusive_curve(n, s, w, e)
        {
            shape = Some(curve);
        }
        if shape.is_none() {
            shape = if ns && ew {
                Some(default)
            } else if ns {
                Some(RailShape::NorthSouth)
            } else if ew {
                Some(RailShape::EastWest)
            } else {
                None
            };
            if !self.straight {
                let choices = if powered {
                    [
                        (s && e, RailShape::SouthEast),
                        (s && w, RailShape::SouthWest),
                        (n && e, RailShape::NorthEast),
                        (n && w, RailShape::NorthWest),
                    ]
                } else {
                    [
                        (n && w, RailShape::NorthWest),
                        (n && e, RailShape::NorthEast),
                        (s && w, RailShape::SouthWest),
                        (s && e, RailShape::SouthEast),
                    ]
                };
                for (connected, candidate) in choices {
                    if connected {
                        shape = Some(candidate);
                    }
                }
            }
        }
        let shape = shape.map(|shape| self.slope(shape)).unwrap_or(default);
        self.update_connections(shape);
        self.set_shape(shape);
        if first || self.world.get_block_state_id(&self.pos) != self.state {
            self.world
                .set_block_state(&self.pos, self.state, BlockFlags::NOTIFY_ALL);
            for pos in &self.connections {
                if let Some(mut neighbor) = self.get_rail(*pos) {
                    neighbor.remove_soft_connections();
                    if neighbor.can_connect_to(self) {
                        neighbor.connect_to(self);
                    }
                }
            }
        }
    }
}
