use crate::entity::ageable::AgeableMob;
use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `BabyFollowAdult`.
///
/// Keeps a baby near an adult, but only walks when it has drifted past the far end of the
/// follow range, so it trails rather than glues itself on.
pub struct BabyFollowAdult {
    follow_range: (i32, i32),
    speed: f32,
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl BabyFollowAdult {
    #[must_use]
    pub fn new(follow_range: (i32, i32), speed: f32) -> Self {
        Self {
            follow_range,
            speed,
            conditions: [(
                MemoryModuleType::NearestVisibleAdult,
                MemoryStatus::ValuePresent,
            )],
        }
    }
}

impl Behavior<MobActor> for BabyFollowAdult {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // `is_baby` lives on AgeableMob; a species that is never a baby has no ageable
        // view, and vanilla's baby-only behaviour simply never starts for it.
        ctx.actor
            .entity()
            .and_then(|entity| {
                entity
                    .get_mob()
                    .and_then(|mob| mob.as_ageable().map(AgeableMob::is_baby))
            })
            .unwrap_or(false)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let adult = match ctx.memories.get(MemoryModuleType::NearestVisibleAdult) {
            Some(MemoryValue::EntityId(id)) => *id,
            _ => return,
        };
        let Some(entity) = ctx.actor.world.get_entity_by_id(adult) else {
            return;
        };
        let pos = entity.get_entity().pos.load();
        let distance = ctx
            .actor
            .position
            .squared_distance_to(pos.x, pos.y, pos.z);
        let (_, far) = self.follow_range;
        // Vanilla only starts walking once the baby is beyond the range's far end.
        if distance > f64::from(far * far) {
            set_walk_target(ctx.memories, pos, self.speed);
        }
    }

    fn debug_name(&self) -> &'static str {
        "baby_follow_adult"
    }
}
