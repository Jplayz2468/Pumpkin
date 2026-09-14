use super::{
    World,
    warden_spawn_attempts::{self, SpawnAccess},
    warden_spawn_position::{full_top_face, move_to_possible_spawn_position},
};
use crate::entity::{
    Entity, EntityBase,
    mob::{
        Mob,
        warden::{WardenEntity, dimensions},
    },
    spawn::SpawnReason,
};
use pumpkin_data::entity::EntityType;
use pumpkin_util::{
    math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3},
    random::RandomImpl,
};
use std::sync::{Arc, atomic::Ordering};

struct Access<'a>(&'a Arc<World>);
impl SpawnAccess for Access<'_> {
    type Mob = Arc<WardenEntity>;
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_bounded_i32(bound)
    }
    fn within_border(&mut self, pos: [i32; 3]) -> bool {
        // Java 26.2 checks the block origin as a point, not both block corners.
        self.0
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(f64::from(pos[0]), f64::from(pos[2]))
    }
    fn find_floor(&mut self, pos: &mut [i32; 3]) -> bool {
        let [x, _, z] = *pos;
        move_to_possible_spawn_position(
            &mut pos[1],
            6,
            |y| self.0.get_block_state(&BlockPos::new(x, y, z)),
            |floor, above| {
                above.get_block_collision_shapes().next().is_none()
                    && full_top_face(floor.get_block_collision_shapes())
            },
        )
    }
    fn create(&mut self, pos: [i32; 3]) -> Option<Self::Mob> {
        let entity = Entity::new(
            self.0.clone(),
            Vector3::new(
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]),
                f64::from(pos[2]) + 0.5,
            ),
            &EntityType::WARDEN,
        );
        let yaw = self
            .0
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f32()
            * 360.0;
        let yaw = if yaw >= 180.0 { yaw - 360.0 } else { yaw };
        entity.set_rotation(yaw, 0.0);
        entity.head_yaw.store(yaw);
        let mob = WardenEntity::new(entity);
        mob.mob_finalize_spawn(SpawnReason::Triggered);
        Some(mob)
    }
    fn check_rules(&mut self, _mob: &Self::Mob) -> bool {
        true
    }
    fn check_obstruction(&mut self, mob: &Self::Mob) -> bool {
        let entity = mob.get_entity();
        let current = entity.bounding_box.load();
        if self.0.contains_any_liquid(current) {
            return false;
        }
        // Living entities, vehicles and solid transient entities block building.
        // Entity collision-category parity is tracked separately from the search.
        if self.0.get_entities_at_box(&current).iter().any(|e| {
            let base = e.get_entity();
            !base.is_removed()
                && (e.get_living_entity().is_some()
                    || e.is_collidable(None)
                    || matches!(base.entity_type.resource_name, "falling_block" | "tnt"))
        }) || self
            .0
            .get_players_at_box(&current)
            .iter()
            .any(|p| !p.is_spectator() && !p.get_entity().is_removed())
        {
            return false;
        }
        let pos = entity.pos.load();
        let full = BoundingBox::new_from_pos(
            pos.x,
            pos.y,
            pos.z,
            &dimensions(pumpkin_data::entity::EntityPose::Standing),
        );
        if !self.0.get_block_collisions(full, mob.as_ref()).0.is_empty() {
            return false;
        }
        // Ordinary mobs can overlap the upper full-height clearance; solid
        // vehicles/shulkers participate in LevelReader.noCollision instead.
        !self.0.get_entities_at_box(&full).iter().any(|e| {
            let base = e.get_entity();
            let name = base.entity_type.resource_name;
            !base.is_removed()
                && (name.ends_with("boat")
                    || name.ends_with("raft")
                    || name.ends_with("minecart")
                    || name == "shulker")
        })
    }
    fn discard(&mut self, _mob: Self::Mob) {
        // Candidate has not entered the world; dropping its Arc disposes it.
    }
    fn add_and_play_ambient(&mut self, mob: Self::Mob) {
        mob.get_entity().data.store(1, Ordering::Relaxed);
        self.0.spawn_initialized_entity(mob);
        // Warden.getAmbientSound returns null while emerging.
    }
}

pub fn try_spawn(world: &Arc<World>, origin: &BlockPos) -> bool {
    warden_spawn_attempts::try_spawn(&mut Access(world), [origin.0.x, origin.0.y, origin.0.z])
}
