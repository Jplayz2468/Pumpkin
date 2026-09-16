use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, Ordering},
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::{
        control::flying_move_control::FlyingMoveControl,
        goal::{
            breed::BreedGoal, follow_parent::FollowParentGoal, swim::SwimGoal, tempt::TemptGoal,
        },
        pathfinder::{Navigator, node::PathType},
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

mod goals;

use pumpkin_data::{attributes::Attributes, damage::DamageType};
use pumpkin_util::math::position::BlockPos;
use rand::RngExt;

pub const FLAG_ROLL: u8 = 2;
pub const FLAG_HAS_STUNG: u8 = 4;
pub const FLAG_HAS_NECTAR: u8 = 8;

pub struct BeeEntity {
    pub hive_pos: crossbeam::atomic::AtomicCell<Option<pumpkin_util::math::position::BlockPos>>,
    pub flower_pos: crossbeam::atomic::AtomicCell<Option<pumpkin_util::math::position::BlockPos>>,
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub ticks_without_nectar: AtomicI32,
    pub cannot_enter_hive_ticks: AtomicI32,
    pub crops_grown_since_pollination: AtomicI32,
    pub time_since_sting: AtomicI32,
    underwater_ticks: AtomicI32,
    anger_end_time: AtomicI64,
    angry_at: crossbeam::atomic::AtomicCell<Option<uuid::Uuid>>,
    pub(super) flower_cooldown: AtomicI32,
    pub(super) hive_cooldown: AtomicI32,
    pub(super) pollinating: AtomicBool,
    blacklisted_hives: std::sync::Mutex<Vec<BlockPos>>,
}

impl BeeEntity {
    pub fn attracts_bees(state: &pumpkin_data::BlockState) -> bool {
        goals::attracts(state)
    }

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let bee = Self {
            hive_pos: crossbeam::atomic::AtomicCell::new(None),
            flower_pos: crossbeam::atomic::AtomicCell::new(None),
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            ticks_without_nectar: AtomicI32::new(0),
            cannot_enter_hive_ticks: AtomicI32::new(0),
            crops_grown_since_pollination: AtomicI32::new(0),
            time_since_sting: AtomicI32::new(0),
            underwater_ticks: AtomicI32::new(0),
            anger_end_time: AtomicI64::new(-1),
            angry_at: crossbeam::atomic::AtomicCell::new(None),
            flower_cooldown: AtomicI32::new(rand::random_range(20..=60)),
            hive_cooldown: AtomicI32::new(0),
            pollinating: AtomicBool::new(false),
            blacklisted_hives: std::sync::Mutex::new(Vec::new()),
        };
        let mob_arc = Arc::new(bee);
        *mob_arc
            .mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FlyingMoveControl::new(20, true));
        let mut navigator = Navigator::java_flying();
        navigator.set_required_path_length(48.0);
        mob_arc
            .mob_entity
            .living_entity
            .controlled_speed
            .store(Some(0.0));
        navigator.set_can_float(false);
        navigator.set_can_open_doors(false);
        navigator.set_can_pass_doors(true);
        for (kind, cost) in [
            (PathType::DamageFire, -1.0),
            (PathType::Water, -1.0),
            (PathType::WaterBorder, 16.0),
            (PathType::Cocoa, -1.0),
            (PathType::Fence, -1.0),
        ] {
            navigator.set_pathfinding_malus(kind, cost);
        }
        *mob_arc
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;
        {
            let mut selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            selector.add_goal(0, Box::new(goals::Attack::new()));
            selector.add_goal(1, Box::new(goals::EnterHive));
            selector.add_goal(2, BreedGoal::new(1.0));
            selector.add_goal(
                3,
                Box::new(TemptGoal::with_tag(1.25, &tag::Item::MINECRAFT_BEE_FOOD)),
            );
            selector.add_goal(3, Box::new(goals::Validate::new(true)));
            selector.add_goal(3, Box::new(goals::Validate::new(false)));
            selector.add_goal(4, Box::new(goals::Pollinate::default()));
            selector.add_goal(5, Box::new(FollowParentGoal::new(1.25)));
            selector.add_goal(5, Box::new(goals::LocateHive));
            selector.add_goal(5, Box::new(goals::ReturnToHive::default()));
            selector.add_goal(6, Box::new(goals::KnownFlower::default()));
            selector.add_goal(7, Box::new(goals::GrowCrops));
            selector.add_goal(8, Box::new(goals::Wander));
            selector.add_goal(9, Box::new(SwimGoal::default()));
        }
        {
            let mut selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            selector.add_goal(1, Box::new(goals::Revenge::new()));
            selector.add_goal(2, Box::new(goals::AngryTarget::new()));
            selector.add_goal(3, Box::new(goals::ResetAnger::default()));
        }

        mob_arc
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::bee::BEE_FLAGS,
            pumpkin_data::tracked_data::bee::DATA_FLAGS_ID,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn has_nectar(&self) -> bool {
        self.has_flag(FLAG_HAS_NECTAR)
    }

    pub fn set_has_nectar(&self, val: bool) {
        if val {
            self.ticks_without_nectar.store(0, Ordering::Relaxed);
        }
        self.set_flag(FLAG_HAS_NECTAR, val);
    }

    #[must_use]
    pub fn has_stung(&self) -> bool {
        self.has_flag(FLAG_HAS_STUNG)
    }

    pub fn set_has_stung(&self, val: bool) {
        self.set_flag(FLAG_HAS_STUNG, val);
    }

    #[must_use]
    pub fn is_rolling(&self) -> bool {
        self.has_flag(FLAG_ROLL)
    }

    pub fn set_rolling(&self, val: bool) {
        self.set_flag(FLAG_ROLL, val);
    }
}

