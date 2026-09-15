use std::sync::atomic::AtomicBool;

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::projectile::ProjectileHit;
use crate::{
    entity::{Entity, EntityBase, projectile::ThrownItemEntity},
    server::Server,
};
use pumpkin_data::world::WorldEvent;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

// ThrownExperienceBottle.java:36 `getDefaultGravity` overrides the throwable default of 0.03.
const GRAVITY: f64 = 0.07;
// ThrownExperienceBottle.java:43 `level.levelEvent(2002, this.blockPosition(), -13083194)`.
const BREAK_EVENT_DATA: i32 = -13083194;

pub struct ThrownExperienceBottleEntity {
    pub thrown: ThrownItemEntity,
}

impl ThrownExperienceBottleEntity {
    pub fn new(entity: Entity) -> Self {
        entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));

        let thrown = ThrownItemEntity {
            entity,
            owner_id: None,
            collides_with_projectiles: false,
            has_hit: AtomicBool::new(false),
            gravity: GRAVITY,
        };

        Self { thrown }
    }

    pub fn new_shot(entity: Entity, shooter: &Entity) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, GRAVITY);
        thrown.entity.set_velocity(Vector3::new(0.0, 0.1, 0.0));
        Self { thrown }
    }
}

impl EntityBase for ThrownExperienceBottleEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.thrown.process_tick(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Mirrors `ThrownExperienceBottle#onHit`, ThrownExperienceBottle.java:39-54: play the
    /// glass-break particle/sound event, then split `3 + nextInt(5) + nextInt(5)` XP
    /// (ThrownExperienceBottle.java:44) into orbs at the impact point.
    fn on_hit(&self, hit: ProjectileHit) {
        let world = self.get_entity().world.load_full();
        let hit_pos = hit.hit_pos();

        let block_pos = BlockPos(Vector3::new(
            hit_pos.x.floor() as i32,
            hit_pos.y.floor() as i32,
            hit_pos.z.floor() as i32,
        ));
        world.sync_world_event(
            WorldEvent::ParticlesSpellPotionSplash,
            block_pos,
            BREAK_EVENT_DATA,
        );

        // ThrownExperienceBottle.java:44: `3 + this.random.nextInt(5) + this.random.nextInt(5)`.
        // This is the sum of two independent rolls (a triangular distribution), not a single
        // uniform roll over the same [3, 11] range.
        let xp_count = 3 + rand::random_range(0..5u32) + rand::random_range(0..5u32);
        ExperienceOrbEntity::spawn(&world, hit_pos, xp_count);
    }
}
