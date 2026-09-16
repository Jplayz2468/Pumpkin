use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_util::math::position::BlockPos;

/// Port of `TryLaySpawnOnFluidNearLand`.
///
/// A pregnant frog standing on land places frogspawn on an adjacent water surface. This
/// is the first brain behaviour here that changes the world rather than the mob.
pub struct TryLaySpawn {
    spawn_block: &'static pumpkin_data::Block,
    conditions: [(MemoryModuleType, MemoryStatus); 3],
}

impl TryLaySpawn {
    #[must_use]
    pub fn new(spawn_block: &'static pumpkin_data::Block) -> Self {
        Self {
            spawn_block,
            conditions: [
                (MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::WalkTarget, MemoryStatus::ValuePresent),
                (MemoryModuleType::IsPregnant, MemoryStatus::ValuePresent),
            ],
        }
    }
}

impl Behavior<MobActor> for TryLaySpawn {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // Vanilla requires the frog to be out of water and on the ground.
        ctx.actor.entity().is_some_and(|entity| {
            let entity = entity.get_entity();
            !entity
                .touching_water
                .load(std::sync::atomic::Ordering::Relaxed)
                && entity.on_ground.load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let origin = BlockPos::floored(
            ctx.actor.position.x,
            ctx.actor.position.y,
            ctx.actor.position.z,
        );
        let below = BlockPos::new(origin.0.x, origin.0.y - 1, origin.0.z);
        let world = &ctx.actor.world;

        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let beside = BlockPos::new(below.0.x + dx, below.0.y, below.0.z + dz);
            let supports = world
                .get_block(&beside)
                .has_tag(&pumpkin_data::tag::Block::MINECRAFT_SUPPORTS_FROGSPAWN);
            if !supports {
                continue;
            }
            let spawn_pos = BlockPos::new(beside.0.x, beside.0.y + 1, beside.0.z);
            if world.get_block(&spawn_pos).id != pumpkin_data::Block::AIR.id {
                continue;
            }
            world.set_block_state(
                &spawn_pos,
                self.spawn_block.default_state.id,
                pumpkin_world::world::BlockFlags::NOTIFY_ALL,
            );
            world.play_sound(
                Sound::EntityFrogLaySpawn,
                SoundCategory::Blocks,
                &ctx.actor.position,
            );
            ctx.memories.erase(MemoryModuleType::IsPregnant);
            return;
        }
    }

    fn debug_name(&self) -> &'static str {
        "try_lay_spawn"
    }
}
