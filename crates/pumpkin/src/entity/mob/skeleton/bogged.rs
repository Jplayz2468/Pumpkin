use crate::entity::{
    Entity,
    mob::{
        Mob, MobEntity,
        skeleton::{INCREASED_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
};
use std::sync::Arc;

pub struct BoggedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl BoggedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        // Bogged.java:117-119 overrides `getHardAttackInterval` to 50.
        let entity = SkeletonEntityBase::new(entity, INCREASED_BOW_ATTACK_INTERVAL);
        let bogged = Self { entity };
        Arc::new(bogged)
    }
}

impl Mob for BoggedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }
}
