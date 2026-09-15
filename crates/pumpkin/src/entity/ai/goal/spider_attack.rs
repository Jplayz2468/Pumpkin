use super::{Controls, Goal};
use crate::entity::ai::goal::melee_attack::MeleeAttackGoal;
use crate::entity::mob::Mob;
use rand::RngExt;

/// Port of `Spider.SpiderAttackGoal` (Spider.java:182-202): a `MeleeAttackGoal` with
/// `pauseWhenMobIdle=true` (Spider.java:184, `super(mob, 1.0, true)`) that additionally
/// refuses to start while the spider is a vehicle (Spider.java:188-189,
/// `return super.canUse() && !this.mob.isVehicle();`) and drops its target once it is
/// standing in bright light (Spider.java:192-200: while light `>= 0.5F`, a 1-in-100 roll
/// per tick clears the target and ends the goal).
///
/// `MeleeAttackGoal`'s fields are private to its own module, so this wraps an inner
/// instance and forwards every `Goal` method instead of subclassing it directly.
pub struct SpiderAttackGoal {
    inner: MeleeAttackGoal,
}

impl SpiderAttackGoal {
    #[must_use]
    pub fn new() -> Self {
        // Spider.java:184: `super(mob, 1.0, true)`.
        Self {
            inner: MeleeAttackGoal::new(1.0, true),
        }
    }
}

impl Default for SpiderAttackGoal {
    fn default() -> Self {
        Self::new()
    }
}

/// `Entity.getLightLevelDependentMagicValue()` (Entity.java:1778-1783) combined with
/// `LevelReader.getLightLevelDependentMagicValue(BlockPos)` (LevelReader.java:112-116):
/// raw sky+block light at the entity's eye position, curved and blended with the
/// dimension's ambient light. Shared with `SpiderTargetGoal`.
pub(crate) fn light_level_dependent_magic_value(mob: &dyn Mob) -> f32 {
    let entity = mob.get_entity();
    let world = entity.world.load();
    let eye_pos = entity.get_eye_pos().to_block_pos();
    let raw = f32::from(world.get_max_local_raw_brightness(&eye_pos)) / 15.0;
    let curved = raw / (4.0 - 3.0 * raw);
    let ambient = world.dimension.ambient_light;
    curved + ambient * (1.0 - curved)
}

impl Goal for SpiderAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        // Spider.java:189: `!this.mob.isVehicle()`.
        if mob.get_entity().is_vehicle() {
            return false;
        }
        self.inner.can_start(mob)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        // Spider.java:194-198.
        let brightness = light_level_dependent_magic_value(mob);
        if brightness >= 0.5 && mob.get_random().random_range(0..100) == 0 {
            mob.set_mob_target(None);
            return false;
        }
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
