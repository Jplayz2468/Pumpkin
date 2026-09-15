use std::sync::Arc;

use pumpkin_data::entity::EntityType;

use crate::entity::{
    Entity,
    ai::goal::active_target::ActiveTargetGoal,
    mob::{
        Mob, MobEntity,
        skeleton::{DEFAULT_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
};

pub struct WitherSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl WitherSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        // WitherSkeleton.java has no `getHardAttackInterval` override.
        let entity = SkeletonEntityBase::new(entity, DEFAULT_BOW_ATTACK_INTERVAL);

        // WitherSkeleton.java:38-41 `registerGoals` adds this target ahead of
        // `super.registerGoals()`. `AbstractPiglin` covers both `Piglin` and
        // `PiglinBrute`; `ActiveTargetGoal` only matches one concrete
        // `EntityType`, so both concrete types are registered here at the
        // same priority to approximate the abstract-class target.
        {
            let mut target_selector = entity
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&entity.mob_entity, &EntityType::PIGLIN, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &entity.mob_entity,
                    &EntityType::PIGLIN_BRUTE,
                    true,
                ),
            );
        }

        let skeleton = Self { entity };
        Arc::new(skeleton)
    }
}

impl Mob for WitherSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }
}
