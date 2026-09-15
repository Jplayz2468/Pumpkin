use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

use super::{Controls, Goal};
use crate::entity::ai::goal::silverfish_merge_with_stone::host_block_for_infested;
use crate::entity::mob::Mob;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

/// Zigzag offset sequence matching vanilla's `for (int off = 0; off <= bound && off >=
/// -bound; off = (off <= 0 ? 1 : 0) - off)` (Silverfish.java:209-211): `0, 1, -1, 2, -2,
/// ..., bound, -bound`.
fn zigzag_offsets(bound: i32) -> Vec<i32> {
    let mut offsets = Vec::with_capacity((2 * bound + 1) as usize);
    let mut v = 0;
    while (-bound..=bound).contains(&v) {
        offsets.push(v);
        v = if v <= 0 { 1 - v } else { -v };
    }
    offsets
}

/// Port of `Silverfish.SilverfishWakeUpFriendsGoal` (Silverfish.java:182-230). Vanilla
/// constructs a single instance per silverfish, held both in the goal selector
/// (Silverfish.java:45) and directly on the entity so `hurtServer` can call
/// `notifyHurt()` on it (Silverfish.java:87-88). Pumpkin's `SilverfishEntity` cannot hand
/// out a live reference into its own `goals_selector` (goals are boxed and owned by the
/// selector), so the trigger is a shared counter instead: `SilverfishEntity::on_damage`
/// (silverfish.rs) sets `look_for_friends` and this goal reads/decrements the same
/// `Arc<AtomicI32>`.
pub struct SilverfishWakeUpFriendsGoal {
    look_for_friends: Arc<AtomicI32>,
}

impl SilverfishWakeUpFriendsGoal {
    #[must_use]
    pub const fn new(look_for_friends: Arc<AtomicI32>) -> Self {
        Self { look_for_friends }
    }
}

impl Goal for SilverfishWakeUpFriendsGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        // Silverfish.java:196-199: `canUse()` doesn't consume the counter, only tick() does.
        self.look_for_friends.load(Relaxed) > 0
    }

    fn should_continue(&self, _mob: &dyn Mob) -> bool {
        // Not overridden in vanilla, so `canContinueToUse()` falls back to `canUse()`
        // (Goal.java default).
        self.look_for_friends.load(Relaxed) > 0
    }

    fn tick(&mut self, mob: &dyn Mob) {
        // Silverfish.java:202-204: `this.lookForFriends--; if (this.lookForFriends <= 0)
        // { ... }`.
        let remaining = self.look_for_friends.fetch_sub(1, Relaxed) - 1;
        if remaining > 0 {
            return;
        }
        self.look_for_friends.store(0, Relaxed);

        let entity = mob.get_entity();
        let world = entity.world.load_full();
        let mob_griefing = world.level_info.load().game_rules.mob_griefing;
        let base = entity.block_pos.load();
        let mut rng = mob.get_random();

        // Silverfish.java:209-227: scan a (-10..=10, -5..=5, -10..=10) volume around the
        // silverfish in zigzag order for infested blocks, revealing (or, with
        // `mobGriefing`, breaking) at most a handful before stopping on a coin flip.
        for y_off in zigzag_offsets(5) {
            for x_off in zigzag_offsets(10) {
                for z_off in zigzag_offsets(10) {
                    let pos = base.offset(Vector3::new(x_off, y_off, z_off));
                    let block = world.get_block(&pos);
                    let Some(host) = host_block_for_infested(block.id) else {
                        continue;
                    };

                    if mob_griefing {
                        // Silverfish.java:215: `level.destroyBlock(testPos, true,
                        // this.silverfish);`. Vanilla's break here can itself spawn a
                        // fresh Silverfish via `InfestedBlock.spawnAfterBreak`
                        // (InfestedBlock.java:61-65), but Pumpkin's block `broken()` hook
                        // requires a player cause (`BrokenArgs.player: &Arc<Player>`,
                        // block/mod.rs) and is never invoked for a mob-caused break, so
                        // that secondary spawn does not happen here.
                        world.break_block(&pos, None, BlockFlags::empty());
                    } else {
                        // Silverfish.java:217: `level.setBlock(testPos,
                        // infestedBlock.hostStateByInfested(...), 3);`
                        world.set_block_state(
                            &pos,
                            host.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        );
                    }

                    // Silverfish.java:221-223: `if (random.nextBoolean()) return;`
                    if rng.random_bool(0.5) {
                        return;
                    }
                }
            }
        }
    }

    fn controls(&self) -> Controls {
        // Silverfish.java:182-230: `SilverfishWakeUpFriendsGoal` never calls `setFlags`,
        // so it claims no controls (Goal.java default: empty `EnumSet`).
        Controls::empty()
    }
}
