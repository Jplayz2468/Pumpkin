use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// `FollowTemptation.TEMPTATION_COOLDOWN`.
const TEMPTATION_COOLDOWN: i32 = 100;

/// Port of `FollowTemptation`.
///
/// Follows a player holding food. Stopping sets a 100-tick cooldown, which is why a mob
/// cannot be led indefinitely without pause.
pub struct FollowTemptation {
    speed: f32,
    close_enough: f64,
    conditions: [(MemoryModuleType, MemoryStatus); 7],
}

impl FollowTemptation {
    #[must_use]
    pub fn new(speed: f32, close_enough: f64) -> Self {
        Self {
            speed,
            close_enough,
            conditions: [
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (MemoryModuleType::WalkTarget, MemoryStatus::Registered),
                (
                    MemoryModuleType::TemptationCooldownTicks,
                    MemoryStatus::ValueAbsent,
                ),
                (MemoryModuleType::IsTempted, MemoryStatus::ValueAbsent),
                (MemoryModuleType::TemptingPlayer, MemoryStatus::ValuePresent),
                (MemoryModuleType::BreedTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::IsPanicking, MemoryStatus::ValueAbsent),
            ],
        }
    }

    fn tempting_player(ctx: &BehaviorContext<'_, MobActor>) -> Option<i32> {
        match ctx.memories.get(MemoryModuleType::TemptingPlayer) {
            Some(MemoryValue::EntityId(id)) => Some(*id),
            _ => None,
        }
    }
}

impl Behavior<MobActor> for FollowTemptation {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    /// Vanilla overrides `timedOut` to false: this runs until the tempting player goes
    /// away, not on a timer.
    fn duration_range(&self) -> (i32, i32) {
        (i32::MAX, i32::MAX)
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        Self::tempting_player(ctx).is_some()
            && !ctx.memories.has(MemoryModuleType::BreedTarget)
            && !ctx.memories.has(MemoryModuleType::IsPanicking)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories
            .set(MemoryModuleType::IsTempted, MemoryValue::Bool(true));
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(player_id) = Self::tempting_player(ctx) else {
            return;
        };
        let Some(player) = ctx.actor.world.get_entity_by_id(player_id) else {
            return;
        };
        let player_pos = player.get_entity().pos.load();

        ctx.memories
            .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(player_pos));

        let close_enough = self.close_enough * self.close_enough;
        if ctx
            .actor
            .position
            .squared_distance_to(player_pos.x, player_pos.y, player_pos.z)
            < close_enough
        {
            ctx.memories.erase(MemoryModuleType::WalkTarget);
        } else {
            set_walk_target(ctx.memories, player_pos, self.speed);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.set(
            MemoryModuleType::TemptationCooldownTicks,
            MemoryValue::Int(TEMPTATION_COOLDOWN),
        );
        ctx.memories.erase(MemoryModuleType::IsTempted);
        ctx.memories.erase(MemoryModuleType::WalkTarget);
        ctx.memories.erase(MemoryModuleType::LookTarget);
    }

    fn debug_name(&self) -> &'static str {
        "follow_temptation"
    }
}
