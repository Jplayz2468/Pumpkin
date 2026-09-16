use super::{Entity, EntityBase, living::LivingEntity};
use pumpkin_data::BlockDirection;
use pumpkin_data::entity::EntityType;
use pumpkin_protocol::java::client::play::CEntityVelocity;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use std::{
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
};
pub mod arrow;
pub mod egg;
pub mod ender_pearl;
pub mod evoker_fangs;
pub mod experience_bottle;
pub mod eye_of_ender;
pub mod fireball;
pub mod firework_rocket;
pub mod fishing_bobber;
pub mod lingering_potion;
pub mod llama_spit;
pub mod shulker_bullet;
pub mod small_fireball;
pub mod snowball;
pub mod splash_potion;
pub mod trident;
pub mod wind_charge;
pub mod wither_skull;

use pumpkin_data::item_stack::ItemStack;

#[must_use]
pub fn is_projectile(entity_type: &EntityType) -> bool {
    *entity_type == EntityType::ARROW
        || *entity_type == EntityType::SPECTRAL_ARROW
        || *entity_type == EntityType::BREEZE_WIND_CHARGE
        || *entity_type == EntityType::TRIDENT
        || *entity_type == EntityType::EGG
        || *entity_type == EntityType::SNOWBALL
        || *entity_type == EntityType::FIREWORK_ROCKET
        || *entity_type == EntityType::WIND_CHARGE
        || *entity_type == EntityType::SPLASH_POTION
        || *entity_type == EntityType::LINGERING_POTION
        || *entity_type == EntityType::ENDER_PEARL
        || *entity_type == EntityType::EXPERIENCE_BOTTLE
        || *entity_type == EntityType::SHULKER_BULLET
        || *entity_type == EntityType::FIREBALL
        || *entity_type == EntityType::SMALL_FIREBALL
        || *entity_type == EntityType::FISHING_BOBBER
        || *entity_type == EntityType::WITHER_SKULL
        || *entity_type == EntityType::LLAMA_SPIT
}

/// Projectile.mayInteract: player owners respect spawn protection; other
/// owners use mobGriefing. An absent or no-longer-loaded owner is unrestricted.
pub fn may_interact(
    projectile: &dyn EntityBase,
    world: &crate::world::World,
    pos: &BlockPos,
) -> bool {
    let owner = projectile
        .get_owner_id()
        .and_then(|id| world.get_entity_by_id(id));
    owner.is_none_or(|owner| {
        owner.get_player().map_or_else(
            || world.level_info.load().game_rules.mob_griefing,
            |player| !world.is_in_spawn_protection(player, pos),
        )
    })
}

pub fn may_break(projectile: &dyn EntityBase, world: &crate::world::World) -> bool {
    use pumpkin_data::tag::Taggable;
    projectile
        .get_entity()
        .entity_type
        .has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_IMPACT_PROJECTILES)
        && world
            .level_info
            .load()
            .game_rules
            .projectiles_can_break_blocks
}

/// Projectile.tick emits this once, including projectiles loaded before their
/// first tick. The flag is persisted as vanilla's HasBeenShot field.
pub fn emit_shoot_event(projectile: &dyn EntityBase) {
    let entity = projectile.get_entity();
    if is_projectile(entity.entity_type)
        && !entity
            .projectile_has_been_shot
            .swap(true, Ordering::Relaxed)
    {
        entity.world.load().emit_game_event_with_source(
            "projectile_shoot",
            entity.pos.load(),
            projectile.get_owner_id(),
        );
    }
}

/// Projectile.onHit emits after the hit callback, using the impacted block's
/// resulting state. Preserve the projectile context even if the callback removes it.
pub fn handle_hit(projectile: &dyn EntityBase, hit: ProjectileHit) {
    let entity = projectile.get_entity();
    let world = entity.world.load_full();
    let (origin, block_pos) = match &hit {
        ProjectileHit::Block { pos, .. } => (pos.to_centered_f64(), Some(*pos)),
        ProjectileHit::Entity { hit_pos, .. } => (*hit_pos, None),
    };
    projectile.on_hit(hit);
    world.emit_game_event_from_entity(
        "projectile_land",
        origin,
        Some(projectile),
        block_pos.map(|pos| world.get_block_state_id(&pos)),
    );
}

