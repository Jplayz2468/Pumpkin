use std::sync::Arc;

use crate::entity::{
    Entity,
    mob::{
        Mob, MobEntity,
        skeleton::{DEFAULT_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
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
}
