use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target_any::ActiveTargetAnyGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

// Zoglin.java:138 `findNearestValidAttackTarget` excludes only zoglins and creepers from the
// nearest-living-entity search; everything else (players, hostiles, passives) is fair game.
const ZOGLIN_TARGET_EXCLUDE: &[&EntityType] = &[&EntityType::ZOGLIN, &EntityType::CREEPER];

pub struct ZoglinEntity {
    pub mob_entity: MobEntity,
    pub is_baby: AtomicBool,
}

impl ZoglinEntity {
    pub const XP_REWARD: u32 = 5;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        {
            let mut attributes = mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 40.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.3;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(knockback_res) = attributes.get_mut(&Attributes::KNOCKBACK_RESISTANCE.id) {
                knockback_res.base_value = 0.6;
                knockback_res.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(attack_kb) = attributes.get_mut(&Attributes::ATTACK_KNOCKBACK.id) {
                attack_kb.base_value = 1.0;
                attack_kb.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 6.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_entity.living_entity.health.store(40.0);

        let zoglin = Self {
            mob_entity,
            is_baby: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(zoglin);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                Box::new(ActiveTargetAnyGoal::new(
                    &mob_arc.mob_entity,
                    ZOGLIN_TARGET_EXCLUDE,
                    10,
                    true,
                    false,
                    Some(|_target: &crate::entity::living::LivingEntity, _world: &crate::world::World| true),
                )),
            );
        };

        mob_arc
    }
}

impl Mob for ZoglinEntity {
    /// `Zoglin.customServerAiStep` (Zoglin.java:235):
    /// `setAggressive(brain.hasMemoryValue(ATTACK_TARGET))`. Goal-driven here, so the
    /// equivalent condition is whether the zoglin currently has a target.
    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.mob_entity
            .set_attacking(self.mob_entity.get_target().is_some());
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if self.is_baby.load(Ordering::Relaxed) {
            nbt.put_bool("IsBaby", true);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(baby) = nbt.get_bool("IsBaby") {
            self.is_baby.store(baby, Ordering::Relaxed);
        }
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        let my_pos = self.mob_entity.living_entity.entity.pos.load();
        let target_pos = target.get_entity().pos.load();
        let dx = target_pos.x - my_pos.x;
        let dz = target_pos.z - my_pos.z;
        let dist = dx.hypot(dz).max(0.001);
        let vel = target.get_entity().velocity.load();
        target.get_entity().velocity.store(Vector3::new(
            vel.x + (dx / dist) * 0.5,
            0.5,
            vel.z + (dz / dist) * 0.5,
        ));
    }
}
