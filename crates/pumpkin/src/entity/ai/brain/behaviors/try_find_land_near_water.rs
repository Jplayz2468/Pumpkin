use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::goal::try_find_water::TryFindWaterGoal;
use pumpkin_util::math::position::BlockPos;

/// Vanilla's cooldown between scans.
const COOLDOWN_TICKS: i64 = 40;

/// Port of `TryFindLandNearWater`.
///
/// Finds a standable spot that has water beside it -- the shoreline a pregnant frog heads
/// for before laying spawn. Distinct from `TryFindLand`, which just wants dry ground.
pub struct TryFindLandNearWater {
    range: i32,
    speed: f32,
    next_ok_start_time: i64,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl TryFindLandNearWater {
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

impl Behavior<MobActor> for TryFindLandNearWater {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // Vanilla bails if the frog is already in water.
        !ctx.actor.entity().is_some_and(|entity| {
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
                    // Standable: clear here, solid underneath.
                    if world.get_block(&pos).id != pumpkin_data::Block::AIR.id
                        || world.get_block(&below).id == pumpkin_data::Block::AIR.id
                    {
                        continue;
                    }
                    // ...and water immediately beside it, one step down.
                    let beside_water = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(ox, oz)| {
                        let side = BlockPos::new(pos.0.x + ox, pos.0.y, pos.0.z + oz);
                        let side_below = BlockPos::new(side.0.x, side.0.y - 1, side.0.z);
                        world.get_block(&side).id == pumpkin_data::Block::AIR.id
                            && TryFindWaterGoal::is_water(world, &side_below)
                    });
                    if beside_water {
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
        "try_find_land_near_water"
    }
}
