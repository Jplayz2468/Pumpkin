use std::sync::atomic::Ordering;

use pumpkin_data::entity::EntityType;

use super::{Controls, Goal, active_target::ActiveTargetGoal};
use crate::entity::EntityBase;
use crate::entity::living::LivingEntity;
use crate::entity::mob::{Mob, MobEntity};
use crate::world::World;

/// Vanilla `PolarBear.PolarBearAttackPlayersGoal` (`PolarBear.java:254-275`).
///
/// Wraps a plain `NearestAttackableTargetGoal<Player>(20, true, true, null)` (mirrored here
/// by [`ActiveTargetGoal`]), but only ever fires for an adult bear that has at least one
/// baby polar bear within an 8/4/8-block box of itself
/// (`getBoundingBox().inflate(8.0, 4.0, 8.0)`, PolarBear.java:266): a lone adult polar bear
/// never picks a fight with players on its own.
pub struct PolarBearAttackPlayersGoal {
    inner: ActiveTargetGoal,
}

impl PolarBearAttackPlayersGoal {
    #[must_use]
    pub fn new(mob: &MobEntity) -> Self {
        Self {
            inner: ActiveTargetGoal::new(
                mob,
                &EntityType::PLAYER,
                20,
                true,
                true,
                None::<fn(&LivingEntity, &World) -> bool>,
            ),
        }
    }
}

impl Goal for PolarBearAttackPlayersGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;

        // PolarBear.java:261-263: `if (PolarBear.this.isBaby()) return false;`
        if entity.age.load(Ordering::Relaxed) < 0 {
            return false;
        }

        // PolarBear.java:265: `if (super.canUse())` -- run the wrapped target search first,
        // since it's what actually finds and reserves the player target.
        if !self.inner.can_start(mob) {
            return false;
        }

        // PolarBear.java:266-270: scan `getBoundingBox().inflate(8.0, 4.0, 8.0)` for any
        // baby polar bear; only then does this goal actually engage.
        let world = entity.world.load();
        let search_box = entity.bounding_box.load().expand(8.0, 4.0, 8.0);
        world.get_entities_at_box(&search_box).iter().any(|other| {
            let other_entity = other.get_entity();
            *other_entity.entity_type == EntityType::POLAR_BEAR
                && other_entity.age.load(Ordering::Relaxed) < 0
        })
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}
