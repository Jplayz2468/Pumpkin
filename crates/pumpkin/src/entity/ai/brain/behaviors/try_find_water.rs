use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::goal::try_find_water::TryFindWaterGoal;
use pumpkin_util::math::position::BlockPos;

/// Port of `TryFindWater`.
///
/// Sends a stranded aquatic mob back toward water. Vanilla prefers a water block with air
/// above it -- somewhere the mob can actually surface -- and falls back to any water block
/// that is not immediately underfoot.
pub struct TryFindWater {
    range: i32,
    speed: f32,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl TryFindWater {
    #[must_use]
    pub fn new(range: i32, speed: f32) -> Self {
        Self {
            range,
            speed,
            conditions: [
                (MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
            ],
        }
    }
}

impl Behavior<MobActor> for TryFindWater {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // Already in water: nothing to look for.
        !ctx.actor.entity().is_some_and(|entity| {
            entity
                .get_entity()
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let origin = BlockPos::floored(
            ctx.actor.position.x,
            ctx.actor.position.y,
            ctx.actor.position.z,
        );
        let world = &ctx.actor.world;

        let mut best: Option<BlockPos> = None;
        let mut alternate: Option<BlockPos> = None;

        for dx in -self.range..=self.range {
            for dy in -self.range..=self.range {
                for dz in -self.range..=self.range {
                    // Vanilla skips the mob's own column.
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let pos = BlockPos::new(origin.0.x + dx, origin.0.y + dy, origin.0.z + dz);
                    if !TryFindWaterGoal::is_water(world, &pos) {
                        continue;
                    }
                    let above = BlockPos::new(pos.0.x, pos.0.y + 1, pos.0.z);
                    if world.get_block(&above).id == pumpkin_data::Block::AIR.id {
                        best = Some(pos);
                        break;
                    }
                    if alternate.is_none() {
                        alternate = Some(pos);
                    }
                }
                if best.is_some() {
                    break;
                }
            }
            if best.is_some() {
                break;
            }
        }

        if let Some(target) = best.or(alternate) {
            set_walk_target(ctx.memories, target.0.to_f64(), self.speed);
        }
    }

    fn debug_name(&self) -> &'static str {
        "try_find_water"
    }
}
