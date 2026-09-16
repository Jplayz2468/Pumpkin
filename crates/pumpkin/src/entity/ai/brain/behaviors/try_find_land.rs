use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::goal::try_find_water::TryFindWaterGoal;
use pumpkin_util::math::position::BlockPos;

/// `TryFindLand.COOLDOWN_TICKS`.
const COOLDOWN_TICKS: i64 = 60;

/// Port of `TryFindLand`.
///
/// Sends a swimming frog back to dry land: a solid-topped block with clear air above and
/// no fluid. The cooldown keeps it from rescanning the area every tick.
pub struct TryFindLand {
    range: i32,
    speed: f32,
    next_ok_start_time: i64,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl TryFindLand {
    #[must_use]
    pub fn new(range: i32, speed: f32) -> Self {
        Self {
            range,
            speed,
            next_ok_start_time: 0,
            conditions: [
                (MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
            ],
        }
    }
}

impl Behavior<MobActor> for TryFindLand {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // Only a frog that is actually in water needs land.
        ctx.actor.entity().is_some_and(|entity| {
            entity
                .get_entity()
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        if ctx.time < self.next_ok_start_time {
            return;
        }
        self.next_ok_start_time = ctx.time + COOLDOWN_TICKS;

        let origin = BlockPos::floored(
            ctx.actor.position.x,
            ctx.actor.position.y,
            ctx.actor.position.z,
        );
        let world = &ctx.actor.world;

        for dy in -self.range..=self.range {
            for dx in -self.range..=self.range {
                for dz in -self.range..=self.range {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let pos = BlockPos::new(origin.0.x + dx, origin.0.y + dy, origin.0.z + dz);
                    let below = BlockPos::new(pos.0.x, pos.0.y - 1, pos.0.z);
                    // Standable: air here, no fluid, something solid underneath.
                    let clear = world.get_block(&pos).id == pumpkin_data::Block::AIR.id
                        && !TryFindWaterGoal::is_water(world, &pos);
                    let supported = world.get_block(&below).id != pumpkin_data::Block::AIR.id
                        && !TryFindWaterGoal::is_water(world, &below);
                    if clear && supported {
                        ctx.memories.set(
                            MemoryModuleType::LookTarget,
                            MemoryValue::Vec3(pos.0.to_f64()),
                        );
                        set_walk_target(ctx.memories, pos.0.to_f64(), self.speed);
                        return;
                    }
                }
            }
        }
    }

    fn debug_name(&self) -> &'static str {
        "try_find_land"
    }
}
