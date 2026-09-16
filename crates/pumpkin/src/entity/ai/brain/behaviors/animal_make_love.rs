use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::entity::EntityType;

/// `AnimalMakeLove.BREED_RANGE`.
const BREED_RANGE: f64 = 3.0;
const MIN_DURATION: i32 = 60;
const MAX_DURATION: i32 = 110;

/// Port of `AnimalMakeLove`.
///
/// Walks a pair of in-love animals together and marks each as the other's breed target.
/// The partner's memory is set through the deferred-write queue, since a behaviour holds
/// only its own brain -- see `DeferredWrite`.
///
/// Known gap: vanilla spawns the child after a 60-110 tick courtship and hands out the
/// breeding experience. This pairs the animals and walks them together, but does not yet
/// produce offspring; that needs the species' own child-spawning path.
pub struct AnimalMakeLove {
    partner_type: &'static EntityType,
    speed: f32,
    close_enough: i32,
    conditions: [(MemoryModuleType, MemoryStatus); 5],
}

impl AnimalMakeLove {
    #[must_use]
    pub fn new(partner_type: &'static EntityType, speed: f32, close_enough: i32) -> Self {
        Self {
            partner_type,
            speed,
            close_enough,
            conditions: [
                // Vanilla's NEAREST_VISIBLE_LIVING_ENTITIES registers as "visible_mobs".
                (MemoryModuleType::VisibleMobs, MemoryStatus::ValuePresent),
                (MemoryModuleType::BreedTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::WalkTarget, MemoryStatus::Registered),
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (MemoryModuleType::IsPanicking, MemoryStatus::ValueAbsent),
            ],
        }
    }

    /// Vanilla's `findValidBreedPartner`: the nearest in-love animal of the same type,
    /// within breeding range.
    fn find_partner(&self, ctx: &BehaviorContext<'_, MobActor>) -> Option<i32> {
        let candidates = match ctx.memories.get(MemoryModuleType::VisibleMobs) {
            Some(MemoryValue::EntityIds(ids)) => ids.clone(),
            _ => return None,
        };
        candidates.into_iter().find(|id| {
            let Some(candidate) = ctx.actor.world.get_entity_by_id(*id) else {
                return false;
            };
            let entity = candidate.get_entity();
            if entity.entity_type.id != self.partner_type.id {
                return false;
            }
            let pos = entity.pos.load();
            let in_range = ctx.actor.position.squared_distance_to(pos.x, pos.y, pos.z)
                <= BREED_RANGE * BREED_RANGE;
            let in_love = candidate
                .get_mob()
                .is_some_and(|mob| mob.get_mob_entity().is_in_love());
            in_range && in_love
        })
    }

    fn breed_target(ctx: &BehaviorContext<'_, MobActor>) -> Option<i32> {
        match ctx.memories.get(MemoryModuleType::BreedTarget) {
            Some(MemoryValue::EntityId(id)) => Some(*id),
            _ => None,
        }
    }
}

impl Behavior<MobActor> for AnimalMakeLove {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (MIN_DURATION, MAX_DURATION)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        let in_love = ctx
            .actor
            .entity()
            .and_then(|entity| entity.get_mob().map(|mob| mob.get_mob_entity().is_in_love()))
            .unwrap_or(false);
        in_love && self.find_partner(ctx).is_some()
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(partner) = self.find_partner(ctx) else {
            return;
        };
        let me = ctx.actor.mob_id;

        ctx.memories
            .set(MemoryModuleType::BreedTarget, MemoryValue::EntityId(partner));
        // The partner's brain is written after this tick, once our lock is released.
        ctx.defer_set(
            partner,
            MemoryModuleType::BreedTarget,
            MemoryValue::EntityId(me),
        );
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        let Some(partner) = Self::breed_target(ctx) else {
            return false;
        };
        ctx.actor
            .world
            .get_entity_by_id(partner)
            .is_some_and(|entity| !entity.get_entity().removed.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// `BehaviorUtils.lockGazeAndWalkToEachOther`: each animal looks at and walks to the
    /// other. Only this side is driven here; the partner runs its own copy.
    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(partner) = Self::breed_target(ctx) else {
            return;
        };
        let Some(entity) = ctx.actor.world.get_entity_by_id(partner) else {
            return;
        };
        let pos = entity.get_entity().pos.load();
        ctx.memories
            .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(pos));
        let close_enough = f64::from(self.close_enough * self.close_enough);
        if ctx.actor.position.squared_distance_to(pos.x, pos.y, pos.z) > close_enough {
            set_walk_target(ctx.memories, pos, self.speed);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        if let Some(partner) = Self::breed_target(ctx) {
            ctx.defer_erase(partner, MemoryModuleType::BreedTarget);
        }
        ctx.memories.erase(MemoryModuleType::BreedTarget);
        ctx.memories.erase(MemoryModuleType::WalkTarget);
        ctx.memories.erase(MemoryModuleType::LookTarget);
    }

    fn debug_name(&self) -> &'static str {
        "animal_make_love"
    }
}
