//! Bee-specific goals, following Bee.java. Shared pathfinding remains responsible
//! for reachability; a failed flower path is cached for 600 game ticks.
use super::BeeEntity;
use crate::{
    block::entities::beehive::BeehiveBlockEntity,
    entity::{
        EntityBase,
        ai::{
            goal::{
                Controls, Goal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
                track_target::TrackTargetGoal,
            },
            pathfinder::path::Path,
            target_predicate::TargetPredicate,
        },
        mob::Mob,
    },
};
use pumpkin_data::{
    Block, BlockState,
    attributes::Attributes,
    sound::Sound,
    tag::{self, Taggable},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;
use rand::RngExt;
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering::Relaxed},
};

fn bee(mob: &dyn Mob) -> &BeeEntity {
    mob.cast_any()
        .downcast_ref::<BeeEntity>()
        .expect("bee goal")
}
fn calm(mob: &dyn Mob) -> bool {
    !bee(mob).is_angry()
}
fn stop(mob: &dyn Mob) {
    mob.get_mob_entity()
        .navigator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .stop();
}
fn navigate(mob: &dyn Mob, pos: Vector3<f64>, speed: f64, reach: i32) -> bool {
    let mut nav = mob
        .get_mob_entity()
        .navigator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let living = &mob.get_mob_entity().living_entity;
    let path = nav.create_path(living, pos, reach);
    let reached = path.as_ref().is_some_and(Path::can_reach);
    nav.move_to_path(path, speed, living);
    reached
}
fn idle(mob: &dyn Mob) -> bool {
    !mob.get_mob_entity()
        .navigator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_in_progress()
}
pub(super) fn attracts(state: &BlockState) -> bool {
    let block = state.id.to_block();
    if !block.has_tag(&tag::Block::MINECRAFT_BEE_ATTRACTIVE) {
        return false;
    }
    let props = block
        .properties(state.id)
        .map(|p| p.to_props())
        .unwrap_or_default();
    !props.contains(&("waterlogged", "true"))
        && (block != &Block::SUNFLOWER || props.contains(&("half", "upper")))
}

pub struct Attack(MeleeAttackGoal);
impl Attack {
    pub fn new() -> Self {
        Self(MeleeAttackGoal::new(1.4, true))
    }
}
impl Goal for Attack {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.0.can_start(mob) && bee(mob).can_use_melee_attack()
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.0.should_continue(mob) && bee(mob).can_use_melee_attack()
    }
    fn start(&mut self, mob: &dyn Mob) {
        self.0.start(mob);
    }
    fn stop(&mut self, mob: &dyn Mob) {
        self.0.stop(mob);
    }
    fn tick(&mut self, mob: &dyn Mob) {
        self.0.tick(mob);
    }
    fn should_run_every_tick(&self) -> bool {
        true
    }
    fn controls(&self) -> Controls {
        self.0.controls()
    }
}

pub struct Revenge(RevengeGoal);
impl Revenge {
    pub fn new() -> Self {
        Self(RevengeGoal::new(true))
    }
}
impl Goal for Revenge {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.0.can_start(mob)
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        bee(mob).is_angry() && self.0.should_continue(mob)
    }
    fn start(&mut self, mob: &dyn Mob) {
        self.0.start(mob);
        let Some(target) = mob.get_mob_entity().get_target() else {
            return;
        };
        let entity = mob.get_entity();
        let world = entity.world.load_full();
        if world.has_line_of_sight(entity.get_eye_pos(), target.get_entity().get_eye_pos()) {
            let range = mob
                .get_mob_entity()
                .living_entity
                .get_attribute_value(&Attributes::FOLLOW_RANGE);
            let bounds = entity.bounding_box.load().expand(range, 10.0, range);
            for other in world.entities.load().iter() {
                if other.get_entity().entity_id != entity.entity_id
                    && other.get_entity().bounding_box.load().intersects(&bounds)
                    && let Some(other) = other.cast_any().downcast_ref::<BeeEntity>()
                    && other.mob_entity.get_target().is_none()
                {
                    other.set_mob_target(Some(target.clone()));
                }
            }
        }
        bee(mob).update_anger();
    }
    fn stop(&mut self, mob: &dyn Mob) {
        self.0.stop(mob);
    }
    fn controls(&self) -> Controls {
        self.0.controls()
    }
}

