use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::start_attacking::attack_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;

/// Port of `MeleeAttack`.
///
/// Hits the attack target when it is in reach, then sets an attack cooldown with an
/// expiry so the mob cannot swing every tick. The cooldown memory is what paces a brain
/// mob's attacks, in place of the goal system's own timer.
///
/// Known gap: vanilla skips this while the mob holds a usable ranged weapon, so a
/// crossbow piglin shoots rather than punching. There is no such check here yet.
pub struct MeleeAttack {
    cooldown: i64,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl MeleeAttack {
    #[must_use]
    pub fn new(cooldown_between_attacks: i64) -> Self {
        Self {
            cooldown: cooldown_between_attacks,
            conditions: [
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
                (
                    MemoryModuleType::AttackCoolingDown,
                    MemoryStatus::ValueAbsent,
                ),
            ],
        }
    }
}

impl Behavior<MobActor> for MeleeAttack {
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
        let Some(me) = ctx.actor.entity() else {
            return;
        };
        let Some(mob) = me.get_mob() else {
            return;
        };

        let target_entity = target.get_entity();
        let pos = target_entity.pos.load();
        let reach = f64::from(me.get_entity().width() + target_entity.width());
        if ctx.actor.position.squared_distance_to(pos.x, pos.y, pos.z) > reach * reach {
            return;
        }

        ctx.memories
            .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(pos));
        mob.get_mob_entity().try_attack(&*me, &*target);
        ctx.memories.set_with_ttl(
            MemoryModuleType::AttackCoolingDown,
            MemoryValue::Bool(true),
            self.cooldown,
        );
    }

    fn debug_name(&self) -> &'static str {
        "melee_attack"
    }
}
