use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnLandedUponArgs,
        OnPlaceArgs, OnProjectileHitArgs, OnScheduledTickArgs, PathComputationType, RandomTickArgs,
    },
    entity::falling::FallingEntity,
    world::World,
};
use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockState, BlockStateId, Fluid,
    block_properties::{
        PointedDripstoneLikeProperties, SpeleothemThickness, SulfurSpikeProperties,
        VerticalDirection,
    },
    tag::{self, Taggable},
    world::WorldEvent,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

#[pumpkin_block("minecraft:pointed_dripstone")]
pub struct DripstoneBlock;

// The two vanilla SpeleothemBlock subclasses share placement, shape, falling and
// growth behavior. Their generated property readers still require their own IDs.
#[derive(Clone, Copy)]
struct Properties {
    direction: BlockDirection,
    thickness: SpeleothemThickness,
    waterlogged: bool,
}
impl Properties {
    fn read(state: BlockStateId) -> Self {
        let (direction, thickness, waterlogged) = if state.to_block() == &Block::SULFUR_SPIKE {
            let p = SulfurSpikeProperties::from_state_id(state);
            (p.vertical_direction, p.thickness, p.waterlogged)
        } else {
            let p = PointedDripstoneLikeProperties::from_state_id(state);
            (p.vertical_direction, p.thickness, p.waterlogged)
        };
        Self {
            direction: if direction == VerticalDirection::Up {
                BlockDirection::Up
            } else {
                BlockDirection::Down
            },
            thickness,
            waterlogged,
        }
    }
    fn state(self, block: &Block) -> BlockStateId {
        let vertical_direction = if self.direction == BlockDirection::Up {
            VerticalDirection::Up
        } else {
            VerticalDirection::Down
        };
        if block == &Block::SULFUR_SPIKE {
            let mut p = SulfurSpikeProperties::default(block);
            p.vertical_direction = vertical_direction;
            p.thickness = self.thickness;
            p.waterlogged = self.waterlogged;
            p.to_state_id(block)
        } else {
            let mut p = PointedDripstoneLikeProperties::default(block);
            p.vertical_direction = vertical_direction;
            p.thickness = self.thickness;
            p.waterlogged = self.waterlogged;
            p.to_state_id(block)
        }
    }
}