impl AgeableMob for BeeEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for BeeEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_BEE_FOOD)
    }
}

impl Mob for BeeEntity {
    fn mob_omnidirectional_air_mover(&self) -> bool {
        true
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_long(
            "anger_end_time",
            self.anger_end_time.load(Ordering::Relaxed),
        );
        if let Some(uuid) = self.angry_at.load() {
            nbt.put_uuid("angry_at", uuid);
        }
        for (name, pos) in [
            ("hive_pos", self.hive_pos.load()),
            ("flower_pos", self.flower_pos.load()),
        ] {
            if let Some(pos) = pos {
                nbt.put(
                    name,
                    pumpkin_nbt::tag::NbtTag::IntArray(vec![pos.0.x, pos.0.y, pos.0.z]),
                );
            }
        }
        nbt.put_bool("HasNectar", self.has_nectar());
        nbt.put_bool("HasStung", self.has_stung());
        nbt.put_int(
            "TicksSincePollination",
            self.ticks_without_nectar.load(Ordering::Relaxed),
        );
        nbt.put_int(
            "CannotEnterHiveTicks",
            self.cannot_enter_hive_ticks.load(Ordering::Relaxed),
        );
        nbt.put_int(
            "CropsGrownSincePollination",
            self.crops_grown_since_pollination.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        self.set_anger_end_time(nbt.get_long("anger_end_time").unwrap_or_else(|| {
            nbt.get_int("AngerTime").map_or(-1, |ticks| {
                self.get_entity().world.load().get_world_age() + i64::from(ticks)
            })
        }));
        self.angry_at.store(nbt.get_uuid("angry_at"));
        if let Some(uuid) = self.angry_at.load() {
            let world = self.get_entity().world.load_full();
            let target = world
                .get_player_by_uuid(uuid)
                .map(|p| p as Arc<dyn EntityBase>)
                .or_else(|| world.get_entity_by_uuid(uuid));
            self.set_mob_target(target.filter(|t| t.get_living_entity().is_some()));
        }
        let pos = |name| {
            nbt.get_int_array(name).and_then(|v| match v {
                [x, y, z] => Some(pumpkin_util::math::position::BlockPos::new(*x, *y, *z)),
                _ => None,
            })
        };
        self.hive_pos.store(pos("hive_pos"));
        self.flower_pos.store(pos("flower_pos"));
        if let Some(nectar) = nbt.get_bool("HasNectar") {
            self.set_has_nectar(nectar);
        }
        if let Some(stung) = nbt.get_bool("HasStung") {
            self.set_has_stung(stung);
        }
        if let Some(ticks) = nbt.get_int("TicksSincePollination") {
            self.ticks_without_nectar.store(ticks, Ordering::Relaxed);
        }
        if let Some(cannot) = nbt.get_int("CannotEnterHiveTicks") {
            self.cannot_enter_hive_ticks
                .store(cannot, Ordering::Relaxed);
        }
        if let Some(crops) = nbt.get_int("CropsGrownSincePollination") {
            self.crops_grown_since_pollination
                .store(crops, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_mob_y_velocity_drag(&self) -> Option<f64> {
        Some(f64::from(
            crate::entity::ai::control::travel_input::modified_friction(
                0.91,
                self.mob_entity
                    .living_entity
                    .get_attribute_value(&Attributes::AIR_DRAG_MODIFIER) as f32,
            ),
        ))
    }
    fn liquid_jump_strength(&self) -> f64 {
        0.01
    }
    fn is_flapping(&self) -> bool {
        !self.get_entity().on_ground.load(Ordering::Relaxed)
            && self.get_entity().tick_count.load(Ordering::Relaxed) % 2 == 0
    }

    fn pauses_navigation(&self) -> bool {
        self.pollinating.load(Ordering::Relaxed)
    }
    fn ticks_look_control(&self) -> bool {
        !self.is_angry()
    }
    fn resets_look_pitch(&self) -> bool {
        !self.pollinating.load(Ordering::Relaxed)
    }
    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        self.pollinating.store(false, Ordering::Relaxed);
    }

    fn can_use_melee_attack(&self) -> bool {
        self.is_angry() && !self.has_stung()
    }

    fn custom_melee_attack(&self, target: &dyn EntityBase) -> Option<bool> {
        if !self.can_use_melee_attack() {
            return Some(false);
        }
        let damage = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::ATTACK_DAMAGE) as i32 as f32;
        let hit = target.damage_with_context(
            target,
            damage,
            DamageType::STING,
            None,
            Some(self),
            Some(self),
        );
        if hit {
            let weapon = self.mob_entity.living_entity.held_item(self);
            crate::enchantment::EnchantmentHelper::on_post_attack(
                self.get_entity(),
                target.get_entity(),
                &weapon,
            );
            if let Some(living) = target.get_living_entity() {
                living.set_stinger_count(living.stinger_count.load(Ordering::Relaxed) + 1);
                let duration = match self.get_entity().world.load().level_info.load().difficulty {
                    pumpkin_util::Difficulty::Normal => 200,
                    pumpkin_util::Difficulty::Hard => 360,
                    _ => 0,
                };
                if duration > 0 {
                    living.add_effect(pumpkin_data::potion::Effect {
                        effect_type: &pumpkin_data::effect::StatusEffect::POISON,
                        duration,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    });
                }
            }
            self.set_has_stung(true);
            self.stop_being_angry();
            self.get_entity().play_sound(Sound::EntityBeeSting);
        }
        Some(hit)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        if self.get_entity().is_in_water() {
            if self.underwater_ticks.fetch_add(1, Ordering::Relaxed) + 1 > 20 {
                self.damage(caller, 1.0, DamageType::DROWN);
            }
        } else {
            self.underwater_ticks.store(0, Ordering::Relaxed);
        }
        if self.has_stung() {
            let time = self.time_since_sting.fetch_add(1, Ordering::Relaxed) + 1;
            if time % 5 == 0
                && self
                    .get_random()
                    .random_range(0..(1200 - time).clamp(1, 1200))
                    == 0
            {
                self.damage(
                    caller,
                    self.mob_entity.living_entity.health.load(),
                    DamageType::GENERIC,
                );
            }
        }
        if !self.has_nectar() {
            self.ticks_without_nectar.fetch_add(1, Ordering::Relaxed);
        }
        self.update_anger();
    }

    fn post_tick(&self) {
        for cooldown in [
            &self.cannot_enter_hive_ticks,
            &self.flower_cooldown,
            &self.hive_cooldown,
        ] {
            let _ = cooldown.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n > 0).then_some(n - 1)
            });
        }
        self.set_rolling(
            self.is_angry()
                && !self.has_stung()
                && self.mob_entity.get_target().is_some_and(|t| {
                    t.get_entity()
                        .pos
                        .load()
                        .squared_distance_to_vec(&self.get_entity().pos.load())
                        < 4.0
                }),
        );
        if self.get_entity().tick_count.load(Ordering::Relaxed) % 20 == 0 && !self.hive_valid() {
            self.hive_pos.store(None);
        }

