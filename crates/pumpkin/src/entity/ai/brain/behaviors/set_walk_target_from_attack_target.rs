use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::behaviors::start_attacking::attack_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `SetWalkTargetFromAttackTargetIfTargetOutOfReach`.
///
/// Walks toward the attack target while it is out of reach, and stops walking once it is
/// close enough to hit -- so a mob closes the distance and then stands and fights.
pub struct SetWalkTargetFromAttackTarget {
    speed: f32,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl SetWalkTargetFromAttackTarget {
    #[must_use]
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            conditions: [
                (MemoryModuleType::WalkTarget, MemoryStatus::Registered),
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
            ],
        }
    }
}

impl Behavior<MobActor> for SetWalkTargetFromAttackTarget {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let Some(target) = ctx.actor.world.get_entity_by_id(target_id) else {
            return;
        };
        let target_entity = target.get_entity();
        let pos = target_entity.pos.load();

        ctx.memories
            .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(pos));

        // `BehaviorUtils.isWithinAttackRange` with the projectile buffer: reach is the
        // mob's own width plus the target's, which is what melee range means here.
        let reach = ctx
            .actor
            .entity()
            .map_or(2.0, |me| f64::from(me.get_entity().width() + target_entity.width()));
        let within_reach = ctx
            .actor
            .position
            .squared_distance_to(pos.x, pos.y, pos.z)
            <= reach * reach;

        if within_reach {
            ctx.memories.erase(MemoryModuleType::WalkTarget);
        } else {
            set_walk_target(ctx.memories, pos, self.speed);
        }
    }

    fn debug_name(&self) -> &'static str {
        "set_walk_target_from_attack_target"
    }
}
