use pumpkin_data::block_properties::{DriedGhastLikeProperties, HorizontalFacing};
use pumpkin_data::entity::EntityType;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use uuid::Uuid;

use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs,
    PathComputationType, PlayerPlacedArgs, RandomTickArgs,
};
use crate::entity::EntityBase;
use crate::entity::ageable::AgeableMob;
use crate::entity::mob::Mob;
use crate::entity::r#type::from_type;
use crate::world::World;
use pumpkin_util::math::vector3::Vector3;

/// `DriedGhastBlock.MAX_HYDRATION_LEVEL` (`DriedGhastBlock.java:40`).
const MAX_HYDRATION_LEVEL: u8 = 3;

/// `DriedGhastBlock.HYDRATION_TICK_DELAY` (`DriedGhastBlock.java:43`).
const HYDRATION_TICK_DELAY: u32 = 5000;

/// Pure result of one hydration tick (`DriedGhastBlock.tick`/`tickWaterlogged`,
/// `DriedGhastBlock.java:92-113`), independent of world side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HydrationStep {
    /// Not waterlogged and already fully dried out: vanilla's `tick` does nothing.
    NoOp,
    /// Not waterlogged: hydration drops by one (`DriedGhastBlock.java:97-101`).
    DryOut(u8),
    /// Waterlogged and below max: hydration rises by one (`DriedGhastBlock.java:106-109`).
    Hydrate(u8),
    /// Waterlogged and already at `MAX_HYDRATION_LEVEL`: spawn the ghastling
    /// (`DriedGhastBlock.java:110-111`).
    SpawnGhastling,
}

#[pumpkin_block("minecraft:dried_ghast")]
pub struct DriedGhastBlock;

impl DriedGhastBlock {
    /// The real production transition table for a scheduled hydration tick. Called from
    /// `on_scheduled_tick`; also exercised directly by the test below so the test pins this
    /// function rather than a re-derived copy of it.
    fn next_hydration_step(waterlogged: bool, hydration: u8) -> HydrationStep {
        if waterlogged {
            if hydration < MAX_HYDRATION_LEVEL {
                HydrationStep::Hydrate(hydration + 1)
            } else {
                HydrationStep::SpawnGhastling
            }
        } else if hydration > 0 {
            HydrationStep::DryOut(hydration - 1)
        } else {
            HydrationStep::NoOp
        }
    }

    /// `Direction.getYRot` (`Direction.java:134`): the yaw a ghastling should face when it
    /// spawns out of a block with this `FACING`.
    fn facing_to_y_rot(facing: HorizontalFacing) -> f32 {
        match facing {
            HorizontalFacing::North => 180.0,
            HorizontalFacing::South => 0.0,
            HorizontalFacing::West => 90.0,
            HorizontalFacing::East => -90.0,
        }
    }

    /// `DriedGhastBlock.spawnGhastling` (`DriedGhastBlock.java:115-127`).
    fn spawn_ghastling(&self, args: &OnScheduledTickArgs<'_>, facing: HorizontalFacing) {
        // Vanilla's `Level.removeBlock` leaves behind whatever fluid was in the space; this
        // branch only runs while the block is waterlogged, so that fluid is always water.
        args.world.set_block_state(
            args.position,
            Block::WATER.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );

        let spawn_at = args.position.to_f64().add(&Vector3::new(0.5, 0.0, 0.5));
        let baby = from_type(
            &EntityType::HAPPY_GHAST,
            spawn_at,
            args.world,
            Uuid::new_v4(),
        );
        if let Some(ageable) = baby.get_mob().and_then(Mob::as_ageable) {
            ageable.set_baby(true);
        }
        let y_rot = Self::facing_to_y_rot(facing);
        let entity = baby.get_entity();
        entity.yaw.store(y_rot);
        entity.head_yaw.store(y_rot);
        entity.pitch.store(0.0);

        args.world.spawn_entity(baby);
        args.world.play_sound(
            Sound::EntityGhastlingSpawn,
            SoundCategory::Blocks,
            &spawn_at,
        );
    }
}

