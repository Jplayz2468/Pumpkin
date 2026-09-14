use super::shrieker_rules::{self, SHRIEK_TICKS};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::{gamemode::GameMode, random::RandomImpl};
use std::sync::Arc;

use crate::block::entities::sculk_shrieker::SculkShriekerBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, OnEntityStepArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::entity::{
    EntityBase,
    player::{Player, warden_spawn_tracker::WardenSpawnTracker},
};
use crate::world::World;
use pumpkin_data::potion::Effect;
use pumpkin_data::{
    BlockId, BlockStateId, block_properties::SculkShriekerLikeProperties, effect::StatusEffect,
};
use pumpkin_data::{entity::EntityType, world::WorldEvent};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::{
    difficulty::Difficulty,
    math::{boundingbox::BoundingBox, vector3::Vector3},
};
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use std::sync::atomic::Ordering;

const DARKNESS_RADIUS: f64 = 40.0;
const DARKNESS_DURATION: i32 = 260;

pub struct SculkShriekerBlock;

impl BlockMetadata for SculkShriekerBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_SHRIEKER].into()
    }
}

impl SculkShriekerBlock {
    fn can_respond(world: &World, can_summon: bool) -> bool {
        let info = world.level_info.load();
        shrieker_rules::can_respond(
            can_summon,
            info.difficulty == Difficulty::Peaceful,
            info.game_rules.spawn_wardens,
        )
    }

    fn try_warn(world: &World, pos: &BlockPos, trigger: &Player) -> Option<i32> {
        let center = pos.to_centered_f64();
        let radius = Vector3::new(24.0, 24.0, 24.0);
        if world
            .get_entities_at_box(&BoundingBox::new(center - radius, center + radius))
            .iter()
            .any(|e| e.get_entity().entity_type == &EntityType::WARDEN)
        {
            return None;
        }
        let players = world.players.load();
        let mut nearby: Vec<&Player> = players
            .iter()
            .filter_map(|player| {
                let delta = player.get_entity().pos.load() - center;
                let alive = player.get_entity().is_alive()
                    && player.living_entity.health.load() > 0.0
                    && !player.living_entity.dead.load(Ordering::Relaxed);
                WardenSpawnTracker::eligible_nearby_player(
                    [delta.x, delta.y, delta.z],
                    alive,
                    player.is_spectator(),
                )
                .then_some(player.as_ref())
            })
            .collect();
        if !nearby
            .iter()
            .any(|p| p.gameprofile.id == trigger.gameprofile.id)
        {
            nearby.push(trigger);
        }
        // Overlapping shriekers acquire player locks in a stable order. Keep all
        // locks until the shared state is committed, with no world callbacks held.
        nearby.sort_unstable_by_key(|p| p.gameprofile.id);
        let mut guards: Vec<_> = nearby
            .iter()
            .map(|p| {
                p.warden_spawn_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect();
        let mut states: Vec<_> = guards.iter().map(|state| **state).collect();
        let warning = WardenSpawnTracker::try_warn_group(&mut states, false)?;
        for (guard, state) in guards.iter_mut().zip(states) {
            **guard = state;
        }
        Some(warning)
    }

    pub fn try_activate(world: &Arc<World>, pos: &BlockPos, player: &Player) -> bool {
        let block = world.get_block(pos);
        if block.id != BlockId::SCULK_SHRIEKER {
            return false;
        }
        let mut props = SculkShriekerLikeProperties::from_state_id(world.get_block_state(pos).id);
        if props.shrieking {
            return false;
        }
        let Some(entity) = world.get_block_entity(pos) else {
            return false;
        };
        let Some(shrieker) = entity.as_any().downcast_ref::<SculkShriekerBlockEntity>() else {
            return false;
        };
        {
            let mut warning = shrieker
                .warning_level
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !shrieker_rules::begin_shriek(
                true,
                props.shrieking,
                Self::can_respond(world, props.can_summon),
                &mut warning,
                || Self::try_warn(world, pos, player),
            ) {
                return false;
            }
        }
        props.shrieking = true;
        world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
        world.schedule_block_tick(block, *pos, SHRIEK_TICKS, TickPriority::Normal);
        world.sync_world_event(WorldEvent::ParticlesSculkShriek, *pos, 0);
        // The shared vibration listener/attributed game-event pipeline is D5.
        world.emit_game_event("minecraft:shriek", pos.to_centered_f64());
        true
    }

    fn respond(world: &Arc<World>, pos: &BlockPos, can_summon: bool) {
        let Some(entity) = world.get_block_entity(pos) else {
            return;
        };
        let Some(shrieker) = entity.as_any().downcast_ref::<SculkShriekerBlockEntity>() else {
            return;
        };
        let warning = *shrieker
            .warning_level
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::respond_with_warning(world, pos, can_summon, warning);
    }

    pub(crate) fn respond_with_warning(
        world: &Arc<World>,
        pos: &BlockPos,
        can_summon: bool,
        warning: i32,
    ) {
        if !Self::can_respond(world, can_summon) || warning <= 0 {
            return;
        }
        let spawned = warning >= 4 && crate::world::warden_spawn::try_spawn(world, pos);
        let sound = match warning {
            1 => Some(Sound::EntityWardenNearbyClose),
            2 => Some(Sound::EntityWardenNearbyCloser),
            3 => Some(Sound::EntityWardenNearbyClosest),
            4 => Some(Sound::EntityWardenListeningAngry),
            _ => None,
        };
        if let Some(sound) = sound.filter(|_| !spawned) {
            let offset = {
                let mut random = world
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Vector3::new(
                    random.next_bounded_i32(21) - 10,
                    random.next_bounded_i32(21) - 10,
                    random.next_bounded_i32(21) - 10,
                )
            };
            let sound_pos = Vector3::new(
                f64::from(pos.0.x + offset.x),
                f64::from(pos.0.y + offset.y),
                f64::from(pos.0.z + offset.z),
            );
            world.play_sound_fine(sound, SoundCategory::Hostile, &sound_pos, 5.0, 1.0);
        }
        let center = pos.to_centered_f64();
        for player in world.get_nearby_players(center, DARKNESS_RADIUS) {
            let previous = player.living_entity.get_effect(&StatusEffect::DARKNESS);
            if !shrieker_rules::should_apply_darkness(
                matches!(
                    player.gamemode.load(),
                    GameMode::Survival | GameMode::Adventure
                ),
                player
                    .get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&center),
                previous.map(|effect| (i32::from(effect.amplifier), effect.duration)),
            ) {
                continue;
            }
            let darkness = Effect {
                effect_type: &StatusEffect::DARKNESS,
                duration: DARKNESS_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: false,
                show_icon: false,
                blend: true,
            };
            player.living_entity.add_effect(darkness);
        }
    }
}

impl BlockBehaviour for SculkShriekerBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        if args.world.get_block_entity(args.position).is_none() {
            args.world
                .add_block_entity(Arc::new(SculkShriekerBlockEntity::new(*args.position)));
        }
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        if let Some(player) = args.entity.get_player() {
            Self::try_activate(args.world, args.position, player);
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SculkShriekerLikeProperties::default(args.block);
        props.shrieking = false;
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = SculkShriekerLikeProperties::from_state_id(state.id);
        if props.shrieking {
            props.shrieking = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            Self::respond(args.world, args.position, props.can_summon);
        }
    }
}
