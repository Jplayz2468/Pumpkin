use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        silverfish_merge_with_stone::SilverfishMergeWithStoneGoal,
        silverfish_wake_up_friends::SilverfishWakeUpFriendsGoal, swim::SwimGoal, to_goal_ticks,
    },
    mob::{Mob, MobEntity},
};

pub struct SilverfishEntity {
    pub entity: Arc<MobEntity>,
    /// Shared with the `SilverfishWakeUpFriendsGoal` instance in `goals_selector`: vanilla
    /// keeps `Silverfish.friendsGoal` as a field so `hurtServer` can call `notifyHurt()`
    /// directly on the running goal (Silverfish.java:34,87-88); Pumpkin's goal selector
    /// owns its goals as `Box<dyn Goal>`, so the trigger is this shared counter instead.
    wake_up_friends_ticks: Arc<AtomicI32>,
}

impl SilverfishEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = Arc::new(MobEntity::new(entity));
        let wake_up_friends_ticks = Arc::new(AtomicI32::new(0));
        let silverfish = Self {
            entity,
            wake_up_friends_ticks: wake_up_friends_ticks.clone(),
        };
        let mob_arc = Arc::new(silverfish);
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

            // Silverfish.java:43: the float goal sits at priority 1, not 0.
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            // Silverfish.java:44.
            goal_selector.add_goal(1, Box::new(ClimbOnTopOfPowderSnowGoal));
            // Silverfish.java:45.
            goal_selector.add_goal(
                3,
                Box::new(SilverfishWakeUpFriendsGoal::new(
                    wake_up_friends_ticks.clone(),
                )),
            );
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, false)));
            // Silverfish.java:47: `SilverfishMergeWithStoneGoal`, not a plain stroll.
            goal_selector.add_goal(5, Box::new(SilverfishMergeWithStoneGoal::new()));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }
}

impl Mob for SilverfishEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let yaw = entity.yaw.load();
        entity.body_yaw.store(yaw);
        entity.head_yaw.store(yaw);

        // The `wake_up_friends_ticks` countdown itself is now owned by
        // `SilverfishWakeUpFriendsGoal::tick` (only while that goal is actually running,
        // matching Silverfish.java:196-204), not decremented unconditionally here.
    }

    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        // Silverfish.java:87-88: `if ((source.getEntity() != null ||
        // source.is(DamageTypeTags.ALWAYS_TRIGGERS_SILVERFISH)) && this.friendsGoal !=
        // null) this.friendsGoal.notifyHurt();`. Pumpkin's `on_damage` hook doesn't expose
        // enough of `DamageSource` to replicate the entity/tag condition, so this
        // approximates it as "any damage" -- notifyHurt() itself only sets the counter
        // when it is currently zero (Silverfish.java:190-193), matched below.
        if self.wake_up_friends_ticks.load(Ordering::Relaxed) == 0 {
            self.wake_up_friends_ticks
                .store(to_goal_ticks(20), Ordering::Relaxed);
        }
    }
}
