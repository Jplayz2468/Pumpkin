use crate::entity::experience_orb::ExperienceOrbEntity;
use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, SpawnAfterBreakArgs};

#[pumpkin_block("minecraft:spawner")]
pub struct SpawnerBlock;

impl BlockBehaviour for SpawnerBlock {
    fn spawn_after_break(&self, args: SpawnAfterBreakArgs<'_>) {
        if args.experience {
            let amount = 15 + args.world.rand_bounded_i32(15) + args.world.rand_bounded_i32(15);
            let mut event = crate::plugin::block::block_exp::BlockExpEvent {
                block_pos: *args.position,
                world: args.world.clone(),
                exp: amount,
            };
            if let Some(server) = args.world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.exp > 0 && args.world.level_info.load().game_rules.block_drops {
                ExperienceOrbEntity::spawn(
                    args.world,
                    args.position.to_centered_f64(),
                    event.exp as u32,
                );
            }
        }
    }
}
