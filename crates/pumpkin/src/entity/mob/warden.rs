use std::sync::{Arc, Mutex, atomic::Ordering};

use super::{
    warden_anger::{AngerManagement, Removal, Suspect},
    warden_anger_nbt, warden_damage, warden_darkness,
    warden_dig::{self, Digging},
    warden_emergence::Emergence,
    warden_melee,
    warden_roar::Roar,
    warden_sensor::{self, Sensor},
    warden_sniff::{self, Sniffing},
    warden_sonic::{self, SonicBoom},
    warden_target::{self, TargetFacts},
};
use crate::entity::{EntityBase, spawn::SpawnReason};
use crate::entity::{RemovalReason, living::get_entity_team};
use pumpkin_data::{
    attributes::Attributes,
    damage::DamageType,
    data_component_impl::EquipmentSlot,
    entity::{EntityPose, EntityStatus, EntityType},
    particle::Particle,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::{codec::var_int::VarInt, java::client::play::Metadata};
use pumpkin_util::{
    math::{boundingbox::EntityDimensions, vector3::Vector3},
    random::RandomImpl,
    version::JavaMinecraftVersion,
};
use std::collections::HashMap;

pub fn dimensions(pose: EntityPose) -> EntityDimensions {
    if matches!(pose, EntityPose::Emerging | EntityPose::Digging) {
        EntityDimensions::new(EntityType::WARDEN.dimension[0], 1.0, 0.85)
    } else {
        EntityDimensions::new(
            EntityType::WARDEN.dimension[0],
            EntityType::WARDEN.dimension[1],
            EntityType::WARDEN.eye_height,
        )
    }
}

use crate::entity::{
    Entity,
    ai::goal::{
        look_around::RandomLookAroundGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct WardenEntity {
    pub mob_entity: MobEntity,
    pub emergence: Mutex<Emergence>,
    anger: Mutex<AngerState>,
    roar: Mutex<Roar>,
    digging: Mutex<Digging>,
    sonic: Mutex<SonicBoom>,
    sensor: Mutex<Sensor>,
    sniff: Mutex<Sniffing>,
    sniff_active: std::sync::atomic::AtomicBool,
    investigate_active: std::sync::atomic::AtomicBool,
    idle_active: std::sync::atomic::AtomicBool,
    fight_active: std::sync::atomic::AtomicBool,
    client_anger: std::sync::atomic::AtomicI32,
    random: Mutex<pumpkin_util::random::legacy_rand::LegacyRand>,
}

struct AngerState {
    manager: AngerManagement,
    removed: HashMap<i32, Removal>,
}

fn suspect(entity: &dyn EntityBase) -> Suspect {
    let base = entity.get_entity();
    Suspect {
        id: base.entity_id,
        uuid: base.entity_uuid.as_u128(),
        player: entity.get_player().is_some(),
        living: entity.get_living_entity().is_some(),
    }
}

impl WardenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut random = pumpkin_util::random::legacy_rand::LegacyRand::from_seed(rand::random());
        let sensor = Sensor::new(|bound| random.next_bounded_i32(bound));
        let warden = Self {
            mob_entity,
            emergence: Mutex::new(Emergence::default()),
            anger: Mutex::new(AngerState {
                manager: AngerManagement::new(rand::random_range(0..=2), &[]),
                removed: HashMap::new(),
            }),
            roar: Mutex::new(Roar::default()),
            digging: Mutex::new(Digging::default()),
            sonic: Mutex::new(SonicBoom::default()),
            sensor: Mutex::new(sensor),
            sniff: Mutex::new(Sniffing::default()),
            sniff_active: std::sync::atomic::AtomicBool::new(false),
            investigate_active: std::sync::atomic::AtomicBool::new(false),
            idle_active: std::sync::atomic::AtomicBool::new(true),
            fight_active: std::sync::atomic::AtomicBool::new(false),
            client_anger: std::sync::atomic::AtomicI32::new(0),
            random: Mutex::new(random),
        };
        let mob_arc = Arc::new(warden);
        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.2, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.5)));
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn can_target_entity(&self, target: &dyn EntityBase) -> bool {
        let entity = target.get_entity();
        let own_world = self.get_entity().world.load();
        let target_world = entity.world.load();
        let own_team = get_entity_team(self);
        let target_team = get_entity_team(target);
        let allied = own_team
            .zip(target_team)
            .is_some_and(|(a, b)| a.name == b.name);
        let living = target.get_living_entity();
        let bounds = entity.bounding_box.load();
        let border = own_world
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let half = border.new_diameter / 2.0;
        let limit = f64::from(border.portal_teleport_boundary);
        warden_target::eligible(
            &TargetFacts {
                living: living.is_some(),
                same_world: Arc::ptr_eq(&own_world, &target_world),
                creative_or_spectator: target
                    .get_player()
                    .is_some_and(|p| p.is_creative() || p.is_spectator()),
                allied,
                armor_stand_or_warden: matches!(
                    entity.entity_type.resource_name,
                    "armor_stand" | "warden"
                ),
                invulnerable: entity.invulnerable.load(Ordering::Relaxed),
                dead_or_dying: living
                    .is_some_and(|l| l.health.load() <= 0.0 || l.dead.load(Ordering::Relaxed)),
            },
            [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z],
            [
                (border.center_x - half).max(-limit),
                (border.center_z - half).max(-limit),
                (border.center_x + half).min(limit),
                (border.center_z + half).min(limit),
            ],
        )
    }

    fn tick_anger(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let value = {
            let mut state = self
                .anger
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let AngerState { manager, removed } = &mut *state;
            manager.tick(
                |uuid| {
                    let uuid = uuid::Uuid::from_u128(uuid);
                    world
                        .get_entity_by_uuid(uuid)
                        .or_else(|| {
                            world
                                .get_player_by_uuid(uuid)
                                .map(|p| p as Arc<dyn EntityBase>)
                        })
                        .map(|e| suspect(e.as_ref()))
                },
                |id| {
                    if let Some(reason) = removed.get(&id) {
                        return (false, Some(*reason));
                    }
                    world.get_entity_by_id(id).map_or((false, None), |e| {
                        (
                            self.can_target_entity(e.as_ref()),
                            e.get_entity().removal_reason.load().map(|r| {
                                if r.should_destroy() {
                                    Removal::Discarded
                                } else {
                                    Removal::Unloaded
                                }
                            }),
                        )
                    })
                },
            );
            removed.clear();
            let target = self
                .mob_entity
                .target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            manager.anger(target.as_ref().map(|e| e.get_entity().entity_id))
        };
        self.set_client_anger(value);
    }

    fn set_client_anger(&self, value: i32) {
        if self.client_anger.swap(value, Ordering::Relaxed) != value {
            self.get_entity().send_meta_data(
                &[
                    Metadata::new(tracked_data::warden::CLIENT_ANGER_LEVEL, VarInt(value)),
                    Metadata::new(tracked_data::warden::ANGER, VarInt(value)),
                ],
                None,
            );
        }
    }

    fn reset_dig_cooldown(&self) {
        let mut emergence = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if emergence.dig_cooldown.is_some() {
            emergence.dig_cooldown = Some(1200);
        }
    }

    fn tick_roar(&self) {
        if self.mob_entity.is_no_ai() {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        // SetRoarTarget runs in the previous tick's idle, sniff or investigate activity.
        let candidate = if self.idle_active.load(Ordering::Relaxed)
            || self.sniff_active.load(Ordering::Relaxed)
            || self.investigate_active.load(Ordering::Relaxed)
        {
            let state = self
                .anger
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.manager.highest >= 80 {
                state.manager.active_entity(|id| {
                    world
                        .get_entity_by_id(id)
                        .is_some_and(|e| self.can_target_entity(e.as_ref()))
                })
            } else {
                None
            }
        } else {
            None
        };
        let change = {
            let mut roar = self
                .roar
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if roar.target.is_none() && roar.attack_target.is_none() {
                roar.target = candidate;
            }
            roar.tick_behavior(world.get_world_age(), |bound| {
                world
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .next_bounded_i32(bound);
            })
        };
        if let Some(id) = change.start {
            entity.set_pose(EntityPose::Roaring);
            self.mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
            if let Some(target) = world.get_entity_by_id(id) {
                self.mob_entity
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at_entity_with_range(&target, 45.0, 90.0);
                if self.can_target_entity(target.as_ref()) {
                    self.reset_dig_cooldown();
                    self.anger
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .manager
                        .increase(suspect(target.as_ref()), 20);
                }
            }
        }
        if change.sound {
            world.play_sound_fine(
                Sound::EntityWardenRoar,
                SoundCategory::Hostile,
                &entity.pos.load(),
                3.0,
                1.0,
            );
        }
        if change.stop && entity.pose.load() == EntityPose::Roaring {
            entity.set_pose(EntityPose::Standing);
        }
        if let Some(id) = change.attack {
            self.mob_entity.set_target(world.get_entity_by_id(id));
        }
    }

    fn sensor_target(&self, target: &dyn EntityBase) -> warden_sensor::Target {
        let base = target.get_entity();
        let pos = base.pos.load();
        let b = base.bounding_box.load();
        let living = target.get_living_entity();
        let mut visibility = if base.is_sneaking() { 0.8 } else { 1.0 };
        if base.invisible.load(Ordering::Relaxed) {
            let covered = living.map_or(0, |living| {
                let equipment = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                [
                    EquipmentSlot::HEAD,
                    EquipmentSlot::CHEST,
                    EquipmentSlot::LEGS,
                    EquipmentSlot::FEET,
                ]
                .iter()
                .filter(|slot| {
                    equipment
                        .equipment
                        .get(*slot)
                        .is_some_and(|stack| !stack.is_empty())
                })
                .count()
            });
            visibility *= 0.7 * f64::from((covered as f32 / 4.0).max(0.1));
        }
        warden_sensor::Target {
            id: base.entity_id,
            player: target.get_player().is_some(),
            position: [pos.x, pos.y, pos.z],
            bounds: [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z],
            alive: base.is_alive() && living.is_some_and(|l| l.health.load() > 0.0),
            spectator: target.is_spectator(),
            eligible: self.can_target_entity(target),
            loaded: true,
            visibility,
        }
    }

    fn tick_sensor(&self) {
        if self.mob_entity.is_no_ai() {
            return;
        }
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let b = entity.bounding_box.load();
        let range = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        let mut sensor = self
            .sensor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let targets = if sensor.delay <= 1 {
            let world = entity.world.load();
            let bounds = b.expand(range, range, range);
            let mut entities = world.get_entities_at_box(&bounds);
            entities.extend(
                world
                    .get_players_at_box(&bounds)
                    .into_iter()
                    .map(|p| p as Arc<dyn EntityBase>),
            );
            entities
                .iter()
                .filter(|e| {
                    e.get_entity().entity_id != entity.entity_id && e.get_living_entity().is_some()
                })
                .map(|e| self.sensor_target(e.as_ref()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        sensor.tick(
            false,
            [pos.x, pos.y, pos.z],
            [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z],
            range,
            &targets,
        );
    }

    fn tick_melee(&self) {
        if self.mob_entity.is_no_ai() || !self.fight_active.load(Ordering::Relaxed) {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let target_id = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target;
        let Some(target) = target_id.and_then(|id| world.get_entity_by_id(id)) else {
            return;
        };
        let pos = entity.pos.load();
        let facts = self.sensor_target(target.as_ref());
        let (sensor_present, visible) = {
            let mut sensor = self
                .sensor
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let present = sensor.present;
            let visible = sensor.contains([pos.x, pos.y, pos.z], target_id, &facts, |_| {
                world.has_line_of_sight(entity.get_eye_pos(), target.get_entity().get_eye_pos())
            });
            (present, visible)
        };
        let b = entity.bounding_box.load();
        let bounds = [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z];
        let vehicle = entity.get_vehicle().map(|v| {
            let b = v.get_entity().bounding_box.load();
            [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
        });
        let attack = warden_melee::attack(
            &mut self
                .sonic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .melee_cooldown,
            &warden_melee::Facts {
                no_ai: false,
                active: true,
                attack: true,
                sensor: sensor_present,
                visible,
                // Warden inherits Mob.canUseNonMeleeWeapon == false.
                usable_non_melee: false,
                in_range: warden_melee::in_range(bounds, vehicle, facts.bounds),
            },
        );
        if !attack {
            return;
        }
        self.mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(self, &target);
        self.mob_entity.living_entity.swing_hand();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
        let pitch = {
            let mut random = self
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (random.next_f32() - random.next_f32()) * 0.2 + 1.0
        };
        world.play_sound_fine(
            Sound::EntityWardenAttackImpact,
            SoundCategory::Hostile,
            &pos,
            10.0,
            pitch,
        );
        // Warden.doHurtTarget writes this even when the target rejects the hit.
        self.roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sonic_cooldown = Some(40);
        self.mob_entity.try_attack(self, target.as_ref());
    }

    fn tick_sonic(&self) {
        if self.mob_entity.is_no_ai() {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let origin = entity.pos.load();
        let (change, target) = {
            // The shared sonic cooldown also receives roar/retaliation writes.
            let mut roar = self
                .roar
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let target = roar.attack_target.and_then(|id| world.get_entity_by_id(id));
            let in_range = target.as_ref().is_some_and(|target| {
                let p = target.get_entity().pos.load();
                warden_sonic::in_range([origin.x, origin.y, origin.z], [p.x, p.y, p.z])
            });
            let eligible = target
                .as_ref()
                .is_some_and(|t| self.can_target_entity(t.as_ref()));
            let change = self
                .sonic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_behavior(
                    world.get_world_age(),
                    &mut roar.sonic_cooldown,
                    &warden_sonic::Facts {
                        active: self.fight_active.load(Ordering::Relaxed),
                        attack: target.is_some(),
                        eligible,
                        in_range,
                    },
                    |bound| {
                        world
                            .random
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .next_bounded_i32(bound);
                    },
                );
            (change, target)
        };
        if change.start {
            world.send_entity_status(entity, EntityStatus::SonicCharge, None);
            world.play_sound_fine(
                Sound::EntityWardenSonicCharge,
                SoundCategory::Hostile,
                &origin,
                3.0,
                1.0,
            );
        }
        let Some(target) = target else {
            return;
        };
        if change.look {
            self.mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_position(self, target.get_entity().pos.load());
        }
        if change.fire {
            let eye = target.get_entity().get_eye_pos();
            // The pinned Warden registry's WARDEN_CHEST attachment is (0,1.6f,0).
            let beam = warden_sonic::Beam::new(
                [origin.x, origin.y + f64::from(1.6_f32), origin.z],
                [eye.x, eye.y, eye.z],
            );
            for [x, y, z] in &beam.particles {
                world.spawn_particles(
                    Particle::SonicBoom,
                    Vector3::new(*x, *y, *z),
                    1,
                    Vector3::new(0.0, 0.0, 0.0),
                    0.0,
                );
            }
            world.play_sound_fine(
                Sound::EntityWardenSonicBoom,
                SoundCategory::Hostile,
                &origin,
                3.0,
                1.0,
            );
            if target.damage_with_context(
                target.as_ref(),
                10.0,
                DamageType::SONIC_BOOM,
                None,
                Some(self),
                Some(self),
            ) {
                if let Some(living) = target.get_living_entity() {
                    let [x, y, z] =
                        beam.push(living.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE));
                    target.get_entity().add_velocity(Vector3::new(x, y, z));
                }
            }
        }
    }

    fn validate_fight_target(&self) {
        if self.mob_entity.is_no_ai() || !self.fight_active.load(Ordering::Relaxed) {
            return;
        }
        let id = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target;
        let Some(id) = id else {
            return;
        };
        self.reset_dig_cooldown();
        let target = self.get_entity().world.load().get_entity_by_id(id);
        let eligible = target
            .as_ref()
            .is_some_and(|e| self.can_target_entity(e.as_ref()));
        let stop = {
            let mut anger = self
                .anger
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stop = !eligible || anger.manager.anger(Some(id)) < 80;
            if !eligible {
                anger.manager.clear(id);
            }
            stop
        };
        if stop {
            self.roar
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .attack_target = None;
            self.mob_entity.set_target(None);
        }
    }

    fn tick_digging(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let emergence = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let roar = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        // Navigation still bridges the old movement controller until its Brain
        // WALK_TARGET storage is replaced; waiting paths are not interrupted.
        let walk = !self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle();
        let change = self
            .digging
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tick(
                world.get_world_age(),
                &warden_dig::Facts {
                    no_ai: self.mob_entity.is_no_ai(),
                    ground: entity.on_ground.load(Ordering::Relaxed),
                    water: entity.is_in_water(),
                    lava: entity.touching_lava.load(Ordering::Relaxed),
                    passenger: entity.has_vehicle(),
                    removed: entity.removal_reason.load().is_some(),
                    attack: roar.attack_target.is_some(),
                    walk,
                    cooldown: emergence.dig_cooldown.is_some(),
                    roar: roar.target.is_some(),
                    emerging: emergence.active,
                },
                |bound| {
                    world
                        .random
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .next_bounded_i32(bound);
                },
                || {
                    let passengers = entity
                        .passengers
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    for passenger in passengers {
                        entity.remove_passenger(passenger.get_entity().entity_id);
                    }
                    if let Some(vehicle) = entity.get_vehicle() {
                        vehicle.get_entity().remove_passenger(entity.entity_id);
                    }
                    (
                        entity.on_ground.load(Ordering::Relaxed),
                        entity.is_in_water(),
                        entity.touching_lava.load(Ordering::Relaxed),
                    )
                },
            );
        if change.start {
            entity.set_pose(EntityPose::Digging);
            world.play_sound_fine(
                Sound::EntityWardenDig,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        if change.agitated {
            world.play_sound_fine(
                Sound::EntityWardenAgitated,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        if change.remove {
            entity.remove();
        }
    }

    fn tick_darkness(&self) {
        let entity = self.get_entity();
        if !warden_darkness::due(
            entity.tick_count.load(Ordering::Relaxed) as i32,
            entity.entity_id,
            self.mob_entity.is_no_ai(),
        ) {
            return;
        }
        let world = entity.world.load();
        let origin = entity.pos.load();
        let own_team = get_entity_team(self);
        for player in world.get_nearby_players(origin, 20.0) {
            let previous = player
                .living_entity
                .get_effect(&pumpkin_data::effect::StatusEffect::DARKNESS);
            let allied = own_team
                .as_ref()
                .zip(get_entity_team(player.as_ref()))
                .is_some_and(|(a, b)| a.name == b.name);
            let pos = player.get_entity().pos.load();
            let delta = pos - origin;
            if !warden_darkness::eligible(
                matches!(
                    player.gamemode.load(),
                    pumpkin_util::gamemode::GameMode::Survival
                        | pumpkin_util::gamemode::GameMode::Adventure
                ),
                allied,
                delta.x * delta.x + delta.y * delta.y + delta.z * delta.z,
                previous.map(|e| (i32::from(e.amplifier), e.duration)),
            ) {
                continue;
            }
            let effect = pumpkin_data::potion::Effect {
                effect_type: &pumpkin_data::effect::StatusEffect::DARKNESS,
                duration: 260,
                amplifier: 0,
                ambient: false,
                show_particles: false,
                show_icon: false,
                blend: true,
            };
            player.living_entity.add_effect(effect);
        }
    }

    fn tick_sniff(&self) {
        if self.mob_entity.is_no_ai() {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let nearest = self
            .sensor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attackable;
        let attack = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target
            .is_some();
        // This bridge is replaced by the shared WALK_TARGET/MoveToTargetSink work.
        let walk = !self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle();
        let change = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tick_behavior(
                world.get_world_age(),
                &warden_sniff::Facts {
                    idle_active: self.idle_active.load(Ordering::Relaxed),
                    sniff_active: self.sniff_active.load(Ordering::Relaxed),
                    nearest: nearest.is_some(),
                    attack,
                    walk,
                },
                |bound| {
                    world
                        .random
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .next_bounded_i32(bound)
                },
            );
        if change.pose {
            entity.set_pose(EntityPose::Sniffing);
        }
        if change.forget_walk {
            self.mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        }
        if change.sound {
            world.play_sound_fine(
                Sound::EntityWardenSniff,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        if change.stop {
            if entity.pose.load() == EntityPose::Sniffing {
                entity.set_pose(EntityPose::Standing);
            }
            if let Some(target) = nearest.and_then(|id| world.get_entity_by_id(id))
                && self.can_target_entity(target.as_ref())
            {
                let origin = entity.pos.load();
                let position = target.get_entity().pos.load();
                if warden_sniff::in_range(
                    [origin.x, origin.y, origin.z],
                    [position.x, position.y, position.z],
                ) {
                    self.reset_dig_cooldown();
                    self.anger
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .manager
                        .increase(suspect(target.as_ref()), 35);
                }
                let missing = self
                    .sniff
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .disturbance
                    .is_none();
                if missing {
                    self.set_disturbance([
                        position.x.floor() as i32,
                        position.y.floor() as i32,
                        position.z.floor() as i32,
                    ]);
                }
            }
        }
    }

    fn set_disturbance(&self, position: [i32; 3]) {
        let world = self.get_entity().world.load();
        let inside = {
            let border = world
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let half = border.new_diameter / 2.0;
            let limit = f64::from(border.portal_teleport_boundary);
            f64::from(position[0]) >= (border.center_x - half).max(-limit)
                && f64::from(position[0]) < (border.center_x + half).min(limit)
                && f64::from(position[2]) >= (border.center_z - half).max(-limit)
                && f64::from(position[2]) < (border.center_z + half).min(limit)
        };
        let angry = {
            let state = self
                .anger
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.manager.highest >= 80
                && state
                    .manager
                    .active_entity(|id| {
                        world
                            .get_entity_by_id(id)
                            .is_some_and(|e| self.can_target_entity(e.as_ref()))
                    })
                    .is_some()
        };
        let attack = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target
            .is_some();
        let mut dig = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let changed = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .disturb(position, inside, angry, attack, &mut dig.dig_cooldown);
        drop(dig);
        if changed {
            self.mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        }
    }

    fn select_activity(&self) {
        if self.mob_entity.is_no_ai() {
            return;
        }
        let emergence = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let emerging = emergence.active;
        let mut roar = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let digging = !emerging && roar.target.is_none() && emergence.dig_cooldown.is_none();
        self.digging
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active = digging;
        roar.active = !emerging && !digging && roar.target.is_some();
        self.fight_active.store(
            !emerging && !digging && !roar.active && roar.attack_target.is_some(),
            Ordering::Relaxed,
        );
        let calm = !emerging && !digging && !roar.active && roar.attack_target.is_none();
        let mut sniff = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let investigate = calm && sniff.disturbance.is_some();
        let sniffing = calm && !investigate && sniff.sniffing.is_some();
        if self.sniff_active.swap(sniffing, Ordering::Relaxed) && !sniffing {
            sniff.sniffing = None;
        }
        if self.investigate_active.swap(investigate, Ordering::Relaxed) && !investigate {
            sniff.disturbance = None;
        }
        self.idle_active
            .store(calm && !investigate && !sniffing, Ordering::Relaxed);
    }

    fn is_digging_or_emerging(&self) -> bool {
        matches!(
            self.get_entity().pose.load(),
            EntityPose::Emerging | EntityPose::Digging
        )
    }
}

impl Mob for WardenEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_observe_removal(&self, id: i32, reason: RemovalReason) {
        let mut state = self
            .anger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.manager.suspects.iter().any(|s| s.id == id) {
            state.removed.insert(
                id,
                if reason.should_destroy() {
                    Removal::Discarded
                } else {
                    Removal::Unloaded
                },
            );
        }
    }

    fn requires_custom_persistence(&self) -> bool {
        self.get_entity().has_vehicle() || self.get_entity().is_leashed()
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        false
    }

    fn mob_finalize_spawn(&self, reason: SpawnReason) {
        let triggered = reason == SpawnReason::Triggered;
        self.emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finalize_spawn(triggered);
        if triggered {
            let entity = self.get_entity();
            entity.set_pose(EntityPose::Emerging);
            entity.world.load().play_sound_fine(
                Sound::EntityWardenAgitated,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        // Mob.finalizeSpawn uses the entity RNG; only the spawn yaw and the
        // Behavior duration draw use the world RNG. Do not perturb later attempts.
        crate::entity::spawn::apply_base_spawn(
            self,
            &mut *self
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        // Warden.tick refreshes this before Monster.tick, even when NoAI is set.
        if self.mob_entity.persistence_required.load(Ordering::Relaxed)
            || self.requires_custom_persistence()
        {
            self.reset_dig_cooldown();
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        if !self.mob_entity.is_no_ai() {
            self.emergence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_memories();
            self.roar
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_memories();
            self.sonic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_memories();
            self.sniff
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_memories();
        }
        self.tick_sensor();
        let transition = if self.mob_entity.is_no_ai() {
            super::warden_emergence::Transition::default()
        } else {
            self.emergence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tick_behavior(world.get_world_age(), |bound| {
                    world
                        .random
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .next_bounded_i32(bound);
                })
        };
        if transition.start {
            entity.set_pose(EntityPose::Emerging);
            world.play_sound_fine(
                Sound::EntityWardenEmerge,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        if transition.stop && entity.pose.load() == EntityPose::Emerging {
            entity.set_pose(EntityPose::Standing);
        }
        self.tick_digging();
        if entity.is_removed() {
            return;
        }
        self.tick_roar();
        self.tick_sniff();
        self.validate_fight_target();
        self.tick_sonic();
        self.tick_melee();
        self.tick_darkness();
        if !self.mob_entity.is_no_ai() && entity.tick_count.load(Ordering::Relaxed) % 20 == 0 {
            self.tick_anger();
        }
        self.select_activity();
    }

    fn run_goal_ai(&self) -> bool {
        !self.is_digging_or_emerging()
            && !self
                .roar
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active
            && !self.sniff_active.load(Ordering::Relaxed)
    }

    fn can_use_melee_attack(&self) -> bool {
        // The retained goal currently supplies navigation only. Brain melee owns
        // attack timing and the shared SonicBoom cooldown; never attack twice.
        false
    }

    fn mob_is_pushable(&self) -> bool {
        !self.is_digging_or_emerging() && self.mob_entity.living_entity.is_pushable()
    }

    fn pre_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) -> bool {
        !self.is_digging_or_emerging()
            || damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
    }

    fn mob_damage_attempt(&self, source: Option<&dyn EntityBase>, cause: Option<&dyn EntityBase>) {
        let Some(attacker) = cause.or(source) else {
            return;
        };
        let entity = self.get_entity();
        let world = entity.world.load();
        let existing = self.mob_entity.get_target();
        let has_dig = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .dig_cooldown
            .is_some();
        let mut roar = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reaction = warden_damage::react(
            &warden_damage::Facts {
                no_ai: self.mob_entity.is_no_ai(),
                digging_or_emerging: self.is_digging_or_emerging(),
                eligible: self.can_target_entity(attacker),
                has_dig_cooldown: has_dig,
                living: attacker.get_living_entity().is_some(),
                player: attacker.get_player().is_some(),
                old_target_player: existing
                    .as_ref()
                    .and_then(|e| e.get_player())
                    .is_some_and(|p| !p.is_creative() && !p.is_spectator()),
                direct: source
                    .is_some_and(|e| e.get_entity().entity_id == attacker.get_entity().entity_id),
                distance_squared: entity
                    .pos
                    .load()
                    .squared_distance_to_vec(&attacker.get_entity().pos.load()),
            },
            roar.attack_target.is_some(),
            |amount| {
                self.anger
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .manager
                    .increase(suspect(attacker), amount)
            },
        );
        if reaction.clear_attack {
            roar.attack_target = None;
        }
        if reaction.set_attack {
            roar.target = None;
            roar.attack_target = Some(attacker.get_entity().entity_id);
            roar.sonic_cooldown = Some(200);
        }
        let target = roar.attack_target;
        drop(roar);
        if reaction.reset_dig {
            self.reset_dig_cooldown();
        }
        if reaction.clear_attack || reaction.set_attack {
            self.mob_entity
                .set_target(target.and_then(|id| world.get_entity_by_id(id)));
        }
    }

    fn mob_java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        let mut metadata = Vec::new();
        Metadata::new(
            tracked_data::entity::DATA_POSE,
            VarInt(self.get_entity().pose.load() as i32),
        )
        .write(&mut metadata, &version)
        .ok()?;
        for field in [
            tracked_data::warden::CLIENT_ANGER_LEVEL,
            tracked_data::warden::ANGER,
        ] {
            Metadata::new(field, VarInt(self.client_anger.load(Ordering::Relaxed)))
                .write(&mut metadata, &version)
                .ok()?;
        }
        metadata.push(255);
        Some(metadata.into_boxed_slice())
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        let state = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut memories = NbtCompound::new();
        for (name, ttl) in [
            ("minecraft:is_emerging", state.emerging_memory),
            ("minecraft:dig_cooldown", state.dig_cooldown),
        ] {
            if let Some(ttl) = ttl {
                let mut memory = NbtCompound::new();
                memory.put_compound("value", NbtCompound::new());
                if ttl != i64::MAX {
                    memory.put_long("ttl", ttl);
                }
                memories.put_compound(name, memory);
            }
        }
        let roar = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (name, ttl) in [
            ("minecraft:roar_sound_delay", roar.sound_delay),
            ("minecraft:roar_sound_cooldown", roar.sound_cooldown),
            ("minecraft:sonic_boom_cooldown", roar.sonic_cooldown),
        ] {
            if let Some(ttl) = ttl {
                let mut memory = NbtCompound::new();
                memory.put_compound("value", NbtCompound::new());
                if ttl != i64::MAX {
                    memory.put_long("ttl", ttl);
                }
                memories.put_compound(name, memory);
            }
        }
        drop(roar);
        let sonic = self
            .sonic
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (name, ttl) in [
            ("minecraft:sonic_boom_sound_delay", sonic.sound_delay),
            ("minecraft:sonic_boom_sound_cooldown", sonic.sound_cooldown),
        ] {
            if let Some(ttl) = ttl {
                let mut memory = NbtCompound::new();
                memory.put_compound("value", NbtCompound::new());
                if ttl != i64::MAX {
                    memory.put_long("ttl", ttl);
                }
                memories.put_compound(name, memory);
            }
        }
        drop(sonic);
        let sniff = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (name, ttl) in [
            ("minecraft:is_sniffing", sniff.sniffing),
            ("minecraft:sniff_cooldown", sniff.cooldown),
        ] {
            if let Some(ttl) = ttl {
                let mut memory = NbtCompound::new();
                memory.put_compound("value", NbtCompound::new());
                if ttl != i64::MAX {
                    memory.put_long("ttl", ttl);
                }
                memories.put_compound(name, memory);
            }
        }
        drop(sniff);
        let mut brain = NbtCompound::new();
        brain.put_compound("memories", memories);
        nbt.put_compound("Brain", brain);

        let anger = self
            .anger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(saved) = warden_anger_nbt::write(&anger.manager) {
            nbt.put_compound("anger", saved);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        *self
            .anger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = AngerState {
            manager: warden_anger_nbt::read(nbt.get_compound("anger"), rand::random_range(0..=2)),
            removed: HashMap::new(),
        };
        let memories = nbt
            .get_compound("Brain")
            .and_then(|b| b.get_compound("memories"));
        let read = |name| {
            memories
                .and_then(|m| m.get_compound(name))
                .filter(|m| m.get_compound("value").is_some())
                .map(|m| m.get_long("ttl").unwrap_or(i64::MAX))
        };
        *self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Emergence {
            emerging_memory: read("minecraft:is_emerging"),
            dig_cooldown: read("minecraft:dig_cooldown"),
            ..Emergence::default()
        };
        *self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Roar {
            sound_delay: read("minecraft:roar_sound_delay"),
            sound_cooldown: read("minecraft:roar_sound_cooldown"),
            sonic_cooldown: read("minecraft:sonic_boom_cooldown"),
            ..Roar::default()
        };
        *self
            .digging
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Digging::default();
        *self
            .sonic
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = SonicBoom {
            sound_delay: read("minecraft:sonic_boom_sound_delay"),
            sound_cooldown: read("minecraft:sonic_boom_sound_cooldown"),
            ..SonicBoom::default()
        };
        *self
            .sensor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Sensor::new(|bound| {
            self.random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next_bounded_i32(bound)
        });
        *self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Sniffing {
            sniffing: read("minecraft:is_sniffing"),
            cooldown: read("minecraft:sniff_cooldown"),
            ..Sniffing::default()
        };
        self.sniff_active.store(false, Ordering::Relaxed);
        self.investigate_active.store(false, Ordering::Relaxed);
        self.idle_active.store(true, Ordering::Relaxed);
        self.fight_active.store(false, Ordering::Relaxed);
        self.set_client_anger(0);
        self.mob_entity.set_target(None);
        // Running behavior/pose are not saved by Java. The loaded Brain selects
        // its activity, then Emerging starts again on its next eligible AI step.
        self.get_entity().data.store(
            i32::from(self.get_entity().pose.load() == EntityPose::Emerging),
            Ordering::Relaxed,
        );
    }
}
