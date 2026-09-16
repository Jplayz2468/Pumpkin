use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `RandomStroll`.
///
/// Picks a random nearby position and writes it as the walk target. `swim` is the
/// variant aquatic mobs use, which is allowed to pick a position inside water.
pub struct RandomStroll {
    speed: f32,
    horizontal_radius: i32,
    vertical_radius: i32,
    swim: bool,
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl RandomStroll {
    /// `RandomStroll.stroll`: the land variant, 10 by 7 as in vanilla.
    #[must_use]
    pub fn stroll(speed: f32) -> Self {
        Self::new(speed, 10, 7, false)
    }

    /// `RandomStroll.swim`: the same radii, but water counts as walkable.
    #[must_use]
    pub fn swim(speed: f32) -> Self {
        Self::new(speed, 10, 7, true)
    }

    fn new(speed: f32, horizontal_radius: i32, vertical_radius: i32, swim: bool) -> Self {
        Self {
            speed,
            horizontal_radius,
            vertical_radius,
            swim,
            conditions: [(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
        }
    }
}

impl Behavior<MobActor> for RandomStroll {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let Some(mob) = entity.get_mob() else {
            return;
        };
        // Vanilla asks `DefaultRandomPos`/`BehaviorUtils` for a position; the wander goal
        // already wraps that search, and `land = !swim` is the same distinction vanilla
        // draws between `LandRandomPos` and the plain variant.
        let target = crate::entity::ai::goal::wander_around::WanderAroundGoal::find_ground_target(
            mob,
            self.horizontal_radius,
            self.vertical_radius,
            !self.swim,
        );
        if let Some(target) = target {
            set_walk_target(ctx.memories, target, self.speed);
        }
    }

    fn debug_name(&self) -> &'static str {
        "random_stroll"
    }
}