/// Helper to apply projectile spawned enchantment effects matching vanilla `Projectile::applyOnProjectileSpawned`.
pub fn apply_on_projectile_spawned(
    projectile_entity: &Entity,
    pickup_item_stack: &ItemStack,
    weapon: Option<&ItemStack>,
    arrow: Option<&arrow::ArrowEntity>,
) {
    crate::enchantment::EnchantmentHelper::on_projectile_spawned(
        pickup_item_stack,
        projectile_entity,
        arrow,
    );
    if let Some(weapon) = weapon
        && weapon.item_count > 0
        && weapon.item.id != pickup_item_stack.item.id
    {
        crate::enchantment::EnchantmentHelper::on_projectile_spawned(
            weapon,
            projectile_entity,
            arrow,
        );
    }
}

pub struct ThrownItemEntity {
    pub entity: Entity,
    pub owner_id: Option<i32>,
    pub collides_with_projectiles: bool,
    pub has_hit: AtomicBool,
    pub gravity: f64,
}

impl ThrownItemEntity {
    pub fn new(entity: Entity, owner: &Entity, gravity: f64) -> Self {
        let mut owner_pos = owner.pos.load();
        owner_pos.y += owner.get_eye_height() - 0.1;
        entity.set_pos(owner_pos);
        Self {
            entity,
            owner_id: Some(owner.entity_id),
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity,
        }
    }

    pub fn set_velocity_from(&self, pitch: f32, yaw: f32, roll: f32, speed: f32, divergence: f32) {
        let yaw_rad = yaw.to_radians();
        let pitch_rad = pitch.to_radians();
        let roll_rad = (pitch + roll).to_radians();

        let x = -yaw_rad.sin() * pitch_rad.cos();
        let y = -roll_rad.sin();
        let z = yaw_rad.cos() * pitch_rad.cos();

        self.set_velocity(
            f64::from(x),
            f64::from(y),
            f64::from(z),
            f64::from(speed),
            f64::from(divergence),
        );
    }

    pub fn set_velocity(&self, x: f64, y: f64, z: f64, power: f64, uncertainty: f64) {
        fn next_triangular(mode: f64, deviation: f64) -> f64 {
            deviation.mul_add(rand::random::<f64>() - rand::random::<f64>(), mode)
        }
        let velocity = Vector3::new(x, y, z)
            .normalize()
            .add_raw(
                next_triangular(0.0, 0.017_227_5 * uncertainty),
                next_triangular(0.0, 0.017_227_5 * uncertainty),
                next_triangular(0.0, 0.017_227_5 * uncertainty),
            )
            .multiply(power, power, power);

        self.entity.velocity.store(velocity);
        let len = velocity.horizontal_length();
        self.entity.set_rotation(
            velocity.x.atan2(velocity.z) as f32 * 57.295_776,
            velocity.y.atan2(len) as f32 * 57.295_776,
        );
    }
}

impl ThrownItemEntity {
    /// Process a tick for projectile movement and collisions
    pub fn process_tick(&self, caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();

        entity.update_last_pos();

        // Apply gravity and inertia
        let mut velocity = entity.velocity.load();
        velocity.y -= self.get_gravity();

        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            0.8
        } else {
            0.99
        };
        velocity = velocity.multiply(inertia, inertia, inertia);

        // Store velocity
        entity.velocity.store(velocity);

        let start_pos = entity.pos.load();
        let delta = velocity;

        // Update position
        let new_pos = start_pos.add(&delta);
        let hit = collision_on_segment(caller, start_pos, new_pos, |candidate| {
            self.should_skip_collision(entity, candidate)
        });
        let new_pos = hit.as_ref().map_or(new_pos, ProjectileHit::hit_pos);
        entity.record_inside_movement(start_pos, new_pos, None);
        entity.set_pos(new_pos);

        // Send updated velocity to clients
        let packet = CEntityVelocity::new(entity.entity_id.into(), velocity);
        let chunk_pos = entity.chunk_pos.load();
        world.broadcast_to_chunk(chunk_pos, &packet);

