use std::sync::Arc;

use pumpkin_data::block_properties::BubbleColumnLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnEntityCollisionArgs, OnNeighborUpdateArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::world::World;

const CREATE_DELAY_TICKS: u32 = 20;
const REMOVE_DELAY_TICKS: u32 = 5;

const UPWARD_ACCELERATION: f64 = 0.06;
const UPWARD_MAX_SPEED: f64 = 0.7;
const SURFACE_UPWARD_ACCELERATION: f64 = 0.1;
const SURFACE_UPWARD_MAX_SPEED: f64 = 1.8;
const DOWNWARD_ACCELERATION: f64 = -0.03;
const DOWNWARD_MIN_SPEED: f64 = -0.3;

pub struct BubbleColumnBlock;

impl BlockMetadata for BubbleColumnBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BUBBLE_COLUMN, BlockId::WATER].into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BubbleColumnKind {
    Upward,
    Downward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconcileAction {
    SetBubble(BubbleColumnKind),
    RestoreWater,
    Stop,
}

fn source_water_state() -> BlockStateId {
    Fluid::WATER
        .states
        .iter()
        .find(|state| state.is_still && state.is_source)
        .map_or(BlockStateId::new_or_air(0), |state| state.block_state_id)
}

fn bubble_column_state(kind: BubbleColumnKind) -> BlockStateId {
    BubbleColumnLikeProperties {
        r#drag: matches!(kind, BubbleColumnKind::Downward),
    }
    .to_state_id(&Block::BUBBLE_COLUMN)
}

fn kind_from_support(block: &Block) -> Option<BubbleColumnKind> {
    if block.has_tag(&tag::Block::MINECRAFT_ENABLES_BUBBLE_COLUMN_PUSH_UP) {
        Some(BubbleColumnKind::Upward)
    } else if block.has_tag(&tag::Block::MINECRAFT_ENABLES_BUBBLE_COLUMN_DRAG_DOWN) {
        Some(BubbleColumnKind::Downward)
    } else {
        None
    }
}

fn kind_from_state(state: BlockStateId) -> BubbleColumnKind {
    let props = BubbleColumnLikeProperties::from_state_id(state);
    if props.r#drag {
        BubbleColumnKind::Downward
    } else {
        BubbleColumnKind::Upward
    }
}

fn kind_from_below(block: &Block, state: BlockStateId) -> Option<BubbleColumnKind> {
    if block == &Block::BUBBLE_COLUMN {
        Some(kind_from_state(state))
    } else {
        kind_from_support(block)
    }
}

fn is_source_water_state(state: BlockStateId) -> bool {
    if state.to_block() != &Block::WATER {
        return false;
    }
    let (fluid, fluid_state) = World::fluid_state_from_block_state(state);
    let source_fluid = if fluid.matches_type(&Fluid::WATER) && fluid_state.is_source {
        &Fluid::WATER
    } else {
        fluid
    };
    source_fluid.has_tag(&tag::Fluid::MINECRAFT_BUBBLE_COLUMN_CAN_OCCUPY)
        && fluid_state.is_source
        && fluid_state.level == 8
}

fn is_source_water(world: &World, position: BlockPos) -> bool {
    is_source_water_state(world.get_block_state_id(&position))
}

fn reconcile_action(
    current_block: &Block,
    current_state: BlockStateId,
    below_block: &Block,
    below_state: BlockStateId,
) -> ReconcileAction {
    let current_is_bubble = current_block == &Block::BUBBLE_COLUMN;
    let current_is_source_water = is_source_water_state(current_state);

    if let Some(kind) = kind_from_below(below_block, below_state)
        && (current_is_bubble || current_is_source_water)
    {
        return ReconcileAction::SetBubble(kind);
    }

    if current_is_bubble {
        ReconcileAction::RestoreWater
    } else {
        ReconcileAction::Stop
    }
}

fn schedule_reconcile(world: &World, block: &Block, position: BlockPos, delay: u32) {
    world.schedule_block_tick(block, position, delay, TickPriority::Normal);
}

