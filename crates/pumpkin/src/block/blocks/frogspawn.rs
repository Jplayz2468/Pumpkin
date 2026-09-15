use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::entity::EntityType;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use rand::{RngExt, rng};
use uuid::Uuid;

use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs,
    OnScheduledTickArgs, PlacedArgs,
};
use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::r#type::from_type;
use crate::world::World;

/// `FrogspawnBlock.DEFAULT_MIN_HATCH_TICK_DELAY` / `DEFAULT_MAX_HATCH_TICK_DELAY`
/// (`FrogspawnBlock.java:34-35`).
const MIN_HATCH_TICK_DELAY: i32 = 3600;
const MAX_HATCH_TICK_DELAY: i32 = 12000;

#[pumpkin_block("minecraft:frogspawn")]
pub struct FrogspawnBlock;

impl FrogspawnBlock {
    /// `FrogspawnBlock#mayPlaceOn` (`FrogspawnBlock.java:107-111`).
    ///
    /// `pos` is the support block (i.e. the position *below* the frogspawn): it must be
    /// water (or otherwise tagged to support frogspawn), and the cell the frogspawn itself
    /// occupies (`pos.up()`) must have no fluid in it.
    fn may_place_on(world: &World, pos: &BlockPos) -> bool {
        let fluid = world.get_fluid(pos);
        let block = world.get_block(pos);
        (fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_FROGSPAWN)
            || block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_FROGSPAWN))
            && *world.get_fluid(&pos.up()) == Fluid::EMPTY
    }

    /// `FrogspawnBlock#getFrogspawnHatchDelay` (`FrogspawnBlock.java:64-66`).
    ///
    fn hatch_delay() -> u32 {
        rng().random_range(MIN_HATCH_TICK_DELAY..MAX_HATCH_TICK_DELAY) as u32
    }

    /// `FrogspawnBlock#hatchFrogspawn` (`FrogspawnBlock.java:113-117`).
    fn hatch_frogspawn(world: &Arc<World>, pos: &BlockPos) {
        world.break_block(pos, None, BlockFlags::SKIP_DROPS);
        world.play_sound(Sound::BlockFrogspawnHatch, SoundCategory::Blocks, &pos.to_f64());
        Self::spawn_tadpoles(world, pos);
    }

    /// `FrogspawnBlock#spawnTadpoles` (`FrogspawnBlock.java:123-137`).
    fn spawn_tadpoles(world: &Arc<World>, pos: &BlockPos) {
        let tadpole_amount = rng().random_range(2..6);

        for _ in 0..tadpole_amount {
            let x_pos = f64::from(pos.0.x) + Self::random_tadpole_position_offset();
            let z_pos = f64::from(pos.0.z) + Self::random_tadpole_position_offset();
            let y_pos = f64::from(pos.0.y) - 0.5;
            let y_rot = rng().random_range(1..361);

            let tadpole = from_type(
                &EntityType::TADPOLE,
                Vector3::new(x_pos, y_pos, z_pos),
                world,
                Uuid::new_v4(),
            );
            tadpole.get_entity().set_rotation(y_rot as f32, 0.0);
            if let Some(mob) = tadpole.get_mob() {
                mob.get_mob_entity()
                    .persistence_required
                    .store(true, Ordering::Relaxed);
            }
            world.spawn_entity(tadpole);
        }
    }

    /// `FrogspawnBlock#getRandomTadpolePositionOffset` (`FrogspawnBlock.java:139-142`).
    fn random_tadpole_position_offset() -> f64 {
        rng().random::<f64>().clamp(0.2, 0.799_999_997_019_767_8)
    }
}

impl BlockBehaviour for FrogspawnBlock {
    /// `FrogspawnBlock#canSurvive` (`FrogspawnBlock.java:54-57`).
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.world
            .is_some_and(|world| Self::may_place_on(world, &args.position.down()))
    }

    /// `FrogspawnBlock#onPlace` (`FrogspawnBlock.java:59-62`): schedules the hatch tick.
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.schedule_block_tick(
            args.block,
            *args.position,
            Self::hatch_delay(),
            TickPriority::Normal,
        );
    }

    /// `FrogspawnBlock#updateShape` (`FrogspawnBlock.java:68-82`): turns to air immediately
    /// once its support is gone, instead of scheduling a break tick.
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !Self::may_place_on(args.world, &args.position.down()) {
            return Block::AIR.default_state.id;
        }
        args.state_id
    }

    /// `FrogspawnBlock#tick` (`FrogspawnBlock.java:84-91`).
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !Self::may_place_on(args.world, &args.position.down()) {
            args.world
                .break_block(args.position, None, BlockFlags::SKIP_DROPS);
        } else {
            Self::hatch_frogspawn(args.world, args.position);
        }
    }

    /// `FrogspawnBlock#entityInside` (`FrogspawnBlock.java:93-105`): a falling block landing
    /// on frogspawn destroys it.
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if args.entity.get_entity().entity_type.id == EntityType::FALLING_BLOCK.id {
            args.world
                .break_block(args.position, None, BlockFlags::SKIP_DROPS);
        }
    }
}