fn is_speleothem(state: &BlockState) -> bool {
    matches!(
        state.id.to_block().id,
        BlockId::POINTED_DRIPSTONE | BlockId::SULFUR_SPIKE
    )
}
fn points(state: &BlockState, direction: BlockDirection) -> bool {
    is_speleothem(state) && Properties::read(state.id).direction == direction
}
fn is_tip(state: &BlockState, merged: bool) -> bool {
    is_speleothem(state)
        && matches!(
            Properties::read(state.id).thickness,
            SpeleothemThickness::Tip
        )
        || merged
            && is_speleothem(state)
            && Properties::read(state.id).thickness == SpeleothemThickness::TipMerge
}
fn free_hanging(state: &BlockState) -> bool {
    points(state, BlockDirection::Down)
        && is_tip(state, false)
        && !Properties::read(state.id).waterlogged
}
fn valid_placement(
    access: &dyn BlockAccessor,
    block: &Block,
    pos: &BlockPos,
    direction: BlockDirection,
) -> bool {
    let behind = access.get_block_state(&pos.offset(direction.opposite().to_offset()));
    behind.is_side_solid(direction) || (behind.id.to_block() == block && points(behind, direction))
}
fn thickness(
    access: &dyn BlockAccessor,
    block: &Block,
    pos: &BlockPos,
    direction: BlockDirection,
    merge: bool,
) -> SpeleothemThickness {
    let front = access.get_block_state(&pos.offset(direction.to_offset()));
    if front.id.to_block() == block && points(front, direction.opposite()) {
        if merge || Properties::read(front.id).thickness == SpeleothemThickness::TipMerge {
            SpeleothemThickness::TipMerge
        } else {
            SpeleothemThickness::Tip
        }
    } else if !points(front, direction) {
        SpeleothemThickness::Tip
    } else if is_tip(front, true) {
        SpeleothemThickness::Frustum
    } else if !points(
        access.get_block_state(&pos.offset(direction.opposite().to_offset())),
        direction,
    ) {
        SpeleothemThickness::Base
    } else {
        SpeleothemThickness::Middle
    }
}
fn source_water(access: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let (fluid, state) = World::fluid_state_from_block_state(access.get_block_state_id(pos));
    fluid.matches_type(&Fluid::WATER) && state.is_source
}
fn find_vertical(
    world: &World,
    start: BlockPos,
    direction: BlockDirection,
    max_steps: i32,
    path: impl Fn(&BlockPos, &BlockState) -> bool,
    target: impl Fn(&BlockState) -> bool,
) -> Option<BlockPos> {
    let mut pos = start;
    for _ in 1..max_steps {
        pos = pos.offset(direction.to_offset());
        let state = world.get_block_state(&pos);
        if target(state) {
            return Some(pos);
        }
        if !world.is_in_height_limit(pos.0.y) || !path(&pos, state) {
            return None;
        }
    }
    None
}
fn find_tip(
    world: &World,
    pos: BlockPos,
    state: &BlockState,
    max_steps: i32,
    merged: bool,
) -> Option<BlockPos> {
    if is_tip(state, merged) {
        return Some(pos);
    }
    let direction = Properties::read(state.id).direction;
    find_vertical(
        world,
        pos,
        direction,
        max_steps,
        |_, next| next.id.to_block() == state.id.to_block() && points(next, direction),
        |next| is_tip(next, merged),
    )
}
fn start_position(world: &World, pos: &BlockPos, state: &BlockState) -> bool {
    points(state, BlockDirection::Down) && world.get_block(&pos.up()) != state.id.to_block()
}
fn create(
    world: &Arc<World>,
    block: &Block,
    pos: BlockPos,
    direction: BlockDirection,
    thickness: SpeleothemThickness,
) {
    let state = Properties {
        direction,
        thickness,
        waterlogged: source_water(world.as_ref(), &pos),
    }
    .state(block);
    world.set_block_state(&pos, state, BlockFlags::NOTIFY_ALL);
}
fn can_tip_grow(world: &World, block: &Block, pos: BlockPos, state: &BlockState) -> bool {
    let direction = Properties::read(state.id).direction;
    let target = world.get_block_state(&pos.offset(direction.to_offset()));
    World::fluid_state_from_block_state(target.id).0 == &Fluid::EMPTY
        && (target.is_air()
            || target.id.to_block() == block
                && points(target, direction.opposite())
                && is_tip(target, false))
}
fn grow(world: &Arc<World>, block: &Block, from: BlockPos, direction: BlockDirection) {
    let pos = from.offset(direction.to_offset());
    let current = world.get_block_state(&pos);
    if current.id.to_block() == block
        && points(current, direction.opposite())
        && is_tip(current, false)
    {
        let (top, bottom) = if points(current, BlockDirection::Up) {
            (pos.up(), pos)
        } else {
            (pos, pos.down())
        };
        create(
            world,
            block,
            top,
            BlockDirection::Down,
            SpeleothemThickness::TipMerge,
        );
        create(
            world,
            block,
            bottom,
            BlockDirection::Up,
            SpeleothemThickness::TipMerge,
        );
    } else if current.is_air() || current.id.to_block() == &Block::WATER {
        create(world, block, pos, direction, SpeleothemThickness::Tip);
    }
}
fn grow_below(world: &Arc<World>, block: &Block, start: BlockPos) {
    let mut pos = start;
    for _ in 0..10 {
        pos = pos.down();
        let state = world.get_block_state(&pos);
        if World::fluid_state_from_block_state(state.id).0 != &Fluid::EMPTY {
            return;
        }
        if state.id.to_block() == block
            && points(state, BlockDirection::Up)
            && is_tip(state, false)
            && can_tip_grow(world, block, pos, state)
        {
            grow(world, block, pos, BlockDirection::Up);
            return;
        }
        let (below_fluid, _) =
            World::fluid_state_from_block_state(world.get_block_state_id(&pos.down()));
        if valid_placement(world.as_ref(), block, &pos, BlockDirection::Up)
            && !below_fluid.has_tag(&tag::Fluid::MINECRAFT_WATER)
        {
            grow(world, block, pos.down(), BlockDirection::Up);
            return;
        }
        if block == &Block::POINTED_DRIPSTONE && !can_drip_through(state) {
            return;
        }
    }
}
fn grow_if_possible(world: &Arc<World>, block: &Block, pos: BlockPos, state: &BlockState) {
    let (base, max_length) = if block == &Block::SULFUR_SPIKE {
        (&Block::SULFUR, 2)
    } else {
        (&Block::DRIPSTONE_BLOCK, 7)
    };
    if world.get_block(&pos.up()) != base
        || block == &Block::POINTED_DRIPSTONE && !source_water(world.as_ref(), &pos.up_height(2))
    {
        return;
    }
    let Some(tip) = find_tip(world, pos, state, max_length, false) else {
        return;
    };
    let tip_state = world.get_block_state(&tip);
    if !free_hanging(tip_state) || !can_tip_grow(world, block, tip, tip_state) {
        return;
    }
    if world.rand_bool() {
        grow(world, block, tip, BlockDirection::Down);
    } else {
        grow_below(world, block, tip);
    }
}

