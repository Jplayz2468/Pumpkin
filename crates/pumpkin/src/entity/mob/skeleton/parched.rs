use std::sync::Arc;

use crate::entity::{
    Entity,
    mob::{
        Mob, MobEntity,
        skeleton::{INCREASED_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
};

pub struct ParchedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl ParchedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        // Parched.java:57-59 overrides `getHardAttackInterval` to 50.
        let entity = SkeletonEntityBase::new(entity, INCREASED_BOW_ATTACK_INTERVAL);
        let parched = Self { entity };
        Arc::new(parched)
    }
}

impl Mob for ParchedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }
}