pub struct AngryTarget {
    track: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
}
impl AngryTarget {
    pub fn new() -> Self {
        Self {
            track: TrackTargetGoal::with_default(true),
            target: None,
        }
    }
}
impl Goal for AngryTarget {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        if !bee.can_use_melee_attack() || mob.get_random().random_range(0..5) != 0 {
            return false;
        }
        let world = mob.get_entity().world.load_full();
        let living = &mob.get_mob_entity().living_entity;
        let range = living.get_attribute_value(&Attributes::FOLLOW_RANGE);
        let predicate = TargetPredicate::create_attackable().set_base_max_distance(range);
        self.target = world
            .players
            .load()
            .iter()
            .filter(|p| {
                bee.is_angry_at(p.get_entity().entity_uuid)
                    && p.get_living_entity()
                        .is_some_and(|l| predicate.test(&world, Some(living), l))
            })
            .min_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&mob.get_entity().pos.load())
                    .total_cmp(
                        &b.get_entity()
                            .pos
                            .load()
                            .squared_distance_to_vec(&mob.get_entity().pos.load()),
                    )
            })
            .map(|p| p.clone() as Arc<dyn EntityBase>);
        self.target.is_some()
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        bee(mob).can_use_melee_attack() && self.track.should_continue(mob)
    }
    fn start(&mut self, mob: &dyn Mob) {
        mob.set_mob_target(self.target.clone());
        self.track.start(mob);
    }
    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        self.track.stop(mob);
    }
    fn controls(&self) -> Controls {
        self.track.controls()
    }
}

pub struct EnterHive;
impl Goal for EnterHive {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        if !calm(mob) || !bee.wants_to_enter_hive() {
            return false;
        }
        let Some(pos) = bee.hive_pos.load() else {
            return false;
        };
        if pos
            .to_centered_f64()
            .squared_distance_to_vec(&mob.get_entity().pos.load())
            >= 4.0
        {
            return false;
        }
        let world = mob.get_entity().world.load_full();
        let Some(be) = world.get_block_entity(&pos) else {
            return false;
        };
        let Some(hive) = be.as_any().downcast_ref::<BeehiveBlockEntity>() else {
            return false;
        };
        if hive.is_full() {
            bee.hive_pos.store(None);
            false
        } else {
            true
        }
    }
    fn start(&mut self, mob: &dyn Mob) {
        if let Some(pos) = bee(mob).hive_pos.load()
            && let Some(be) = mob.get_entity().world.load().get_block_entity(&pos)
            && let Some(hive) = be.as_any().downcast_ref::<BeehiveBlockEntity>()
        {
            hive.add_occupant(bee(mob));
        }
    }
}

pub struct Validate {
    hive: bool,
    cooldown: i64,
    last: i64,
}
impl Validate {
    pub fn new(hive: bool) -> Self {
        Self {
            hive,
            cooldown: rand::random_range(20..=40),
            last: -1,
        }
    }
}
impl Goal for Validate {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        calm(mob) && mob.get_entity().world.load().get_world_age() > self.last + self.cooldown
    }
    fn start(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        let world = mob.get_entity().world.load_full();
        if self.hive {
            if let Some(pos) = bee.hive_pos.load()
                && world.level.is_chunk_loaded(&pos.chunk_position())
                && !bee.hive_valid()
            {
                bee.drop_hive();
            }
        } else if let Some(pos) = bee.flower_pos.load()
            && world.level.is_chunk_loaded(&pos.chunk_position())
            && !attracts(world.get_block_state(&pos))
        {
            bee.drop_flower();
        }
        self.last = world.get_world_age();
    }
}