impl BlockBehaviour for DripstoneBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        valid_placement(
            args.block_accessor,
            args.block,
            args.position,
            Properties::read(args.state.id).direction,
        )
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let (_, pitch) = args.player.rotation();
        let preferred = if pitch >= 0.0 {
            BlockDirection::Up
        } else {
            BlockDirection::Down
        };
        let direction = if valid_placement(args.world, args.block, args.position, preferred) {
            preferred
        } else if valid_placement(args.world, args.block, args.position, preferred.opposite()) {
            preferred.opposite()
        } else {
            return Block::AIR.default_state.id;
        };
        Properties {
            direction,
            thickness: thickness(
                args.world,
                args.block,
                args.position,
                direction,
                !args.player.get_entity().is_sneaking(),
            ),
            waterlogged: source_water(args.world, args.position),
        }
        .state(args.block)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut p = Properties::read(args.state_id);
        if p.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }
        if args.direction.is_horizontal()
            || p.direction == BlockDirection::Down
                && args
                    .world
                    .is_block_tick_scheduled(args.position, args.block)
        {
            return args.state_id;
        }
        if args.direction == p.direction.opposite()
            && !valid_placement(args.world, args.block, args.position, p.direction)
        {
            args.world.schedule_block_tick(
                args.block,
                *args.position,
                if p.direction == BlockDirection::Down {
                    2
                } else {
                    1
                },
                TickPriority::Normal,
            );
            args.state_id
        } else {
            p.thickness = thickness(
                args.world,
                args.block,
                args.position,
                p.direction,
                p.thickness == SpeleothemThickness::TipMerge,
            );
            p.state(args.block)
        }
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if points(state, BlockDirection::Up)
            && !valid_placement(
                args.world.as_ref(),
                args.block,
                args.position,
                BlockDirection::Up,
            )
        {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
            return;
        }
        let mut pos = *args.position;
        let mut state = state;
        while points(state, BlockDirection::Down) {
            let falling = FallingEntity::replace_spawn(args.world, pos, state.id);
            if is_tip(state, true) {
                falling.set_hurts_entities((1 + args.position.0.y - pos.0.y).max(6) as f32, 40);
                break;
            }
            pos = pos.down();
            state = args.world.get_block_state(&pos);
        }
    }
    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if args.projectile.get_entity().entity_type == &pumpkin_data::entity::EntityType::TRIDENT
            && crate::entity::projectile::may_interact(args.projectile, args.world, args.position)
            && crate::entity::projectile::may_break(args.projectile, args.world)
            && args.projectile.get_entity().velocity.load().length() > 0.6
        {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if let Some(living) = args.entity.get_living_entity() {
            if state.id.to_block() == &Block::POINTED_DRIPSTONE
                && points(state, BlockDirection::Up)
                && is_tip(state, false)
            {
                living.handle_fall_damage_with_type(
                    args.entity,
                    args.fall_distance + 2.5,
                    2.0,
                    pumpkin_data::damage::DamageType::STALAGMITE,
                );
            } else {
                living.handle_fall_damage(args.entity, args.fall_distance, 1.0);
            }
        }
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if args.block == &Block::POINTED_DRIPSTONE {
            transfer_fluid(args.world, *args.position, state, args.world.rand_f32());
        }
        if args.world.rand_f32() < 0.011377778 && start_position(args.world, args.position, state) {
            grow_if_possible(args.world, args.block, *args.position, state);
        }
    }
    fn is_pathfindable(&self, _: &BlockState, _: PathComputationType) -> bool {
        false
    }
}

