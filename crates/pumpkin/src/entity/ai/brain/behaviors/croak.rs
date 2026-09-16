use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::entity::EntityPose;

/// `Croak.CROAK_TICKS` and `TIME_OUT_DURATION`.
const CROAK_TICKS: i32 = 60;
const TIME_OUT: i32 = 100;

/// Port of the frog's `Croak`.
///
/// Holds the croaking pose for 60 ticks while the frog is standing still on land.
pub struct Croak {
    counter: i32,
    conditions: [(MemoryModuleType, MemoryStatus); 1],
}

impl Default for Croak {
    fn default() -> Self {
        Self {
            counter: 0,
            conditions: [(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
        }
    }
}

impl Behavior<MobActor> for Croak {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (TIME_OUT, TIME_OUT)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        ctx.actor
            .entity()
            .is_some_and(|entity| entity.get_entity().pose.load() == EntityPose::Standing)
    }

    fn can_still_use(&mut self, _ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        self.counter < CROAK_TICKS
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        let entity = entity.get_entity();
        // Vanilla only croaks out of liquid.
        if entity
            .touching_water
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        entity.set_pose(EntityPose::Croaking);
        self.counter = 0;
    }

    fn tick(&mut self, _ctx: &mut BehaviorContext<'_, MobActor>) {
        self.counter += 1;
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        if let Some(entity) = ctx.actor.entity() {
            entity.get_entity().set_pose(EntityPose::Standing);
        }
    }

    fn debug_name(&self) -> &'static str {
        "croak"
    }
}
