use super::{Controls, Goal, to_goal_ticks};
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use pumpkin_data::{Block, BlockId};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;
use rand::RngExt;
use std::sync::atomic::Ordering::Relaxed;

/// Host block -> infested block, mirroring vanilla's `InfestedBlock` registrations
/// (`BLOCK_BY_HOST_BLOCK`, InfestedBlock.java:28,40 — one entry per infested variant
/// constructed in `Blocks.java`). None of these blocks carry extra state properties, so
/// `defaultBlockState()` (InfestedBlock.java:69) needs no property copying. Shared with
/// `SilverfishWakeUpFriendsGoal`.
pub(crate) fn infested_block_for_host(host_id: BlockId) -> Option<&'static Block> {
    match host_id {
        BlockId::STONE => Some(&Block::INFESTED_STONE),
        BlockId::COBBLESTONE => Some(&Block::INFESTED_COBBLESTONE),
        BlockId::STONE_BRICKS => Some(&Block::INFESTED_STONE_BRICKS),
        BlockId::MOSSY_STONE_BRICKS => Some(&Block::INFESTED_MOSSY_STONE_BRICKS),
        BlockId::CRACKED_STONE_BRICKS => Some(&Block::INFESTED_CRACKED_STONE_BRICKS),
        BlockId::CHISELED_STONE_BRICKS => Some(&Block::INFESTED_CHISELED_STONE_BRICKS),
        _ => None,
    }
}

/// Reverse of [`infested_block_for_host`], mirroring `InfestedBlock.hostStateByInfested`
/// (InfestedBlock.java:72-74). Shared with `SilverfishWakeUpFriendsGoal`.
pub(crate) fn host_block_for_infested(infested_id: BlockId) -> Option<&'static Block> {
    match infested_id {
        BlockId::INFESTED_STONE => Some(&Block::STONE),
        BlockId::INFESTED_COBBLESTONE => Some(&Block::COBBLESTONE),
        BlockId::INFESTED_STONE_BRICKS => Some(&Block::STONE_BRICKS),
        BlockId::INFESTED_MOSSY_STONE_BRICKS => Some(&Block::MOSSY_STONE_BRICKS),
        BlockId::INFESTED_CRACKED_STONE_BRICKS => Some(&Block::CRACKED_STONE_BRICKS),
        BlockId::INFESTED_CHISELED_STONE_BRICKS => Some(&Block::CHISELED_STONE_BRICKS),
        _ => None,
    }
}

/// Port of `Silverfish.SilverfishMergeWithStoneGoal` (Silverfish.java:126-180): a
/// `RandomStrollGoal(mob, 1.0, 10)` (Silverfish.java:131) that, on each attempt, first
/// rolls a `1/reducedTickDelay(10)` chance (gated on the `mobGriefing` game rule,
/// Silverfish.java:146) to pick one of the six neighboring blocks; if that block can host
/// silverfish, the goal "merges" into it instead of wandering (Silverfish.java:166-179):
/// replacing the host block with its infested state and discarding itself. Otherwise it
/// falls back to `RandomStrollGoal`'s own `canUse()`/`start()` (Silverfish.java:157,
/// RandomStrollGoal.java:36-76).
///
/// Vanilla's `spawnAnim()` call on a successful merge (Silverfish.java:175) is a
/// client-only particle/animation trigger with no equivalent hook in Pumpkin's entity API
/// (grepped for `spawn_anim`/entity-event broadcasting — nothing exists), so it is
/// intentionally omitted here; the block conversion and despawn still happen.
pub struct SilverfishMergeWithStoneGoal {
    target: Option<Vector3<f64>>,
    do_merge: bool,
    merge_pos: Option<BlockPos>,
}

impl SilverfishMergeWithStoneGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            target: None,
            do_merge: false,
            merge_pos: None,
        }
    }
}

impl Default for SilverfishMergeWithStoneGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal for SilverfishMergeWithStoneGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();

        // Silverfish.java:137-139: `if (this.mob.getTarget() != null) return false;`
        if mob_entity.get_target().is_some() {
            return false;
        }

        // Silverfish.java:141-143: `if (!this.mob.getNavigation().isDone()) return false;`
        let is_idle = mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle();
        if !is_idle {
            return false;
        }

        let entity = mob.get_entity();
        let world = entity.world.load_full();

        // Silverfish.java:146-153.
        if world.level_info.load().game_rules.mob_griefing
            && mob.get_random().random_range(0..to_goal_ticks(10).max(1)) == 0
        {
            let base = (entity.pos.load() + Vector3::new(0.0, 0.5, 0.0)).to_block_pos();
            let neighbors = [
                base.up(),
                base.down(),
                base.north(),
                base.south(),
                base.east(),
                base.west(),
            ];
            let candidate = neighbors[mob.get_random().random_range(0..6)];
            let block = world.get_block(&candidate);
            if infested_block_for_host(block.id).is_some() {
                self.do_merge = true;
                self.merge_pos = Some(candidate);
                return true;
            }
        }

        self.do_merge = false;

        // Silverfish.java:157: `return super.canUse();` -> RandomStrollGoal.canUse()
        // (RandomStrollGoal.java:37-62), with `checkNoActionTime=true` (default) and
        // `interval=10` (Silverfish.java:131).
        if entity.has_passengers() {
            return false;
        }
        if mob_entity.no_action_time.load(Relaxed) >= 100 {
            return false;
        }
        if mob.get_random().random_range(0..to_goal_ticks(10).max(1)) != 0 {
            return false;
        }
        self.target = WanderAroundGoal::find_ground_target(mob, 10, 7, false);
        self.target.is_some()
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        // Silverfish.java:161-163.
        if self.do_merge {
            return false;
        }
        let mob_entity = mob.get_mob_entity();
        !mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle()
            && !mob.get_entity().has_passengers()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if !self.do_merge {
            // RandomStrollGoal.java:74-76.
            if let Some(target) = self.target {
                let mob_entity = mob.get_mob_entity();
                let mob_pos = mob_entity.living_entity.entity.pos.load();
                mob_entity
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(mob_pos, target, 1.0));
            }
            return;
        }

        // Silverfish.java:169-178.
        let Some(pos) = self.merge_pos else {
            return;
        };
        let entity = mob.get_entity();
        let world = entity.world.load_full();
        let block = world.get_block(&pos);
        if let Some(infested) = infested_block_for_host(block.id) {
            world.set_block_state(&pos, infested.default_state.id, BlockFlags::NOTIFY_ALL);
            entity.remove();
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        // RandomStrollGoal.java:79-82: `this.mob.getNavigation().stop();` unconditionally.
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.target = None;
        self.merge_pos = None;
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