fn can_drip_through(state: &BlockState) -> bool {
    if state.is_air() {
        return true;
    }
    if state.is_solid_render() || World::fluid_state_from_block_state(state.id).0 != &Fluid::EMPTY {
        return false;
    }
    // Intersection with the centered four-pixel-wide vertical drip column.
    !state.get_block_collision_shapes().any(|shape| {
        shape.max.x > 0.375
            && shape.min.x < 0.625
            && shape.max.z > 0.375
            && shape.min.z < 0.625
            && shape.max.y > 0.0
            && shape.min.y < 1.0
    })
}
fn fluid_above(
    world: &World,
    pos: BlockPos,
    state: &BlockState,
) -> Option<(BlockPos, &'static Fluid, &'static BlockState)> {
    if !points(state, BlockDirection::Down) {
        return None;
    }
    let block = state.id.to_block();
    let root = find_vertical(
        world,
        pos,
        BlockDirection::Up,
        11,
        |_, next| next.id.to_block() == block && points(next, BlockDirection::Down),
        |next| next.id.to_block() != block,
    )?;
    let above = root.up();
    let source = world.get_block_state(&above);
    let fluid = if source.id.to_block() == &Block::MUD
        && !world.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayWaterEvaporates,
            &above,
        ) {
        &Fluid::WATER
    } else {
        let (fluid, state) = World::fluid_state_from_block_state(source.id);
        if state.is_source && fluid.matches_type(&Fluid::WATER) {
            &Fluid::WATER
        } else if state.is_source && fluid.matches_type(&Fluid::LAVA) {
            &Fluid::LAVA
        } else {
            fluid
        }
    };
    Some((above, fluid, source))
}
pub(super) fn accepts_drip(state: &BlockState, fluid: &Fluid) -> bool {
    state.id.to_block() == &Block::CAULDRON
        || state.id.to_block() == &Block::WATER_CAULDRON && fluid == &Fluid::WATER
}
pub(super) fn cauldron_drip(world: &World, pos: BlockPos) -> Option<&'static Fluid> {
    let tip = find_vertical(
        world,
        pos,
        BlockDirection::Up,
        11,
        |_, state| can_drip_through(state),
        free_hanging,
    )?;
    let (_, fluid, _) = fluid_above(world, tip, world.get_block_state(&tip))?;
    (fluid == &Fluid::WATER || fluid == &Fluid::LAVA).then_some(fluid)
}
fn transfer_fluid(world: &Arc<World>, pos: BlockPos, state: &BlockState, random: f32) {
    if random > 0.17578125 || !start_position(world, &pos, state) {
        return;
    }
    let Some((source_pos, fluid, source_state)) = fluid_above(world, pos, state) else {
        return;
    };
    let probability = if fluid == &Fluid::WATER {
        0.17578125
    } else if fluid == &Fluid::LAVA {
        0.05859375
    } else {
        return;
    };
    if random >= probability {
        return;
    }
    let Some(tip) = find_tip(world, pos, state, 11, false) else {
        return;
    };
    if source_state.id.to_block() == &Block::MUD && fluid == &Fluid::WATER {
        world.set_block_state(
            &source_pos,
            Block::CLAY.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        crate::block::shape::push_entities_up(
            world,
            source_pos,
            source_state,
            Block::CLAY.default_state,
        );
        world.emit_game_event_from_entity(
            "block_change",
            source_pos.to_centered_f64(),
            None,
            Some(Block::CLAY.default_state.id),
        );
        world.sync_world_event(WorldEvent::DripstoneDrip, tip, 0);
    } else if let Some(cauldron) = find_vertical(
        world,
        tip,
        BlockDirection::Down,
        11,
        |_, state| can_drip_through(state),
        |state| accepts_drip(state, fluid),
    ) {
        world.sync_world_event(WorldEvent::DripstoneDrip, tip, 0);
        world.schedule_block_tick(
            world.get_block(&cauldron),
            cauldron,
            (50 + tip.0.y - cauldron.0.y) as u32,
            TickPriority::Normal,
        );
    }
}
