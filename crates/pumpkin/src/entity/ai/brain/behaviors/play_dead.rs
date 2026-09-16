use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;

/// `PlayDead`'s duration, and the length of the regeneration it grants.
const PLAY_DEAD_TICKS: i32 = 200;

/// Port of the axolotl's `PlayDead`.
///
/// An axolotl hurt in water plays dead: it stops moving and regenerates. Holding still is
/// achieved by erasing the walk and look targets, so the sinks have nothing to act on.
pub struct PlayDead {
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl Default for PlayDead {
    fn default() -> Self {
        Self {
            conditions: [
                (MemoryModuleType::PlayDeadTicks, MemoryStatus::ValuePresent),
                (MemoryModuleType::HurtByEntity, MemoryStatus::ValuePresent),
            ],
        }
    }
}

impl PlayDead {
    fn in_water(ctx: &BehaviorContext<'_, MobActor>) -> bool {
        ctx.actor.entity().is_some_and(|entity| {
            entity
                .get_entity()
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }
}

impl Behavior<MobActor> for PlayDead {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (PLAY_DEAD_TICKS, PLAY_DEAD_TICKS)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        Self::in_water(ctx)
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        Self::in_water(ctx) && ctx.memories.has(MemoryModuleType::PlayDeadTicks)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(MemoryModuleType::WalkTarget);
        ctx.memories.erase(MemoryModuleType::LookTarget);
        if let Some(entity) = ctx.actor.entity()
            && let Some(living) = entity.get_living_entity()
        {
            living.add_effect(Effect {
                effect_type: &StatusEffect::REGENERATION,
                duration: PLAY_DEAD_TICKS,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }

    fn debug_name(&self) -> &'static str {
        "play_dead"
    }
}

/// Port of `ValidatePlayDead`.
///
/// Counts the play-dead timer down and, at zero, clears it and returns the brain to its
/// default activity -- which is how the axolotl stops playing dead.
pub struct ValidatePlayDead {
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl Default for ValidatePlayDead {
    fn default() -> Self {
        Self {
            conditions: [
                (MemoryModuleType::PlayDeadTicks, MemoryStatus::ValuePresent),
                (MemoryModuleType::HurtByEntity, MemoryStatus::Registered),
            ],
        }
    }
}

impl Behavior<MobActor> for ValidatePlayDead {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let ticks = match ctx.memories.get(MemoryModuleType::PlayDeadTicks) {
            Some(MemoryValue::Int(ticks)) => *ticks,
            _ => return,
        };
        if ticks <= 0 {
            ctx.memories.erase(MemoryModuleType::PlayDeadTicks);
            ctx.memories.erase(MemoryModuleType::HurtByEntity);
        } else {
            ctx.memories
                .set(MemoryModuleType::PlayDeadTicks, MemoryValue::Int(ticks - 1));
        }
    }

    fn debug_name(&self) -> &'static str {
        "validate_play_dead"
    }
}