impl BubbleColumnBlock {
    pub fn update_column(world: &Arc<World>, pos: &BlockPos) {
        let (block, state) = world.get_block_and_state(pos);
        let (below, below_state) = world.get_block_and_state(&pos.down());
        let new = match reconcile_action(block, state.id, below, below_state.id) {
            ReconcileAction::SetBubble(kind) => bubble_column_state(kind),
            ReconcileAction::RestoreWater => source_water_state(),
            ReconcileAction::Stop => {
                if !is_source_water_state(state.id) {
                    return;
                }
                state.id
            }
        };
        world.set_block_state(pos, new, BlockFlags::NOTIFY_LISTENERS);
        let mut pos = pos.up();
        while world.is_in_height_limit(pos.0.y) {
            let state = world.get_block_state_id(&pos);
            if state.to_block() != &Block::BUBBLE_COLUMN && !is_source_water_state(state) {
                break;
            }
            if world.set_block_state(&pos, new, BlockFlags::NOTIFY_LISTENERS) == new {
                break;
            }
            pos = pos.up();
        }
    }
}

fn bubble_column_vertical_velocity(
    current_y: f64,
    kind: BubbleColumnKind,
    at_surface: bool,
) -> f64 {
    match (kind, at_surface) {
        (BubbleColumnKind::Upward, true) => {
            (current_y + SURFACE_UPWARD_ACCELERATION).min(SURFACE_UPWARD_MAX_SPEED)
        }
        (BubbleColumnKind::Upward, false) => {
            (current_y + UPWARD_ACCELERATION).min(UPWARD_MAX_SPEED)
        }
        (BubbleColumnKind::Downward, surface) => {
            (current_y + DOWNWARD_ACCELERATION).max(if surface { -0.9 } else { DOWNWARD_MIN_SPEED })
        }
    }
}

fn bubble_column_velocity(
    current: Vector3<f64>,
    kind: BubbleColumnKind,
    at_surface: bool,
) -> Vector3<f64> {
    Vector3::new(
        current.x,
        bubble_column_vertical_velocity(current.y, kind, at_surface),
        current.z,
    )
}

