use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use rand::RngExt;

/// Port of the brain-side `Swim`.
///
/// Keeps a mob's head above water by jumping while submerged, the brain equivalent of the
/// goal system's `FloatGoal`.
pub struct Swim {
    /// Chance per tick of jumping while in water, as vanilla passes it.
    chance: f32,
}

impl Swim {
    #[must_use]
    pub const fn new(chance: f32) -> Self {
        Self { chance }
    }

    fn in_water(ctx: &BehaviorContext<'_, MobActor>) -> bool {
        ctx.actor.entity().is_some_and(|entity| {
            entity
                .get_entity()
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }
}

impl Behavior<MobActor> for Swim {
    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        Self::in_water(ctx)
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        Self::in_water(ctx)
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let Some(mob) = entity.get_mob() else {
            return;
        };
        // SwimGoal drives the same flag; `jump` itself is private to LivingEntity.
        let mut random = mob.get_random();
        if random.random::<f32>() < self.chance {
            mob.get_mob_entity()
                .living_entity
                .jumping
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn debug_name(&self) -> &'static str {
        "swim"
    }
}
