use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::entity::EntityType;

/// Port of `SetEntityLookTargetSometimes`.
///
/// Glances at a nearby entity, but only every so often: the ticker means a mob notices
/// you intermittently rather than staring from the moment you come into range.
pub struct SetEntityLookTargetSometimes {
    target_type: Option<&'static EntityType>,
    max_distance_squared: f64,
    interval: (i32, i32),
    ticks_until_next: i32,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl SetEntityLookTargetSometimes {
    #[must_use]
    pub fn new(
        target_type: Option<&'static EntityType>,
        max_distance: f32,
        interval: (i32, i32),
    ) -> Self {
        Self {
            target_type,
            max_distance_squared: f64::from(max_distance * max_distance),
            interval,
            ticks_until_next: 0,
            conditions: [
                (MemoryModuleType::LookTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::VisibleMobs, MemoryStatus::ValuePresent),
            ],
        }
    }

    /// Vanilla's `Ticker.tickDownAndCheck`: counts down, and on reaching zero rolls the
    /// next interval and fires.
    fn tick_down_and_check(&mut self) -> bool {
        if self.ticks_until_next > 0 {
            self.ticks_until_next -= 1;
            return false;
        }
        let (min, max) = self.interval;
        self.ticks_until_next = min + rand::random_range(0..=(max - min).max(0));
        true
    }
}

impl Behavior<MobActor> for SetEntityLookTargetSometimes {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        let candidates = match ctx.memories.get(MemoryModuleType::VisibleMobs) {
            Some(MemoryValue::EntityIds(ids)) => ids.clone(),
            _ => return false,
        };
        let found = candidates.iter().any(|id| {
            let Some(entity) = ctx.actor.world.get_entity_by_id(*id) else {
                return false;
            };
            let entity = entity.get_entity();
            if self
                .target_type
                .is_some_and(|wanted| entity.entity_type.id != wanted.id)
            {
                return false;
            }
            let pos = entity.pos.load();
            ctx.actor.position.squared_distance_to(pos.x, pos.y, pos.z) <= self.max_distance_squared
        });
        // Vanilla checks the ticker only once it has something worth looking at.
        found && self.tick_down_and_check()
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let candidates = match ctx.memories.get(MemoryModuleType::VisibleMobs) {
            Some(MemoryValue::EntityIds(ids)) => ids.clone(),
            _ => return,
        };
        let target = candidates.iter().find_map(|id| {
            let entity = ctx.actor.world.get_entity_by_id(*id)?;
            let entity = entity.get_entity();
            if self
                .target_type
                .is_some_and(|wanted| entity.entity_type.id != wanted.id)
            {
                return None;
            }
            let pos = entity.pos.load();
            (ctx.actor.position.squared_distance_to(pos.x, pos.y, pos.z)
                <= self.max_distance_squared)
                .then_some(pos)
        });
        if let Some(pos) = target {
            ctx.memories
                .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(pos));
        }
    }

    fn debug_name(&self) -> &'static str {
        "set_entity_look_target_sometimes"
    }
}