#[derive(Default)]
pub struct Pollinate {
    successful: i32,
    elapsed: i32,
    last_sound: i32,
    hover: Option<Vector3<f64>>,
    unreachable: HashMap<BlockPos, i64>,
}
impl Goal for Pollinate {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        if !calm(mob)
            || bee.flower_cooldown.load(Relaxed) > 0
            || bee.has_nectar()
            || bee.is_raining()
        {
            return false;
        }
        let world = mob.get_entity().world.load_full();
        let time = world.get_world_age();
        let mut cache = HashMap::new();
        for pos in BlockPos::iterate_outwards(mob.get_entity().block_pos.load(), 5, 5, 5) {
            if let Some(until) = self.unreachable.get(&pos)
                && time < *until
            {
                cache.insert(pos, *until);
                continue;
            }
            if attracts(world.get_block_state(&pos)) {
                let reachable = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .create_path(&mob.get_mob_entity().living_entity, pos.to_f64(), 1)
                    .is_some_and(|p| p.can_reach());
                if reachable {
                    bee.flower_pos.store(Some(pos));
                    navigate(mob, pos.to_centered_f64(), 1.2_f32 as f64, 1);
                    return true;
                }
                cache.insert(pos, time + 600);
            }
        }
        self.unreachable = cache;
        bee.flower_cooldown
            .store(mob.get_random().random_range(20..=60), Relaxed);
        false
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        calm(mob)
            && bee.pollinating.load(Relaxed)
            && bee.flower_pos.load().is_some()
            && !bee.is_raining()
            && (self.successful <= 400 || mob.get_random().random::<f32>() < 0.2)
    }
    fn start(&mut self, mob: &dyn Mob) {
        self.successful = 0;
        self.elapsed = 0;
        self.last_sound = 0;
        bee(mob).pollinating.store(true, Relaxed);
        bee(mob).ticks_without_nectar.store(0, Relaxed);
    }
    fn stop(&mut self, mob: &dyn Mob) {
        if self.successful > 400 {
            bee(mob).set_has_nectar(true);
        }
        bee(mob).pollinating.store(false, Relaxed);
        stop(mob);
        bee(mob).flower_cooldown.store(200, Relaxed);
    }
    fn tick(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        let Some(pos) = bee.flower_pos.load() else {
            return;
        };
        self.elapsed += 1;
        if self.elapsed > 600 {
            bee.drop_flower();
            bee.pollinating.store(false, Relaxed);
            bee.flower_cooldown.store(200, Relaxed);
            return;
        }
        let flower = pos.to_centered_f64() + Vector3::new(0.0, 0.6_f32 as f64 - 0.5, 0.0);
        let current = mob.get_entity().pos.load();
        let mut wanted = true;
        if current.squared_distance_to_vec(&flower) > 1.0 {
            self.hover = Some(flower);
        } else {
            let hover = *self.hover.get_or_insert(flower);
            if current.squared_distance_to_vec(&hover) <= 0.01 {
                if mob.get_random().random_range(0..25) == 0 {
                    let mut rng = mob.get_random();
                    self.hover = Some(
                        flower
                            + Vector3::new(
                                ((rng.random::<f32>() * 2.0 - 1.0) * 0.33333334) as f64,
                                0.0,
                                ((rng.random::<f32>() * 2.0 - 1.0) * 0.33333334) as f64,
                            ),
                    );
                    stop(mob);
                } else {
                    wanted = false;
                }
                mob.get_mob_entity()
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at_position(mob, flower);
            }
            self.successful += 1;
            if mob.get_random().random::<f32>() < 0.05 && self.successful > self.last_sound + 60 {
                self.last_sound = self.successful;
                mob.get_entity().play_sound(Sound::EntityBeePollinate);
            }
        }
        if wanted && let Some(pos) = self.hover {
            mob.get_mob_entity()
                .move_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_wanted_position(pos.x, pos.y, pos.z, 0.35_f32 as f64);
        }
    }
    fn should_run_every_tick(&self) -> bool {
        true
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

pub struct LocateHive;
impl Goal for LocateHive {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        calm(mob)
            && bee(mob).hive_pos.load().is_none()
            && bee(mob).hive_cooldown.load(Relaxed) == 0
            && bee(mob).wants_to_enter_hive()
    }
    fn start(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        if calm(mob)
            && bee.hive_pos.load().is_none()
            && bee.hive_cooldown.load(Relaxed) == 0
            && bee.wants_to_enter_hive()
        {
            bee.hive_cooldown.store(200, Relaxed);
            let world = mob.get_entity().world.load_full();
            let origin = mob.get_entity().block_pos.load();
            let mut candidates = Vec::new();
            for chunk in world.block_entities.iter() {
                for (pos, be) in chunk.value() {
                    if origin.squared_distance(pos) <= 400
                        && be
                            .as_any()
                            .downcast_ref::<BeehiveBlockEntity>()
                            .is_some_and(|h| !h.is_full())
                    {
                        candidates.push(*pos);
                    }
                }
            }
            candidates.sort_by_key(|pos| origin.squared_distance(pos));
            if let Some(first) = candidates.first().copied() {
                let chosen = candidates
                    .into_iter()
                    .find(|pos| {
                        !bee.blacklisted_hives
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .contains(pos)
                    })
                    .unwrap_or_else(|| {
                        bee.blacklisted_hives
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clear();
                        first
                    });
                bee.hive_pos.store(Some(chosen));
            }
        }
    }
}