impl BlockBehaviour for DriedGhastBlock {
    /// `DriedGhastBlock.getStateForPlacement` (`DriedGhastBlock.java:169-173`).
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = DriedGhastLikeProperties::default(args.block);
        let (fluid, fluid_state) =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position));
        props.waterlogged = fluid_state.is_source && fluid.matches_type(&Fluid::WATER);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    /// Vanilla's placement sound belongs to setPlacedBy, not onPlace.
    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        let props = DriedGhastLikeProperties::from_state_id(args.state_id);
        let sound = if props.waterlogged {
            Sound::BlockDriedGhastPlaceInWater
        } else {
            Sound::BlockDriedGhastPlace
        };
        args.world.play_sound(
            sound,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
        );
    }

    /// `DriedGhastBlock.updateShape` (`DriedGhastBlock.java:61-77`): only the waterlogged
    /// fluid-tick scheduling matters here, `HorizontalDirectionalBlock.updateShape` is a no-op.
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = DriedGhastLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }
        args.state_id
    }

    /// `DriedGhastBlock.randomTick` (`DriedGhastBlock.java:162-166`).
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let props =
            DriedGhastLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if (props.waterlogged || props.hydration > 0)
            && !args
                .world
                .is_block_tick_scheduled(args.position, args.block)
        {
            args.world.schedule_block_tick(
                args.block,
                *args.position,
                HYDRATION_TICK_DELAY,
                TickPriority::Normal,
            );
        }
    }

    /// `DriedGhastBlock.tick`/`tickWaterlogged` (`DriedGhastBlock.java:92-113`).
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let props = DriedGhastLikeProperties::from_state_id(state_id);

        match Self::next_hydration_step(props.waterlogged, props.hydration) {
            HydrationStep::NoOp => {}
            HydrationStep::DryOut(new_hydration) => {
                let mut new_props = props;
                new_props.hydration = new_hydration;
                args.world.set_block_state(
                    args.position,
                    new_props.to_state_id(args.block),
                    BlockFlags::NOTIFY_LISTENERS,
                );
                args.world.emit_game_event_from_entity(
                    GameEvent::BlockChange.name(),
                    args.position.to_centered_f64(),
                    None,
                    Some(state_id),
                );
            }
            HydrationStep::Hydrate(new_hydration) => {
                args.world.play_sound(
                    Sound::BlockDriedGhastTransition,
                    SoundCategory::Blocks,
                    &args.position.to_centered_f64(),
                );
                let mut new_props = props;
                new_props.hydration = new_hydration;
                args.world.set_block_state(
                    args.position,
                    new_props.to_state_id(args.block),
                    BlockFlags::NOTIFY_LISTENERS,
                );
                args.world.emit_game_event_from_entity(
                    GameEvent::BlockChange.name(),
                    args.position.to_centered_f64(),
                    None,
                    Some(state_id),
                );
            }
            HydrationStep::SpawnGhastling => {
                self.spawn_ghastling(&args, props.facing);
            }
        }
    }

    /// `DriedGhastBlock.isPathfindable` (`DriedGhastBlock.java:203-206`).
    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

#[cfg(test)]
mod dried_ghast_tests {
    use super::*;

    #[test]
    fn not_waterlogged_and_dry_is_a_no_op() {
        assert_eq!(
            DriedGhastBlock::next_hydration_step(false, 0),
            HydrationStep::NoOp
        );
    }

    #[test]
    fn drying_out_decrements_by_one_each_tick() {
        assert_eq!(
            DriedGhastBlock::next_hydration_step(false, 3),
            HydrationStep::DryOut(2)
        );
        assert_eq!(
            DriedGhastBlock::next_hydration_step(false, 1),
            HydrationStep::DryOut(0)
        );
    }

    #[test]
    fn waterlogged_hydrates_by_one_each_tick_up_to_max() {
        assert_eq!(
            DriedGhastBlock::next_hydration_step(true, 0),
            HydrationStep::Hydrate(1)
        );
        assert_eq!(
            DriedGhastBlock::next_hydration_step(true, MAX_HYDRATION_LEVEL - 1),
            HydrationStep::Hydrate(MAX_HYDRATION_LEVEL)
        );
    }

    #[test]
    fn waterlogged_at_max_hydration_spawns_the_ghastling_instead_of_overflowing() {
        assert_eq!(
            DriedGhastBlock::next_hydration_step(true, MAX_HYDRATION_LEVEL),
            HydrationStep::SpawnGhastling
        );
    }

    #[test]
    fn facing_to_y_rot_matches_vanilla_direction_get_y_rot() {
        assert_eq!(
            DriedGhastBlock::facing_to_y_rot(HorizontalFacing::South),
            0.0
        );
        assert_eq!(
            DriedGhastBlock::facing_to_y_rot(HorizontalFacing::West),
            90.0
        );
        assert_eq!(
            DriedGhastBlock::facing_to_y_rot(HorizontalFacing::North),
            180.0
        );
        assert_eq!(
            DriedGhastBlock::facing_to_y_rot(HorizontalFacing::East),
            -90.0
        );
    }
}