        // Handle hit or continue
        if let Some(h) = hit {
            // Ensure hit is only processed once per projectile
            if self.has_hit.swap(true, Ordering::SeqCst) {
                return;
            }

            if let ProjectileHit::Block {
                pos, hit_pos, face, ..
            } = &h
            {
                let block = world.get_block(pos);
                let state = world.get_block_state(pos);
                if let Some(server) = world.server.upgrade() {
                    world.block_registry.on_projectile_hit(
                        block, &world, caller, pos, state, hit_pos, *face, &server,
                    );
                }
            }

            // Just trigger hit effects and remove
            handle_hit(caller, h);
            entity.remove();
        }
    }

    /// Returns if collision should be skipped (e.g. owner or projectile vs projectile)
    fn should_skip_collision(&self, self_ent: &Entity, other: &Arc<dyn EntityBase>) -> bool {
        let other_ent = other.get_entity();
        if other_ent.entity_id == self_ent.entity_id {
            return true;
        }

        // Skip owner for initial frames
        if Some(other_ent.entity_id) == self.owner_id
            && self_ent.tick_count.load(Ordering::Relaxed) < 5
        {
            return true;
        }

        // Projectiles should pass through lingering clouds
        if *other_ent.entity_type == EntityType::AREA_EFFECT_CLOUD {
            return true;
        }

        // Projectile vs projectile logic
        if !self.collides_with_projectiles && is_projectile(other_ent.entity_type) {
            return true;
        }

        false
    }

    const fn get_entity(&self) -> &Entity {
        &self.entity
    }

    #[allow(dead_code, clippy::unused_self)]
    const fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }
    const fn get_gravity(&self) -> f64 {
        self.gravity
    }
}

/// ProjectileUtil's movement query: clip blocks first, then query the swept
/// projectile box (including players) and select the nearest entering entity face.
fn collision_on_segment(
    caller: &dyn EntityBase,
    start: Vector3<f64>,
    end: Vector3<f64>,
    mut should_skip: impl FnMut(&Arc<dyn EntityBase>) -> bool,
) -> Option<ProjectileHit> {
    let entity = caller.get_entity();
    let world = entity.world.load();
    let delta = entity.velocity.load();
    let normal = delta.normalize() * -1.0;
    let block = world.ray_trace_block_including_border(start, end, caller);
    let entity_end = block.map_or(end, |(_, hit, _)| hit.position);
    let search = entity
        .bounding_box
        .load()
        .stretch(delta)
        .expand(1.0, 1.0, 1.0);
    let margin = entity_margin(entity.tick_count.load(Ordering::Relaxed));
    let candidates = world
        .get_all_at_box(&search)
        .into_iter()
        .filter(|candidate| {
            candidate.get_entity().entity_id != entity.entity_id
                && !candidate.get_entity().is_removed()
                && !candidate.is_spectator()
                && !should_skip(candidate)
        })
        .map(|candidate| {
            let bounds = candidate.get_entity().bounding_box.load();
            (candidate, bounds)
        });
    if let Some((entity, hit_pos)) = nearest_entity_hit(start, entity_end, margin, candidates) {
        return Some(ProjectileHit::Entity {
            entity,
            hit_pos,
            normal,
        });
    }
    block.map(|(pos, hit, world_border)| ProjectileHit::Block {
        world_border,
        pos,
        face: hit.direction,
        hit_pos: hit.position,
        normal,
    })
}

fn nearest_entity_hit<T>(
    start: Vector3<f64>,
    end: Vector3<f64>,
    margin: f32,
    candidates: impl IntoIterator<Item = (T, BoundingBox)>,
) -> Option<(T, Vector3<f64>)> {
    let mut nearest = f64::MAX;
    let mut hit = None;
    for (candidate, bounds) in candidates {
        let margin = f64::from(margin);
        let bounds = bounds.expand(margin, margin, margin);
        if let Some((_, _, position)) =
            crate::world::World::intersects_aabb_with_hit(start, end, bounds.min, bounds.max)
        {
            let distance = (position - start).length_squared();
            if distance < nearest {
                nearest = distance;
                hit = Some((candidate, position));
            }
        }
    }
    hit
}

fn entity_margin(ticks: i32) -> f32 {
    (ticks.wrapping_sub(2) as f32 / 20.0).clamp(0.0, 0.3)
}

/// AbstractArrow (including tridents) reverses on border hits without invoking
/// onHit, lodging in the border, consuming pierce count or emitting a land event.
fn bounce_on_border(entity: &Entity, hit: &ProjectileHit) -> bool {
    if !matches!(
        hit,
        ProjectileHit::Block {
            world_border: true,
            ..
        }
    ) {
        return false;
    }
    let (velocity, yaw) = border_deflection(
        entity.velocity.load(),
        entity.yaw.load(),
        &mut *entity.random(),
    );
    entity.velocity.store(velocity);
    entity.yaw.store(yaw);
    entity.velocity_dirty.store(true, Ordering::Relaxed);
    true
}

