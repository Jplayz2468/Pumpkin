use super::{Controls, Goal};
use crate::entity::ai::goal::active_target::ActiveTargetGoal;
use crate::entity::ai::goal::spider_attack::light_level_dependent_magic_value;
use crate::entity::mob::Mob;

/// Port of `Spider.SpiderTargetGoal` (Spider.java:221-231): a
/// `NearestAttackableTargetGoal` that additionally refuses to start in bright light
/// (Spider.java:227-229, `getLightLevelDependentMagicValue() >= 0.5F` blocks `canUse()`).
/// Vanilla instantiates this for both the Player (Spider.java:66) and Iron Golem
/// (Spider.java:67) target-selector entries, and `CaveSpider` inherits the same
/// `registerGoals()` unchanged (CaveSpider.java has no override), so both mobs use this
/// wrapper for both entries.
///
/// `ActiveTargetGoal`'s fields are private to its own module, so this wraps an inner
/// instance and forwards every `Goal` method instead of subclassing it directly.
pub struct SpiderTargetGoal {
    inner: Box<ActiveTargetGoal>,
}

impl SpiderTargetGoal {
    #[must_use]
    pub fn new(inner: Box<ActiveTargetGoal>) -> Box<Self> {
        Box::new(Self { inner })
    }
}

impl Goal for SpiderTargetGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        // Spider.java:228: `float br = this.mob.getLightLevelDependentMagicValue(); return
        // br >= 0.5F ? false : super.canUse();`
        if light_level_dependent_magic_value(mob) >= 0.5 {
            return false;
        }
        self.inner.can_start(mob)
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

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        self.inner.should_run_every_tick()
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}
