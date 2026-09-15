use std::sync::Arc;

use pumpkin_data::effect::StatusEffect;

use crate::entity::{
    Entity,
    mob::{
        Mob, MobEntity,
        skeleton::{DEFAULT_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
    projectile::arrow::ArrowEntity,
};

pub struct StraySkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl StraySkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        // Stray.java has no `getHardAttackInterval` override.
        let entity = SkeletonEntityBase::new(entity, DEFAULT_BOW_ATTACK_INTERVAL);
        let stray = Self { entity };
        Arc::new(stray)
    }
}

impl Mob for StraySkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    /// Stray.java:59-67 `getArrow`: every arrow fired carries `Slowness` for 600 ticks
    /// (30s) at amplifier 0.
    fn customize_arrow(&self, arrow: &ArrowEntity) {
        arrow.add_effect(StatusEffect::SLOWNESS.minecraft_name, 0, 600);
    }
}