        if self.has_nectar()
            && self.crops_grown_since_pollination.load(Ordering::Relaxed) < 10
            && self.get_random().random::<f32>() < 0.05
        {
            use pumpkin_util::{math::vector3::Vector3, random::RandomImpl};
            let entity = self.get_entity();
            let world = entity.world.load_full();
            let pos = entity.pos.load();
            let mut i = 0;
            while i < self.get_random().random_range(0..2) + 1 {
                let (x, z) = {
                    let mut random = world
                        .random
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    (random.next_f64(), random.next_f64())
                };
                world.spawn_particle(
                    Vector3::new(
                        pos.x - 0.3_f32 as f64 + x * (0.6_f32 as f64),
                        pos.y + entity.entity_dimension.load().height as f64 * 0.5,
                        pos.z - 0.3_f32 as f64 + z * (0.6_f32 as f64),
                    ),
                    Vector3::new(0.0, 0.0, 0.0),
                    0.0,
                    1,
                    pumpkin_data::particle::Particle::FallingNectar,
                );
                i += 1;
            }
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = self.get_age() < 0;
        entity.set_synced_data(
            pumpkin_data::tracked_data::bee::DATA_ANGER_END_TIME,
            self.anger_end_time.load(Ordering::Relaxed),
        );
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::bee::DATA_BABY_ID, true);
        }
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::bee::BEE_FLAGS,
            pumpkin_data::tracked_data::bee::DATA_FLAGS_ID,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.is_food(item_stack) {
            let effect = match item_stack.item.id {
                id if id == pumpkin_data::item::Item::WITHER_ROSE.id => {
                    Some((&pumpkin_data::effect::StatusEffect::WITHER, 40))
                }
                id if id == pumpkin_data::item::Item::OPEN_EYEBLOSSOM.id
                    || id == pumpkin_data::item::Item::CLOSED_EYEBLOSSOM.id =>
                {
                    Some((&pumpkin_data::effect::StatusEffect::POISON, 25))
                }
                _ => None,
            };
            if let Some((effect_type, duration)) = effect {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.mob_entity
                    .living_entity
                    .add_effect(pumpkin_data::potion::Effect {
                        effect_type,
                        duration,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    });
                return true;
            }
        }
        self.animal_interact(player, item_stack, Sound::EntityBeePollinate)
    }
}

