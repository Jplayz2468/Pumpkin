use std::sync::Arc;

use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;

use crate::entity::EntityBase;
use pumpkin_data::potion::Effect;

use crate::entity::{
    Entity,
    ai::goal::active_target::ActiveTargetGoal,
    mob::{
        Mob, MobEntity,
        skeleton::{DEFAULT_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
    projectile::arrow::ArrowEntity,
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

    /// `WitherSkeleton.doHurtTarget` (WitherSkeleton.java): a successful hit gives the
    /// victim wither for a flat 200 ticks, on every difficulty.
    fn on_attack(&self, target: &dyn EntityBase) {
        if let Some(living) = target.get_living_entity() {
            living.add_effect(Effect {
                effect_type: &StatusEffect::WITHER,
                duration: 200,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }

    /// WitherSkeleton.java:106-110 `getArrow`: `arrow.igniteForSeconds(100.0F)`,
    /// unconditionally - not gated on whether this skeleton itself is on fire (that's a
    /// separate, generic shooter-on-fire check already handled in `BowAttackGoal::shoot`
    /// before this hook runs). `ArrowEntity::set_flame` is the same "burn for 100s" call
    /// that check uses, so this just forces it on regardless.
    fn customize_arrow(&self, arrow: &ArrowEntity) {
        arrow.set_flame(true);
    }
}
