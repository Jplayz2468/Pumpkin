mod loot;
use pumpkin_data::{Block, attributes::Attributes, entity::EntityType, item::Item};
use pumpkin_util::random::{RandomImpl, get_seed, legacy_rand::LegacyRand};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::entity::projectile::{ProjectileHit, is_projectile};
use crate::{
    entity::{Entity, EntityBase, living::LivingEntity, player::Player},
    server::Server,
};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;

pub struct FishingBobberEntity {
    pub entity: Entity,
    pub owner_id: i32,
    pub hooked_entity_id: AtomicI32,
    pub in_ground: AtomicBool,
    pub has_hit: AtomicBool,
    pub wait_countdown: AtomicI32,
    pub bite_countdown: AtomicI32,
    approach_countdown: AtomicI32,
    open_water: AtomicBool,
    bobbing: AtomicBool,
    out_of_water: AtomicI32,
    lifetime_ground: AtomicI32,
    lure: i32,
    luck: i32,
    random: Mutex<LegacyRand>,
}

impl FishingBobberEntity {
    const AIR_INERTIA: f64 = 0.92;
    const GRAVITY: f64 = 0.03;

    pub fn new(entity: Entity, owner: &Player) -> Self {
        Self::with_rod(entity, owner, &owner.inventory.held_item())
    }

    /// Positions, rotates and launches the bobber exactly like vanilla's
    /// `FishingHook(Player, Level, int, int)` constructor (`FishingHook.java:82-107`).
    ///
    /// Casting direction/velocity is derived from the owner's own current entity
    /// rotation (`player.getYRot()`/`getXRot()`), never from the use-item packet's
    /// reported yaw/pitch - matching vanilla, which ignores that packet field here.
    pub fn with_rod(entity: Entity, owner: &Player, rod: &ItemStack) -> Self {
        let owner_entity = &owner.living_entity.entity;
        let y_rot = owner_entity.yaw.load();
        let x_rot = owner_entity.pitch.load();

        // FishingHook.java:87-90
        let y_cos = (-y_rot.to_radians() - std::f32::consts::PI).cos();
        let y_sin = (-y_rot.to_radians() - std::f32::consts::PI).sin();
        let x_cos = -(-x_rot.to_radians()).cos();
        let x_sin = (-x_rot.to_radians()).sin();

        // FishingHook.java:91-94: snapTo(x - ySin*0.3, eyeY, z - yCos*0.3, yRot, xRot)
        let owner_pos = owner_entity.pos.load();
        let spawn_pos = Vector3::new(
            owner_pos.x - f64::from(y_sin) * 0.3,
            owner_pos.y + owner_entity.get_eye_height(),
            owner_pos.z - f64::from(y_cos) * 0.3,
        );
        entity.pos.store(spawn_pos);

        // FishingHook.java:95-101: three independent triangular-jittered scale factors,
        // one per axis, applied to the look-direction vector.
        let mut random = LegacyRand::from_seed(get_seed());
        let direction = Vector3::new(
            -f64::from(y_sin),
            f64::from((-(x_sin / x_cos)).clamp(-5.0, 5.0)),
            -f64::from(y_cos),
        );
        let dist = direction.length();
        let velocity = direction.multiply(
            0.6 / dist + random.next_triangular(0.5, 0.010_336_5),
            0.6 / dist + random.next_triangular(0.5, 0.010_336_5),
            0.6 / dist + random.next_triangular(0.5, 0.010_336_5),
        );
        entity.velocity.store(velocity);

        // FishingHook.java:103-104
        entity.set_rotation(
            velocity.x.atan2(velocity.z).to_degrees() as f32,
            velocity.y.atan2(velocity.horizontal_length()).to_degrees() as f32,
        );

        Self {
            entity,
            owner_id: owner.living_entity.entity.entity_id,
            hooked_entity_id: AtomicI32::new(0),
            in_ground: AtomicBool::new(false),
            has_hit: AtomicBool::new(false),
            wait_countdown: AtomicI32::new(0),
            bite_countdown: AtomicI32::new(0),
            approach_countdown: AtomicI32::new(0),
            open_water: AtomicBool::new(true),
            bobbing: AtomicBool::new(false),
            out_of_water: AtomicI32::new(0),
            lifetime_ground: AtomicI32::new(0),
            lure: i32::from(rod.get_enchantment_level(&pumpkin_data::Enchantment::LURE)) * 100,
            luck: i32::from(rod.get_enchantment_level(&pumpkin_data::Enchantment::LUCK_OF_THE_SEA)),
            random: Mutex::new(random),
        }
    }