impl BeeEntity {
    fn set_anger_end_time(&self, time: i64) {
        self.anger_end_time.store(time, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(pumpkin_data::tracked_data::bee::DATA_ANGER_END_TIME, time);
    }

    pub fn is_angry(&self) -> bool {
        let end = self.anger_end_time.load(Ordering::Relaxed);
        end > 0 && end > self.get_entity().world.load().get_world_age()
    }

    fn is_angry_at(&self, uuid: uuid::Uuid) -> bool {
        self.angry_at.load() == Some(uuid)
            || (self.angry_at.load().is_none()
                && self.is_angry()
                && self
                    .get_entity()
                    .world
                    .load()
                    .level_info
                    .load()
                    .game_rules
                    .universal_anger)
    }

    fn stop_being_angry(&self) {
        self.mob_entity
            .living_entity
            .last_attacker_id
            .store(0, Ordering::Relaxed);
        self.angry_at.store(None);
        self.set_mob_target(None);
        self.set_anger_end_time(-1);
    }

    fn update_anger(&self) {
        let world = self.get_entity().world.load_full();
        let previous = self.angry_at.load();
        if let Some(target) = self.mob_entity.get_target() {
            let entity = target.get_entity();
            if !entity.is_alive()
                && target.get_mob().is_some()
                && previous == Some(entity.entity_uuid)
            {
                self.stop_being_angry();
                return;
            }
            if entity.is_alive() && previous != Some(entity.entity_uuid) {
                self.angry_at.store(Some(entity.entity_uuid));
                self.set_anger_end_time(
                    world.get_world_age() + self.get_random().random_range(400..=780),
                );
            }
        }
        if previous.is_some() && !self.is_angry() {
            self.stop_being_angry();
        }
        if let Some(uuid) = previous
            && let Some(player) = world.get_player_by_uuid(uuid)
            && (player.is_creative()
                || player.is_spectator()
                || world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful
                || (!player.get_entity().is_alive()
                    && world.level_info.load().game_rules.forgive_dead_players))
        {
            self.stop_being_angry();
        }
    }

    fn close_to(&self, pos: BlockPos, distance: i32) -> bool {
        self.get_entity().block_pos.load().squared_distance(&pos) < distance * distance
    }

    fn hive_valid(&self) -> bool {
        self.hive_pos.load().is_some_and(|pos| {
            self.close_to(pos, 48)
                && self
                    .get_entity()
                    .world
                    .load()
                    .get_block_entity(&pos)
                    .is_some_and(|be| {
                        be.as_any()
                            .is::<crate::block::entities::beehive::BeehiveBlockEntity>()
                    })
        })
    }

    fn drop_hive(&self) {
        self.hive_pos.store(None);
        self.hive_cooldown.store(200, Ordering::Relaxed);
    }
    fn drop_flower(&self) {
        self.flower_pos.store(None);
        self.flower_cooldown
            .store(self.get_random().random_range(20..=60), Ordering::Relaxed);
    }

    fn is_raining(&self) -> bool {
        self.get_entity()
            .world
            .load()
            .weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .rain_level
            > 0.2
    }

    fn wants_to_enter_hive(&self) -> bool {
        if self.cannot_enter_hive_ticks.load(Ordering::Relaxed) > 0
            || self.pollinating.load(Ordering::Relaxed)
            || self.has_stung()
            || self.mob_entity.get_target().is_some()
        {
            return false;
        }
        let world = self.get_entity().world.load_full();
        let wants = self.has_nectar()
            || self.ticks_without_nectar.load(Ordering::Relaxed) > 3600
            || world.bees_stay_in_hive(&self.get_entity().block_pos.load());
        wants
            && !self
                .hive_pos
                .load()
                .filter(|pos| self.close_to(*pos, 48))
                .and_then(|pos| world.get_block_entity(&pos))
                .is_some_and(|be| {
                    be.as_any()
                        .downcast_ref::<crate::block::entities::beehive::BeehiveBlockEntity>()
                        .is_some_and(|hive| hive.is_fire_nearby(&world))
                })
    }

    fn pathfind_towards(&self, target: BlockPos) {
        use pumpkin_util::math::vector3::Vector3;
        let origin = self.get_entity().block_pos.load();
        let delta_y = target.0.y - origin.0.y;
        let offset = if delta_y > 2 {
            4
        } else if delta_y < -2 {
            -4
        } else {
            0
        };
        let distance = (target.0.x - origin.0.x).abs()
            + (target.0.y - origin.0.y).abs()
            + (target.0.z - origin.0.z).abs();
        let (horizontal, vertical) = if distance < 15 {
            (distance / 2, distance / 2)
        } else {
            (6, 8)
        };
        let target_vec = target.to_centered_f64() - Vector3::new(0.0, 0.5, 0.0);
        if let Some(pos) = self.random_air_position(
            horizontal,
            vertical,
            offset,
            target_vec - self.get_entity().pos.load(),
            (std::f32::consts::PI / 10.0) as f64,
            false,
        ) {
            let world = self.get_entity().world.load_full();
            if world
                .get_block_state(&BlockPos::new(
                    pos.x.floor() as i32,
                    pos.y.floor() as i32,
                    pos.z.floor() as i32,
                ))
                .is_liquid()
            {
                return;
            }
            let mut nav = self
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            nav.set_max_visited_nodes_multiplier(0.5);
            nav.move_to_coords(pos.x, pos.y, pos.z, 1.0, &self.mob_entity.living_entity);
        }
    }

    // RandomPos / HoverRandomPos / AirAndWaterRandomPos: ten candidates, angular
    // sampling, home bias, upward escape from solids, then the bee's air preference.
    fn random_air_position(
        &self,
        horizontal: i32,
        vertical: i32,
        height: i32,
        direction: pumpkin_util::math::vector3::Vector3<f64>,
        angle: f64,
        hover: bool,
    ) -> Option<pumpkin_util::math::vector3::Vector3<f64>> {
        use pumpkin_util::math::vector3::Vector3;
        let world = self.get_entity().world.load_full();
        let origin = self.get_entity().pos.load();
        let mut rng = self.get_random();
        let mut best = None;
        let mut best_score = f64::NEG_INFINITY;
        let has_home = self.mob_entity.has_position_target();
        let home = self.mob_entity.position_target.load();
        let radius = self
            .mob_entity
            .position_target_range
            .load(Ordering::Relaxed);
        let restrict = has_home
            && home.to_centered_f64().squared_distance_to_vec(&origin)
                < f64::from(radius + horizontal + 1).powi(2);
        let mut context =
            crate::entity::ai::pathfinder::pathfinding_context::PathfindingContext::new(
                self.get_entity().block_pos.load().0,
                world.clone(),
            );
        for _ in 0..10 {
            let theta = direction.z.atan2(direction.x) - std::f32::consts::FRAC_PI_2 as f64
                + (2.0 * rng.random::<f32>() - 1.0) as f64 * angle;
            let distance =
                rng.random::<f64>().sqrt() * horizontal as f64 * std::f32::consts::SQRT_2 as f64;
            let x = -distance * theta.sin();
            let z = distance * theta.cos();
            if x.abs() > horizontal as f64 || z.abs() > horizontal as f64 {
                continue;
            }
            let y = rng.random_range(-vertical..=vertical) + height;
            let mut x = x.floor();
            let mut z = z.floor();
            if has_home && horizontal > 1 {
                x += (if origin.x > home.0.x as f64 {
                    -1.0
                } else {
                    1.0
                }) * rng.random::<f64>()
                    * horizontal as f64
                    / 2.0;
                z += (if origin.z > home.0.z as f64 {
                    -1.0
                } else {
                    1.0
                }) * rng.random::<f64>()
                    * horizontal as f64
                    / 2.0;
            }
            let mut pos = BlockPos::new(
                (origin.x + x).floor() as i32,
                (origin.y + y as f64).floor() as i32,
                (origin.z + z).floor() as i32,
            );
            if !world.is_in_height_limit(pos.0.y)
                || (restrict && !self.mob_entity.is_in_position_target_range_pos(&pos))
            {
                continue;
            }
            if hover && world.get_block_state(&pos.down()).is_air() {
                continue;
            }
            let above = if hover { rng.random_range(1..=3) } else { 0 };
            if world.get_block_state(&pos).is_solid() {
                pos = pos.up();
                while world.is_in_height_limit(pos.0.y) && world.get_block_state(&pos).is_solid() {
                    pos = pos.up();
                }
                let first_air = pos.0.y;
                while world.is_in_height_limit(pos.0.y) && pos.0.y - first_air < above {
                    let next = pos.up();
                    if world.get_block_state(&next).is_solid() {
                        break;
                    }
                    pos = next;
                }
            }
            let kind = context.get_land_node_type(pos.0);
            if self
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_pathfinding_malus(kind)
                != 0.0
            {
                continue;
            }
            if hover && kind == PathType::Water {
                continue;
            }
            let score = if world.get_block_state(&pos).is_air() {
                10.0
            } else {
                0.0
            };
            if score > best_score {
                best_score = score;
                best = Some(pos.to_centered_f64() - Vector3::new(0.0, 0.5, 0.0));
            }
        }
        best
    }
}
