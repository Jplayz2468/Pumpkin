use std::sync::Arc;

use pumpkin_data::effect::StatusEffect;

use crate::entity::{
    Entity,
    mob::{
        Mob, MobEntity,
        skeleton::{INCREASED_BOW_ATTACK_INTERVAL, SkeletonEntityBase},
    },
    projectile::arrow::ArrowEntity,
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

    /// Parched.java:22-30 `getArrow`: every arrow fired carries `Weakness` for 600 ticks
    /// (30s) at amplifier 0.
    fn customize_arrow(&self, arrow: &ArrowEntity) {
        arrow.add_effect(StatusEffect::WEAKNESS.minecraft_name, 0, 600);
    }
}