impl BlockBehaviour for BubbleColumnBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if args.block != &Block::BUBBLE_COLUMN
            || args
                .entity
                .get_player()
                .is_some_and(|player| player.is_flying())
        {
            return;
        }
        let kind = kind_from_state(args.state.id);
        let above = args.world.get_block_state(&args.position.up());
        let at_surface = above.collision_shapes.is_empty()
            && World::fluid_state_from_block_state(above.id).1.is_empty;
        let entity = args.entity.get_entity();
        if at_surface
            && let Some(boat) = args
                .entity
                .cast_any()
                .downcast_ref::<crate::entity::vehicle::boat::BoatEntity>()
        {
            boat.on_above_bubble_column(matches!(kind, BubbleColumnKind::Downward));
            return;
        }
        if let Some(arrow) = args
            .entity
            .cast_any()
            .downcast_ref::<crate::entity::projectile::arrow::ArrowEntity>()
            && arrow.in_ground.load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        let velocity = entity.velocity.load();
        let projectile = crate::entity::projectile::is_projectile(entity.entity_type)
            && entity.entity_type != &pumpkin_data::entity::EntityType::ENDER_PEARL;
        let new_velocity = if projectile {
            velocity.add_raw(
                0.0,
                if matches!(kind, BubbleColumnKind::Downward) {
                    -0.03
                } else if at_surface {
                    0.1
                } else {
                    0.06
                },
                0.0,
            )
        } else {
            bubble_column_velocity(velocity, kind, at_surface)
        };
        entity.velocity.store(new_velocity);
        if at_surface {
            for _ in 0..2 {
                args.world.spawn_particle(
                    args.position.to_f64().add_raw(
                        args.world.rand_f64(),
                        1.0,
                        args.world.rand_f64(),
                    ),
                    Vector3::new(0.0, 0.0, 0.0),
                    1.0,
                    1,
                    pumpkin_data::particle::Particle::Splash,
                );
                args.world.spawn_particle(
                    args.position.to_f64().add_raw(
                        args.world.rand_f64(),
                        1.0,
                        args.world.rand_f64(),
                    ),
                    Vector3::new(0.0, 0.01, 0.0),
                    0.2,
                    1,
                    pumpkin_data::particle::Particle::Bubble,
                );
            }
        } else {
            if let Some(living) = args.entity.get_living_entity() {
                living.fall_distance.store(0.0);
            }
            if let Some(falling) = args
                .entity
                .cast_any()
                .downcast_ref::<crate::entity::falling::FallingEntity>()
            {
                falling.reset_fall_distance();
            }
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block == &Block::WATER
            && is_source_water(args.world, *args.position)
            && kind_from_support(args.world.get_block(&args.position.down())).is_some()
        {
            schedule_reconcile(args.world, args.block, *args.position, CREATE_DELAY_TICKS);
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.block == &Block::WATER
            && is_source_water(args.world, *args.position)
            && kind_from_support(args.world.get_block(&args.position.down())).is_some()
        {
            schedule_reconcile(args.world, args.block, *args.position, CREATE_DELAY_TICKS);
        }
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.block != &Block::BUBBLE_COLUMN || {
            let below = args.block_accessor.get_block(&args.position.down());
            below == &Block::BUBBLE_COLUMN || kind_from_support(below).is_some()
        }
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.block == &Block::BUBBLE_COLUMN {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
            let below = args.world.get_block(&args.position.down());
            if (below != &Block::BUBBLE_COLUMN && kind_from_support(below).is_none())
                || args.direction == pumpkin_data::BlockDirection::Down
                || (args.direction == pumpkin_data::BlockDirection::Up
                    && args.neighbor_state_id.to_block() != &Block::BUBBLE_COLUMN
                    && is_source_water_state(args.neighbor_state_id))
            {
                schedule_reconcile(args.world, args.block, *args.position, REMOVE_DELAY_TICKS);
            }
        } else {
            if World::fluid_state_from_block_state(args.state_id)
                .1
                .is_source
                || World::fluid_state_from_block_state(args.neighbor_state_id)
                    .1
                    .is_source
            {
                args.world.schedule_fluid_tick(
                    &Fluid::WATER,
                    *args.position,
                    Fluid::WATER.flow_speed as u32,
                    TickPriority::Normal,
                );
            }
            if args.direction == pumpkin_data::BlockDirection::Down
                && is_source_water_state(args.state_id)
                && kind_from_support(args.neighbor_state_id.to_block()).is_some()
            {
                schedule_reconcile(args.world, args.block, *args.position, CREATE_DELAY_TICKS);
            }
        }
        args.state_id
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        Self::update_column(args.world, args.position);
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::BlockStateId;
    use pumpkin_data::fluid::{Falling, FlowingWaterLikeFluidProperties, FluidProperties, Level};

    use super::*;

    fn flowing_water_state() -> BlockStateId {
        FlowingWaterLikeFluidProperties {
            r#falling: Falling::False,
            r#level: Level::L1,
        }
        .to_state_id(&Fluid::FLOWING_WATER)
    }

    #[test]
    fn support_tags_map_to_expected_kinds() {
        assert_eq!(
            kind_from_support(&Block::SOUL_SAND),
            Some(BubbleColumnKind::Upward)
        );
        assert_eq!(
            kind_from_support(&Block::MAGMA_BLOCK),
            Some(BubbleColumnKind::Downward)
        );
        assert_eq!(kind_from_support(&Block::STONE), None);
    }

    #[test]
    fn reconciliation_creates_from_upward_support() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Upward)
        );
    }

    #[test]
    fn reconciliation_creates_from_downward_support() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::MAGMA_BLOCK,
                Block::MAGMA_BLOCK.default_state.id,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Downward)
        );
    }

    #[test]
    fn reconciliation_inherits_direction_from_lower_column() {
        let upward_state = bubble_column_state(BubbleColumnKind::Upward);
        let downward_state = bubble_column_state(BubbleColumnKind::Downward);

        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::BUBBLE_COLUMN,
                upward_state
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Upward)
        );
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::BUBBLE_COLUMN,
                downward_state,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Downward)
        );
    }

    #[test]
    fn reconciliation_rejects_flowing_water_and_air() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                flowing_water_state(),
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::Stop
        );
        assert_eq!(
            reconcile_action(
                &Block::AIR,
                BlockStateId::AIR,
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::Stop
        );
    }

    #[test]
    fn reconciliation_restores_invalidated_column() {
        assert_eq!(
            reconcile_action(
                &Block::BUBBLE_COLUMN,
                bubble_column_state(BubbleColumnKind::Upward),
                &Block::STONE,
                Block::STONE.default_state.id,
            ),
            ReconcileAction::RestoreWater
        );
    }
}
