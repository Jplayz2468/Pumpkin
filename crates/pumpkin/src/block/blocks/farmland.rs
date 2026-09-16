use std::sync::Arc;

use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnLandedUponArgs, OnPlaceArgs,
    OnScheduledTickArgs, PathComputationType, RandomTickArgs,
};
use crate::world::World;
use pumpkin_data::block_properties::FarmlandLikeProperties;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;
use pumpkin_world::world::BlockFlags;

type FarmlandProperties = FarmlandLikeProperties;

#[pumpkin_block("minecraft:farmland")]
pub struct FarmlandBlock;

impl BlockBehaviour for FarmlandBlock {
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_place_at(args.world.as_ref(), args.position) {
            turn_to_dirt(args.world, args.position, None);
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            return Block::DIRT.default_state.id;
        }
        args.block.default_state.id
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == BlockDirection::Up && !can_place_at(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }
        args.state_id
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let mut props =
            FarmlandProperties::from_state_id(args.world.get_block_state_id(args.position));
        let wet = is_water_nearby(args.world, args.position)
            || args.world.is_raining_at(&args.position.up());
        let mut new_moisture = if wet {
            if props.moisture == 7 {
                return;
            }
            7
        } else if props.moisture > 0 {
            i32::from(props.moisture) - 1
        } else {
            if !args
                .world
                .get_block(&args.position.up())
                .has_tag(&tag::Block::MINECRAFT_MAINTAINS_FARMLAND)
            {
                turn_to_dirt(args.world, args.position, None);
            }
            return;
        };
        if let Some(server) = args.world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::block::moisture_change::MoistureChangeEvent::new(
                    *args.position,
                    args.world.clone(),
                    new_moisture,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            new_moisture = event.new_moisture;
        }
        props.moisture = new_moisture.clamp(0, 7) as u8;
        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }

    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        let entity = args.entity.get_entity();
        // Keep the source random draw before the living-entity and griefing gates.
        if f64::from(args.world.rand_f32()) < f64::from(args.fall_distance) - 0.5
            && args.entity.get_living_entity().is_some()
            && (args.entity.get_player().is_some()
                || args.world.level_info.load().game_rules.mob_griefing)
            && entity.width() * entity.width() * entity.height() > 0.512_f32
        {
            turn_to_dirt(args.world, args.position, Some(args.entity));
        }
        args.entity.cause_fall_damage(
            args.entity,
            args.fall_distance,
            1.0,
            pumpkin_data::damage::DamageType::FALL,
        );
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    let state = world.get_block_state(&block_pos.up());
    !state.is_solid()
        || state
            .id
            .to_block()
            .has_tag(&tag::Block::MINECRAFT_MAINTAINS_FARMLAND)
}

fn is_water_nearby(world: &Arc<World>, block_pos: &BlockPos) -> bool {
    for dz in -4..=4 {
        for dy in 0..=1 {
            for dx in -4..=4 {
                let check_pos = block_pos.offset(Vector3 {
                    x: dx,
                    y: dy,
                    z: dz,
                });
                if world
                    .get_fluid(&check_pos)
                    .matches_type(&pumpkin_data::fluid::Fluid::WATER)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// FarmlandBlock.turnToDirt, also used by dirt paths.
pub(super) fn turn_to_dirt(
    world: &Arc<World>,
    pos: &BlockPos,
    source: Option<&dyn crate::entity::EntityBase>,
) {
    crate::block::shape::push_entities_up(
        world,
        *pos,
        world.get_block_state(pos),
        Block::DIRT.default_state,
    );
    world.set_block_state(pos, Block::DIRT.default_state.id, BlockFlags::NOTIFY_ALL);
    world.emit_game_event_from_entity(
        "block_change",
        pos.to_centered_f64(),
        source,
        Some(Block::DIRT.default_state.id),
    );
}
