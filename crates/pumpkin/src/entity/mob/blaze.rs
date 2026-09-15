use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, move_towards_restriction::MoveTowardsRestrictionGoal,
        revenge::RevengeGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct BlazeEntity {
    pub entity: Arc<MobEntity>,
    pub is_charged: AtomicBool,
}

impl BlazeEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = Arc::new(MobEntity::new(entity));
        let blaze = Self {
            entity,
            is_charged: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(blaze);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        {
            let mut goal_selector = mob_arc
                .entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Blaze.java:44-52 (registerGoals): no FloatGoal/SwimGoal is registered for
            // Blaze (only Mob.registerGoals(), which is empty, precedes it -- Mob.java:162-163),
            // so a `SwimGoal` here would be an extra goal relative to vanilla. Removed.
            goal_selector.add_goal(
                4,
                Box::new(
                    crate::entity::ai::goal::blaze_attack::BlazeShootFireballGoal::new(
                        Arc::downgrade(&mob_arc),
                    ),
                ),
            );

            // Blaze.java:46: `MoveTowardsRestrictionGoal(this, 1.0)` at priority 5 -- was
            // entirely missing here.
            goal_selector.add_goal(5, Box::new(MoveTowardsRestrictionGoal::new(1.0)));
            // Blaze.java:47: `WaterAvoidingRandomStrollGoal(this, 1.0, 0.0F)` at priority 7,
            // not 5 -- was previously registered at the wrong priority using the
            // non-water-avoiding variant. `water_avoiding` doesn't expose the vanilla
            // `probability` parameter (Blaze passes 0.0F instead of the 0.001F default,
            // i.e. it should *always* prefer the water-avoiding candidate); no site in this
            // codebase currently threads that parameter through, so this is a known,
            // reported deviation rather than a silent one.
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            // Blaze.java:50: `HurtByTargetGoal(this).setAlertOthers()` at priority 1 -- was
            // entirely missing here. `setAlertOthers()` (alerting nearby Blazes) has no
            // equivalent on `RevengeGoal` in this codebase and isn't implemented per-mob for
            // Blaze either (unlike e.g. `ZombifiedPiglinEntity`'s bespoke alert timer), so
            // that half of vanilla's behaviour is still missing -- flagged, not silently
            // dropped.
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // Blaze.java:51: `NearestAttackableTargetGoal<>(this, Player.class, true)`.
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    pub fn is_charged(&self) -> bool {
        self.is_charged.load(Ordering::Relaxed)
    }

    pub fn set_charged(&self, charged: bool) {
        self.is_charged.store(charged, Ordering::Relaxed);
        let flags = i8::from(charged);
        self.entity
            .living_entity
            .entity
            .set_synced_data(pumpkin_data::tracked_data::blaze::DATA_FLAGS_ID, flags);
    }
}

impl Mob for BlazeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let base_entity = &self.entity.living_entity.entity;
        if !base_entity.is_alive() {
            return;
        }

        let on_ground = base_entity.on_ground.load(Ordering::Relaxed);
        let vel = base_entity.velocity.load();
        if !on_ground && vel.y < 0.0 {
            base_entity
                .velocity
                .store(Vector3::new(vel.x, vel.y * 0.6, vel.z));
        }

        if base_entity.touching_water.load(Ordering::Relaxed) {
            caller.damage(caller, 1.0, DamageType::DROWN);
        }
    }
}