fn border_deflection(
    velocity: Vector3<f64>,
    yaw: f32,
    random: &mut impl pumpkin_util::random::RandomImpl,
) -> (Vector3<f64>, f32) {
    let rotation = 170.0_f32 + random.next_f32() * 20.0_f32;
    (velocity * -0.5 * 0.2, yaw + rotation)
}

pub enum ProjectileHit {
    Block {
        world_border: bool,
        pos: BlockPos,
        face: BlockDirection,
        hit_pos: Vector3<f64>,
        normal: Vector3<f64>,
    },
    Entity {
        entity: Arc<dyn EntityBase>,
        hit_pos: Vector3<f64>,
        normal: Vector3<f64>,
    },
}

impl ProjectileHit {
    /// Returns the exact impact coordinates regardless of what was hit.
    #[must_use]
    pub const fn hit_pos(&self) -> Vector3<f64> {
        match self {
            Self::Block { hit_pos, .. } | Self::Entity { hit_pos, .. } => *hit_pos,
        }
    }

    /// Returns the surface normal of the impact.
    #[must_use]
    pub const fn normal(&self) -> Vector3<f64> {
        match self {
            Self::Block { normal, .. } | Self::Entity { normal, .. } => *normal,
        }
    }

    /// Safely returns the face hit if it was a block, otherwise None.
    #[must_use]
    pub const fn face(&self) -> Option<BlockDirection> {
        match self {
            Self::Block { face, .. } => Some(*face),
            Self::Entity { .. } => None,
        }
    }
}

#[cfg(test)]
mod ray_tests {
    use super::*;

    #[test]
    fn java_border_deflection() {
        use pumpkin_util::random::{RandomImpl, legacy_rand::LegacyRand};
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("border_deflection_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let vector = |json: &serde_json::Value| {
                Vector3::new(
                    f64::from_bits(json[0].as_u64().unwrap()),
                    f64::from_bits(json[1].as_u64().unwrap()),
                    f64::from_bits(json[2].as_u64().unwrap()),
                )
            };
            let mut random = LegacyRand::from_seed(case[0].as_i64().unwrap() as u64);
            let (velocity, yaw) = border_deflection(
                vector(&case[1]),
                f32::from_bits(case[2].as_u64().unwrap() as u32),
                &mut random,
            );
            let expected = vector(&case[3]);
            assert_eq!(
                [
                    velocity.x.to_bits(),
                    velocity.y.to_bits(),
                    velocity.z.to_bits()
                ],
                [
                    expected.x.to_bits(),
                    expected.y.to_bits(),
                    expected.z.to_bits()
                ],
                "velocity {index}"
            );
            assert_eq!(
                yaw.to_bits(),
                case[4].as_u64().unwrap() as u32,
                "yaw {index}"
            );
            assert!(case[5].as_bool().unwrap());
            assert_eq!(random.next_i64(), case[6].as_i64().unwrap(), "rng {index}");
        }
    }

    #[test]
    fn java_projectile_entity_rays() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("ray_cases.json")).unwrap();
        for (index, case) in cases.iter().enumerate() {
            let vector = |value: &serde_json::Value, offset: usize| {
                Vector3::new(
                    f64::from_bits(value[offset].as_u64().unwrap()),
                    f64::from_bits(value[offset + 1].as_u64().unwrap()),
                    f64::from_bits(value[offset + 2].as_u64().unwrap()),
                )
            };
            let margin = entity_margin(case[0].as_i64().unwrap() as i32);
            assert_eq!(
                margin.to_bits(),
                case[1].as_u64().unwrap() as u32,
                "margin {index}"
            );
            let candidates = case[4]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(id, value)| (id, BoundingBox::new(vector(value, 0), vector(value, 3))));
            let hit =
                nearest_entity_hit(vector(&case[2], 0), vector(&case[3], 0), margin, candidates);
            assert_eq!(hit.is_some(), !case[5].is_null(), "presence {index}");
            if let Some((id, position)) = hit {
                assert_eq!(id, case[5][0].as_u64().unwrap() as usize, "target {index}");
                let expected = vector(&case[5][1], 0);
                assert_eq!(
                    [
                        position.x.to_bits(),
                        position.y.to_bits(),
                        position.z.to_bits()
                    ],
                    [
                        expected.x.to_bits(),
                        expected.y.to_bits(),
                        expected.z.to_bits()
                    ],
                    "position {index}"
                );
            }
        }
    }
}
