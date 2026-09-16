use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;

/// Vanilla's `AnimalPanic` runs for 100-120 ticks.
const MIN_DURATION: i32 = 100;
const MAX_DURATION: i32 = 120;
/// `LandRandomPos.getPos(mob, 5, 4)`.
const HORIZONTAL_RANGE: i32 = 5;
const VERTICAL_RANGE: i32 = 4;

/// Port of `AnimalPanic`.
///
/// The brain-side equivalent of the goal system's `PanicGoal`: once the `hurt_by` memory
/// carries a damage type in `panic_causes`, the mob flees for a fixed spell, repathing
/// each time it arrives.
///
/// Known gap: vanilla's burning mob heads for water first (`getPanicPos`). This picks a
/// random position in every case.
pub struct AnimalPanic {
    speed: f32,
    swim: bool,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl AnimalPanic {
    /// `land = true` matches vanilla's default `LandRandomPos` position getter; aquatic
    /// mobs pass `swim` so water counts as somewhere to flee to.
    #[must_use]
    pub fn new(speed: f32, swim: bool) -> Self {
        Self {
            speed,
            swim,
            conditions: [
                (MemoryModuleType::IsPanicking, MemoryStatus::Registered),
                (MemoryModuleType::HurtBy, MemoryStatus::Registered),
            ],
        }
    }
}

impl Behavior<MobActor> for AnimalPanic {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (MIN_DURATION, MAX_DURATION)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        let hurt_by_panic_cause = matches!(
            ctx.memories.get(MemoryModuleType::HurtBy),
            Some(MemoryValue::DamageType(damage_type))
                if damage_type.has_tag(&tag::DamageType::MINECRAFT_PANIC_CAUSES)
        );
        hurt_by_panic_cause || ctx.memories.has(MemoryModuleType::IsPanicking)
    }

    /// Vanilla returns true unconditionally: the spell runs to its timeout.
    fn can_still_use(&mut self, _ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        true
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories
            .set(MemoryModuleType::IsPanicking, MemoryValue::Bool(true));
        ctx.memories.erase(MemoryModuleType::WalkTarget);
        if let Some(entity) = ctx.actor.entity()
            && let Some(mob) = entity.get_mob()
        {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        }
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        // Vanilla repaths only once the previous flee leg is finished.
        if ctx.memories.has(MemoryModuleType::WalkTarget) {
            return;
        }
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let Some(mob) = entity.get_mob() else {
            return;
        };
        if let Some(target) = crate::entity::ai::goal::wander_around::WanderAroundGoal::find_ground_target(
            mob,
            HORIZONTAL_RANGE,
            VERTICAL_RANGE,
            !self.swim,
        ) {
            set_walk_target(ctx.memories, target, self.speed);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(MemoryModuleType::IsPanicking);
    }

    fn debug_name(&self) -> &'static str {
        "animal_panic"
    }
}