    pub fn reel_in(&self, player: &Player) -> i32 {
        let world = self.entity.world.load_full();
        let mut result = 0;
        let hooked = self.hooked_entity_id.swap(0, Ordering::Relaxed);
        if hooked != 0
            && let Some(target) = world.get_entity_by_id(hooked)
        {
            let delta = player.position() - self.entity.pos.load();
            target.get_entity().add_velocity(delta * 0.1);
            result = if target.get_item_entity().is_some() {
                3
            } else {
                5
            };
        } else if self.bite_countdown.swap(0, Ordering::Relaxed) > 0 {
            let mut rng = self
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let pos = self.entity.pos.load();
            let biome = world.get_biome(&self.entity.block_pos.load()).registry_id;
            let jungle = matches!(
                biome.trim_start_matches("minecraft:"),
                "jungle" | "sparse_jungle" | "bamboo_jungle"
            );
            let stack = loot::catch(
                &mut *rng,
                self.luck as f32
                    + player.living_entity.get_attribute_value(&Attributes::LUCK) as f32,
                self.open_water.load(Ordering::Relaxed),
                jungle,
            );
            player.trigger_advancement(
                crate::entity::player::advancement::trigger::AdvancementTrigger::FishedItem {
                    item_id: format!("minecraft:{}", stack.item.registry_key),
                },
            );
            if matches!(stack.item.id, id if [Item::COD.id, Item::SALMON.id, Item::TROPICAL_FISH.id, Item::PUFFERFISH.id].contains(&id))
            {
                player.increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Custom,
                    pumpkin_data::statistic::CustomStatistic::FishCaught as i32,
                    1,
                );
            }
            let delta = player.position() - pos;
            let velocity =
                delta
                    .multiply(0.1, 0.1, 0.1)
                    .add_raw(0.0, delta.length().sqrt() * 0.08, 0.0);
            let drop = crate::entity::item::ItemEntity::new_with_velocity(
                Entity::new(world.clone(), pos, &EntityType::ITEM),
                stack,
                velocity,
                0,
            );
            world.spawn_entity(Arc::new(drop));
            crate::entity::experience_orb::ExperienceOrbEntity::spawn(
                &world,
                player.position().add_raw(0.0, 0.5, 0.5),
                (rng.next_bounded_i32(6) + 1) as u32,
            );
            result = 1;
        }
        if self.in_ground.load(Ordering::Relaxed) {
            result = 2;
        }
        result
    }

    fn calculate_open_water(&self) -> bool {
        let world = self.entity.world.load();
        let pos = self.entity.block_pos.load();
        let mut above = false;
        for dy in -1..=2 {
            let mut layer = None;
            for dx in -2..=2 {
                for dz in -2..=2 {
                    let at = pos.offset(Vector3::new(dx, dy, dz));
                    let state = world.get_block_state(&at);
                    let kind = if state.is_air() || state.id.to_block() == &Block::LILY_PAD {
                        true
                    } else if world
                        .get_fluid(&at)
                        .matches_type(&pumpkin_data::fluid::Fluid::WATER)
                        && world.get_fluid_and_fluid_state(&at).1.is_source
                        && state.get_block_collision_shapes_at(&at).next().is_none()
                    {
                        false
                    } else {
                        return false;
                    };
                    if layer.is_some_and(|value| value != kind) {
                        return false;
                    }
                    layer = Some(kind);
                }
            }
            let air = layer.unwrap_or(true);
            if (dy == -1 && air) || (above && !air) {
                return false;
            }
            above = air;
        }
        true
    }

    fn tick_fishing(&self, rng: &mut impl RandomImpl) {
        let world = self.entity.world.load();
        let pos = self.entity.block_pos.load();
        let rate = 1 + i32::from(rng.next_f32() < 0.25 && world.is_raining_at(&pos.up()))
            - i32::from(rng.next_f32() < 0.5 && !world.can_see_sky(&pos.up()));
        if self.bite_countdown.load(Ordering::Relaxed) > 0 {
            if self.bite_countdown.fetch_sub(1, Ordering::Relaxed) == 1 {
                self.wait_countdown.store(0, Ordering::Relaxed);
                self.approach_countdown.store(0, Ordering::Relaxed);
                self.entity.set_synced_data(
                    pumpkin_data::tracked_data::fishing_bobber::DATA_BITING,
                    false,
                );
            }
        } else if self.approach_countdown.load(Ordering::Relaxed) > 0 {
            let remaining = self.approach_countdown.fetch_sub(rate, Ordering::Relaxed) - rate;
            if remaining <= 0 {
                // FishingHook.java:338: volume 0.25, pitch = 1.0 + (rand.nextFloat() - rand.nextFloat()) * 0.4
                let pitch = 1.0 + (rng.next_f32() - rng.next_f32()) * 0.4;
                world.play_sound_fine(
                    Sound::EntityFishingBobberSplash,
                    SoundCategory::Neutral,
                    &self.entity.pos.load(),
                    0.25,
                    pitch,
                );
                self.bite_countdown
                    .store(rng.next_inbetween_i32(20, 40), Ordering::Relaxed);
                self.entity.set_synced_data(
                    pumpkin_data::tracked_data::fishing_bobber::DATA_BITING,
                    true,
                );
                world.spawn_particle(
                    self.entity.pos.load(),
                    Vector3::new(0.25, 0.0, 0.25),
                    0.2,
                    6,
                    pumpkin_data::particle::Particle::Bubble,
                );
            } else {
                world.spawn_particle(
                    self.entity.pos.load(),
                    Vector3::new(0.1, 0.0, 0.1),
                    0.01,
                    2,
                    pumpkin_data::particle::Particle::Fishing,
                );
            }
        } else if self.wait_countdown.load(Ordering::Relaxed) > 0 {
            if self.wait_countdown.fetch_sub(rate, Ordering::Relaxed) - rate <= 0 {
                self.approach_countdown
                    .store(rng.next_inbetween_i32(20, 80), Ordering::Relaxed);
            }
        } else {
            self.wait_countdown.store(
                rng.next_inbetween_i32(100, 600) - self.lure,
                Ordering::Relaxed,
            );
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn process_tick(&self, caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();

        let Some(owner) = world.get_entity_by_id(self.owner_id) else {
            entity.remove();
            return;
        };
        let Some(player) = owner.get_player() else {
            entity.remove();
            return;
        };
        if player.living_entity.health.load() <= 0.0
            || player
                .position()
                .squared_distance_to_vec(&entity.pos.load())
                > 1024.0
            || !pumpkin_util::Hand::all()
                .iter()
                .any(|hand| player.inventory.get_stack_in_hand(*hand).item == &Item::FISHING_ROD)
        {
            player.fishing_bobber.store(-1, Ordering::Relaxed);
            entity.remove();
            return;
        }
        if self.in_ground.load(Ordering::Relaxed) {
            if self.lifetime_ground.fetch_add(1, Ordering::Relaxed) >= 1199 {
                entity.remove();
                player.fishing_bobber.store(-1, Ordering::Relaxed);
            }
            return;
        }
        let hooked_id = self.hooked_entity_id.load(Ordering::Relaxed);
        if hooked_id != 0 {
            if let Some(hooked) = world.get_entity_by_id(hooked_id) {
                if hooked.get_entity().removed.load(Ordering::Relaxed) {
                    self.hooked_entity_id.store(0, Ordering::Relaxed);
                } else {
                    let mut hooked_pos = hooked.get_entity().pos.load();
                    hooked_pos.y += hooked.get_entity().get_eye_height() * 0.8;
                    entity.set_pos(hooked_pos);
                    return;
                }
            } else {
                self.hooked_entity_id.store(0, Ordering::Relaxed);
            }
        }

        let mut velocity = entity.velocity.load();
        let start_pos = entity.pos.load();

        let pos = entity.block_pos.load();
        let water = world
            .get_fluid(&pos)
            .matches_type(&pumpkin_data::fluid::Fluid::WATER);
        let mut rng = self
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if water && !self.bobbing.swap(true, Ordering::Relaxed) {
            entity.velocity.store(velocity.multiply(0.3, 0.2, 0.3));
            return;
        }
        if self.bobbing.load(Ordering::Relaxed) {
            let (fluid, state) = world.get_fluid_and_fluid_state(&pos);
            let height = world.get_fluid_height(&pos, fluid, &state);
            let mut difference = start_pos.y + velocity.y - f64::from(pos.0.y) - f64::from(height);
            if difference.abs() < 0.01 {
                difference += difference.signum() * 0.1;
            }
            velocity = Vector3::new(
                velocity.x * 0.9,
                velocity.y - difference * f64::from(rng.next_f32()) * 0.2,
                velocity.z * 0.9,
            );
            if self.bite_countdown.load(Ordering::Relaxed) > 0
                || self.approach_countdown.load(Ordering::Relaxed) > 0
            {
                self.open_water.store(
                    self.open_water.load(Ordering::Relaxed)
                        && self.out_of_water.load(Ordering::Relaxed) < 10
                        && self.calculate_open_water(),
                    Ordering::Relaxed,
                );
            } else {
                self.open_water.store(true, Ordering::Relaxed);
            }
            if water {
                self.out_of_water.store(
                    (self.out_of_water.load(Ordering::Relaxed) - 1).max(0),
                    Ordering::Relaxed,
                );
                if self.bite_countdown.load(Ordering::Relaxed) > 0 {
                    velocity.y -= 0.1 * f64::from(rng.next_f32()) * f64::from(rng.next_f32());
                }
                self.tick_fishing(&mut *rng);
            } else {
                self.out_of_water.store(
                    (self.out_of_water.load(Ordering::Relaxed) + 1).min(10),
                    Ordering::Relaxed,
                );
            }
        }
        if !water {
            velocity.y -= Self::GRAVITY;
        }
        entity.velocity.store(velocity);
        let new_pos = start_pos + velocity;
        drop(rng);

        let search_box = BoundingBox::new(
            Vector3::new(
                start_pos.x.min(new_pos.x),
                start_pos.y.min(new_pos.y),
                start_pos.z.min(new_pos.z),
            ),
            Vector3::new(
                start_pos.x.max(new_pos.x),
                start_pos.y.max(new_pos.y),
                start_pos.z.max(new_pos.z),
            ),
        )
        .expand(0.3, 0.3, 0.3);

        entity.move_entity(caller, velocity);
        entity
            .velocity
            .store(entity.velocity.load() * Self::AIR_INERTIA);

        self.in_ground
            .store(entity.on_ground.load(Ordering::Relaxed), Ordering::Relaxed);
        if self.bobbing.load(Ordering::Relaxed) {
            return;
        }

        let candidates = world.get_entities_at_box(&search_box);
        for cand in candidates {
            if cand.get_entity().entity_id == self.owner_id
                || cand.get_entity().entity_id == entity.entity_id
            {
                continue;
            }

            if is_projectile(cand.get_entity().entity_type) {
                continue;
            }

            let ebb = cand.get_entity().bounding_box.load().expand(0.3, 0.3, 0.3);
            if ebb.intersects(&search_box) {
                self.hooked_entity_id
                    .store(cand.get_entity().entity_id, Ordering::Relaxed);
                entity.set_synced_data(
                    pumpkin_data::tracked_data::fishing_bobber::DATA_HOOKED_ENTITY,
                    cand.get_entity().entity_id + 1,
                );
                return;
            }
        }
    }
}

impl EntityBase for FishingBobberEntity {
    fn get_owner_id(&self) -> Option<i32> {
        Some(self.owner_id)
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
    fn on_hit(&self, _hit: ProjectileHit) {
        self.has_hit.store(true, Ordering::Relaxed);
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.process_tick(caller);
    }
}