#[derive(Default)]
pub struct ReturnToHive {
    travelling: i32,
    stuck: i32,
    last_path: Option<Path>,
}
impl ReturnToHive {
    fn drop_and_blacklist(&mut self, bee: &BeeEntity) {
        if let Some(pos) = bee.hive_pos.load() {
            let mut blacklist = bee
                .blacklisted_hives
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            blacklist.push(pos);
            if blacklist.len() > 3 {
                blacklist.remove(0);
            }
        }
        bee.drop_hive();
    }
    fn eligible(&self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        calm(mob)
            && !mob.get_mob_entity().has_position_target()
            && bee.wants_to_enter_hive()
            && bee.hive_pos.load().is_some_and(|pos| {
                bee.close_to(pos, 48)
                    && !bee.close_to(pos, 2)
                    && !mob
                        .get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .get_path()
                        .is_some_and(|p| p.get_target() == pos && p.can_reach() && p.is_done())
                    && mob
                        .get_entity()
                        .world
                        .load()
                        .get_block(&pos)
                        .has_tag(&tag::Block::MINECRAFT_BEEHIVES)
            })
    }
}
impl Goal for ReturnToHive {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.eligible(mob)
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.eligible(mob)
    }
    fn start(&mut self, _mob: &dyn Mob) {
        self.travelling = 0;
        self.stuck = 0;
    }
    fn stop(&mut self, mob: &dyn Mob) {
        self.travelling = 0;
        self.stuck = 0;
        stop(mob);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset_max_visited_nodes_multiplier();
    }
    fn tick(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        let Some(pos) = bee.hive_pos.load() else {
            return;
        };
        self.travelling += 1;
        if self.travelling > self.get_tick_count(2400) {
            self.drop_and_blacklist(bee);
            return;
        }
        if !idle(mob) {
            return;
        }
        if !bee.close_to(pos, 16) {
            if !bee.close_to(pos, 48) {
                bee.drop_hive();
            } else {
                bee.pathfind_towards(pos);
            }
            return;
        }
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_max_visited_nodes_multiplier(10.0);
        if !navigate(
            mob,
            pos.to_f64(),
            1.0,
            if bee.close_to(pos, 3) { 1 } else { 2 },
        ) {
            self.drop_and_blacklist(bee);
            return;
        }
        let nav = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(path) = nav.get_path() {
            if self
                .last_path
                .as_ref()
                .is_some_and(|last| path.same_as_path(last))
            {
                self.stuck += 1;
                if self.stuck > 60 {
                    bee.drop_hive();
                    self.stuck = 0;
                }
            } else {
                self.last_path = Some(path.clone());
            }
        }
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[derive(Default)]
pub struct KnownFlower {
    travelling: i32,
}
impl Goal for KnownFlower {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.should_continue(mob)
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        calm(mob)
            && !mob.get_mob_entity().has_position_target()
            && bee.ticks_without_nectar.load(Relaxed) > 600
            && bee
                .flower_pos
                .load()
                .is_some_and(|pos| !bee.close_to(pos, 2))
    }
    fn start(&mut self, _mob: &dyn Mob) {
        self.travelling = 0;
    }
    fn stop(&mut self, mob: &dyn Mob) {
        self.travelling = 0;
        stop(mob);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset_max_visited_nodes_multiplier();
    }
    fn tick(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        let Some(pos) = bee.flower_pos.load() else {
            return;
        };
        self.travelling += 1;
        if self.travelling > self.get_tick_count(2400) {
            bee.drop_flower();
        } else if idle(mob) {
            if !bee.close_to(pos, 48) {
                bee.drop_flower();
            } else {
                bee.pathfind_towards(pos);
            }
        }
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

pub struct GrowCrops;
impl Goal for GrowCrops {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.should_continue(mob)
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let bee = bee(mob);
        calm(mob)
            && bee.crops_grown_since_pollination.load(Relaxed) < 10
            && mob.get_random().random::<f32>() >= 0.3
            && bee.has_nectar()
            && bee.hive_valid()
    }
    fn tick(&mut self, mob: &dyn Mob) {
        if mob.get_random().random_range(0..self.get_tick_count(30)) != 0 {
            return;
        }
        let world = mob.get_entity().world.load_full();
        for below in 1..=2 {
            let pos = mob
                .get_entity()
                .block_pos
                .load()
                .offset(Vector3::new(0, -below, 0));
            let state = world.get_block_state(&pos);
            let block = state.id.to_block();
            if !block.has_tag(&tag::Block::MINECRAFT_BEE_GROWABLES) {
                continue;
            }
            let Some(properties) = block.properties(state.id) else {
                continue;
            };
            let mut props = properties.to_props();
            let mut changed = false;
            if block == &Block::CAVE_VINES || block == &Block::CAVE_VINES_PLANT {
                if let Some((_, value)) = props.iter_mut().find(|(key, _)| *key == "berries")
                    && *value == "false"
                {
                    *value = "true";
                    changed = true;
                }
            } else {
                let max = if block == &Block::BEETROOTS || block == &Block::SWEET_BERRY_BUSH {
                    3
                } else if block == &Block::TORCHFLOWER_CROP {
                    2
                } else if [
                    &Block::WHEAT,
                    &Block::CARROTS,
                    &Block::POTATOES,
                    &Block::MELON_STEM,
                    &Block::PUMPKIN_STEM,
                ]
                .contains(&block)
                {
                    7
                } else {
                    continue;
                };
                if let Some((_, value)) = props.iter_mut().find(|(key, _)| *key == "age")
                    && let Ok(age) = value.parse::<usize>()
                    && age < max
                {
                    *value = ["0", "1", "2", "3", "4", "5", "6", "7"][age + 1];
                    changed = true;
                }
            }
            if changed {
                let grown = if block == &Block::TORCHFLOWER_CROP && props.contains(&("age", "2")) {
                    Block::TORCHFLOWER.default_state.id
                } else {
                    block.from_properties(&props).to_state_id(block)
                };
                world.sync_world_event(
                    pumpkin_data::world_event::WorldEvent::ParticlesBeeGrowth,
                    pos,
                    15,
                );
                world.set_block_state(&pos, grown, BlockFlags::NOTIFY_ALL);
                bee(mob).crops_grown_since_pollination.fetch_add(1, Relaxed);
            }
        }
    }
}

pub struct Wander;
impl Goal for Wander {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        idle(mob) && mob.get_random().random_range(0..10) == 0
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        !idle(mob)
    }
    fn start(&mut self, mob: &dyn Mob) {
        let bee = bee(mob);
        let direction = if let Some(pos) = bee.hive_pos.load()
            && bee.hive_valid()
            && !bee.close_to(pos, 24)
        {
            (pos.to_centered_f64() - mob.get_entity().pos.load()).normalize()
        } else {
            let yaw = mob.get_entity().yaw.load().to_radians();
            Vector3::new(-f64::from(yaw.sin()), 0.0, f64::from(yaw.cos()))
        };
        if let Some(pos) = bee
            .random_air_position(8, 7, 0, direction, std::f64::consts::FRAC_PI_2, true)
            .or_else(|| {
                bee.random_air_position(8, 4, -2, direction, std::f64::consts::FRAC_PI_2, false)
            })
        {
            navigate(mob, pos, 1.0, 1);
        }
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[derive(Default)]
pub struct ResetAnger {
    last_hurt: i32,
}
impl Goal for ResetAnger {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let living = &mob.get_mob_entity().living_entity;
        let world = mob.get_entity().world.load_full();
        world.level_info.load().game_rules.universal_anger
            && living.last_attacked_time.load(Relaxed) > self.last_hurt
            && world
                .get_entity_by_id(living.last_attacker_id.load(Relaxed))
                .is_some_and(|e| e.get_player().is_some())
    }
    fn start(&mut self, mob: &dyn Mob) {
        self.last_hurt = mob
            .get_mob_entity()
            .living_entity
            .last_attacked_time
            .load(Relaxed);
        let bee = bee(mob);
        bee.stop_being_angry();
        let world = mob.get_entity().world.load_full();
        bee.set_anger_end_time(world.get_world_age() + mob.get_random().random_range(400..=780));
        let range = mob
            .get_mob_entity()
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        let p = mob.get_entity().pos.load();
        let bounds =
            pumpkin_util::math::boundingbox::BoundingBox::new(p, p + Vector3::new(1.0, 1.0, 1.0))
                .expand(range, 10.0, range);
        for other in world.entities.load().iter() {
            if other.get_entity().entity_id != mob.get_entity().entity_id
                && bounds.intersects(&other.get_entity().bounding_box.load())
                && let Some(other) = other.cast_any().downcast_ref::<BeeEntity>()
            {
                other.stop_being_angry();
                other.set_anger_end_time(
                    world.get_world_age() + other.get_random().random_range(400..=780),
                );
            }
        }
    }
}
