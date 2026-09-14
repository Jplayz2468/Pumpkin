//! Port of vanilla `CollectingNeighborUpdater`
//! (`net/minecraft/world/level/redstone/CollectingNeighborUpdater.java`).
//!
//! Vanilla does **not** run neighbor updates immediately. An update raised while another
//! is already running is appended to `addedThisLayer` instead of being recursed into;
//! `runUpdates` then drains layer by layer. That queueing is what determines the order
//! chained redstone updates resolve in, so matching it is a prerequisite for redstone
//! parity -- not an optimization.
//!
//! It also bounds the work: vanilla caps chained updates at `max-chained-neighbor-updates`
//! (server.properties, default 1000000) and, being a flat queue rather than recursion,
//! cannot overflow the stack on a deep chain.
//!
//! Concurrency note: the queue lives behind a `Mutex`, but the lock is **never** held
//! while a block's update handler runs. Handlers re-enter through `neighbor_changed` and
//! take the lock themselves; holding it across execution would deadlock the non-reentrant
//! `std::sync::Mutex` immediately.

use std::sync::{Arc, Mutex, PoisonError};

use pumpkin_data::{Block, BlockDirection, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use tracing::error;

use crate::world::World;

/// Vanilla `MinecraftServer.getMaxChainedNeighborUpdates` (`MinecraftServer.java:2059`),
/// overridable in vanilla via the `max-chained-neighbor-updates` server property.
const DEFAULT_MAX_CHAINED_NEIGHBOR_UPDATES: u32 = 1_000_000;

/// Vanilla `Block.updateOrDestroy` recursion limit (`Block.java:215`).
pub const DEFAULT_UPDATE_LIMIT: u32 = 512;

/// One queued update. Mirrors vanilla's `NeighborUpdates` implementations (`CollectingNeighborUpdater.java`).
#[derive(Debug)]
enum NeighborUpdate {
    /// Vanilla `SimpleNeighborUpdate`: notify exactly one position, then finish.
    Simple { pos: BlockPos, source_block: BlockId },
    /// Vanilla `MultiNeighborUpdate`: walk `UPDATE_ORDER`, one direction per step.
    Multi {
        source_pos: BlockPos,
        source_block: BlockId,
        skip: Option<BlockDirection>,
        idx: usize,
    },
    /// Vanilla `ShapeUpdate`: execute shape update on `pos` given neighbor's state, then finish.
    Shape {
        direction: BlockDirection,
        pos: BlockPos,
        neighbor_pos: BlockPos,
        neighbor_state: BlockStateId,
        flags: BlockFlags,
        update_limit: u32,
    },
}

/// The single notification produced by one step, executed with no lock held.
enum Step {
    Neighbor {
        pos: BlockPos,
        source_block: BlockId,
    },
    Shape {
        direction: BlockDirection,
        pos: BlockPos,
        neighbor_pos: BlockPos,
        neighbor_state: BlockStateId,
        flags: BlockFlags,
        update_limit: u32,
    },
}

impl NeighborUpdate {
    fn multi(source_pos: BlockPos, source_block: BlockId, skip: Option<BlockDirection>) -> Self {
        // Vanilla's constructor skips the first direction up front when it is the
        // skipped face, so the first `runNext` starts on a direction it will use.
        let order = BlockDirection::update_order();
        let idx = usize::from(skip == Some(order[0]));
        Self::Multi {
            source_pos,
            source_block,
            skip,
            idx,
        }
    }

    /// Vanilla `runNext`, split in two: this advances the entry's cursor and returns the
    /// work to do, and the caller performs it after releasing the lock. Returns the step
    /// plus whether this entry is now exhausted (vanilla's `runNext` returning `false`).
    ///
    /// The split is sound because vanilla's `runNext` advances `idx` *before* executing
    /// and the execution never reads `idx`.
    fn next_step(&mut self) -> (Step, bool) {
        match self {
            Self::Simple { pos, source_block } => (
                Step::Neighbor {
                    pos: *pos,
                    source_block: *source_block,
                },
                true,
            ),
            Self::Multi {
                source_pos,
                source_block,
                skip,
                idx,
            } => {
                let order = BlockDirection::update_order();
                let direction = order[*idx];
                *idx += 1;

                let step = Step::Neighbor {
                    pos: source_pos.offset(direction.to_offset()),
                    source_block: *source_block,
                };

                // Vanilla applies the skip adjustment after executing, but it only
                // affects the exhaustion result, so doing it here is equivalent.
                if *idx < order.len() && Some(order[*idx]) == *skip {
                    *idx += 1;
                }

                (step, *idx >= order.len())
            }
            Self::Shape {
                direction,
                pos,
                neighbor_pos,
                neighbor_state,
                flags,
                update_limit,
            } => (
                Step::Shape {
                    direction: *direction,
                    pos: *pos,
                    neighbor_pos: *neighbor_pos,
                    neighbor_state: *neighbor_state,
                    flags: *flags,
                    update_limit: *update_limit,
                },
                true,
            ),
        }
    }
}

#[derive(Default)]
struct UpdaterState {
    /// Vanilla's `ArrayDeque` used as a stack; the back of this `Vec` is its head.
    stack: Vec<NeighborUpdate>,
    added_this_layer: Vec<NeighborUpdate>,
    count: u32,
}

pub struct CollectingNeighborUpdater {
    state: Mutex<UpdaterState>,
    max_chained_neighbor_updates: u32,
}

impl Default for CollectingNeighborUpdater {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CHAINED_NEIGHBOR_UPDATES)
    }
}

impl CollectingNeighborUpdater {
    #[must_use]
    pub fn new(max_chained_neighbor_updates: u32) -> Self {
        Self {
            state: Mutex::new(UpdaterState::default()),
            max_chained_neighbor_updates,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, UpdaterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Vanilla `neighborChanged(pos, block, orientation)`.
    pub fn neighbor_changed(
        &self,
        world: &Arc<World>,
        pos: BlockPos,
        source_block: BlockId,
    ) {
        self.add_and_run(world, pos, NeighborUpdate::Simple { pos, source_block });
    }

    /// Vanilla `CollectingNeighborUpdater.shapeUpdate(direction, neighborState, pos, neighborPos, updateFlags, updateLimit)`.
    pub fn shape_update(
        &self,
        world: &Arc<World>,
        direction: BlockDirection,
        pos: BlockPos,
        neighbor_pos: BlockPos,
        neighbor_state: BlockStateId,
        flags: BlockFlags,
        update_limit: u32,
    ) {
        self.add_and_run(
            world,
            pos,
            NeighborUpdate::Shape {
                direction,
                pos,
                neighbor_pos,
                neighbor_state,
                flags,
                update_limit,
            },
        );
    }

    /// Vanilla `updateNeighborsAtExceptFromFacing(pos, block, skipDirection, orientation)`.
    pub fn update_neighbors_at_except_from_facing(
        &self,
        world: &Arc<World>,
        source_pos: BlockPos,
        source_block: BlockId,
        skip: Option<BlockDirection>,
    ) {
        self.add_and_run(
            world,
            source_pos,
            NeighborUpdate::multi(source_pos, source_block, skip),
        );
    }

    /// Vanilla `CollectingNeighborUpdater.addAndRun`.
    fn add_and_run(&self, world: &Arc<World>, pos: BlockPos, update: NeighborUpdate) {
        let running_already = {
            let mut state = self.lock();
            let running_already = state.count > 0;
            let too_many_updates = state.count >= self.max_chained_neighbor_updates;
            state.count += 1;

            if too_many_updates {
                if state.count - 1 == self.max_chained_neighbor_updates {
                    error!(
                        "Too many chained neighbor updates. Skipping the rest. First skipped position: {}, {}, {}",
                        pos.0.x, pos.0.y, pos.0.z
                    );
                }
            } else if running_already {
                state.added_this_layer.push(update);
            } else {
                state.stack.push(update);
            }

            running_already
        };

        if !running_already {
            self.run_updates(world);
        }
    }

    /// Vanilla `CollectingNeighborUpdater.runUpdates`.
    fn run_updates(&self, world: &Arc<World>) {
        loop {
            {
                let mut state = self.lock();
                // Vanilla pushes `addedThisLayer` onto the stack back-to-front, so the
                // first-added entry ends up on top and runs next.
                while let Some(update) = state.added_this_layer.pop() {
                    state.stack.push(update);
                }
                if state.stack.is_empty() {
                    break;
                }
            }

            // Vanilla: `while (this.addedThisLayer.isEmpty()) { if (!runNext(..)) { pop; break; } }`
            loop {
                let next = {
                    let mut state = self.lock();
                    if state.added_this_layer.is_empty() {
                        state.stack.last_mut().map(NeighborUpdate::next_step)
                    } else {
                        None
                    }
                };

                let Some((step, exhausted)) = next else {
                    break;
                };

                // Executed with the lock released: the handler re-enters through
                // `neighbor_changed`, which takes the lock again.
                Self::execute_update(world, &step);

                if exhausted {
                    self.lock().stack.pop();
                    break;
                }
            }
        }

        // Vanilla's `finally` block.
        let mut state = self.lock();
        state.stack.clear();
        state.added_this_layer.clear();
        state.count = 0;
    }

    /// Vanilla `NeighborUpdater.executeUpdate` and `NeighborUpdater.executeShapeUpdate`.
    fn execute_update(world: &Arc<World>, step: &Step) {
        match step {
            Step::Neighbor { pos, source_block } => {
                world.execute_neighbor_update(pos, Block::from_id(*source_block));
            }
            Step::Shape {
                direction,
                pos,
                neighbor_pos,
                neighbor_state,
                flags,
                update_limit,
            } => {
                world.execute_shape_update(
                    *direction,
                    pos,
                    neighbor_pos,
                    *neighbor_state,
                    *flags,
                    *update_limit,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `MultiNeighborUpdate` must visit vanilla's `UPDATE_ORDER` exactly once each, in
    /// order, and report exhaustion only on the final direction.
    #[test]
    fn multi_walks_update_order_once() {
        let mut update = NeighborUpdate::multi(BlockPos::new(0, 0, 0), Block::AIR.id, None);
        let mut visited = Vec::new();
        loop {
            let (step, exhausted) = update.next_step();
            match step {
                Step::Neighbor { pos, .. } => visited.push(pos),
                Step::Shape { .. } => panic!("expected Step::Neighbor"),
            }
            if exhausted {
                break;
            }
        }

        let expected: Vec<BlockPos> = BlockDirection::update_order()
            .iter()
            .map(|d| BlockPos::new(0, 0, 0).offset(d.to_offset()))
            .collect();
        assert_eq!(visited, expected);
    }

    /// A skipped face is never visited, whether it is first in the order or later.
    #[test]
    fn multi_honours_skip_direction() {
        let order = BlockDirection::update_order();
        for skip in order {
            let mut update =
                NeighborUpdate::multi(BlockPos::new(0, 0, 0), Block::AIR.id, Some(skip));
            let skipped = BlockPos::new(0, 0, 0).offset(skip.to_offset());
            let mut visited = Vec::new();
            loop {
                let (step, exhausted) = update.next_step();
                match step {
                    Step::Neighbor { pos, .. } => visited.push(pos),
                    Step::Shape { .. } => panic!("expected Step::Neighbor"),
                }
                if exhausted {
                    break;
                }
            }
            assert!(!visited.contains(&skipped), "visited the skipped face {skip:?}");
            assert_eq!(visited.len(), order.len() - 1);
        }
    }

    /// A simple update notifies one position and is immediately exhausted.
    #[test]
    fn simple_runs_once() {
        let pos = BlockPos::new(3, 4, 5);
        let mut update = NeighborUpdate::Simple {
            pos,
            source_block: Block::AIR.id,
        };
        let (step, exhausted) = update.next_step();
        match step {
            Step::Neighbor { pos: p, source_block } => {
                assert_eq!(p, pos);
                assert_eq!(source_block, Block::AIR.id);
            }
            Step::Shape { .. } => panic!("expected Step::Neighbor"),
        }
        assert!(exhausted);
    }

    /// A shape update notifies one position with the captured neighbor state and exhausts immediately.
    #[test]
    fn shape_runs_once_and_preserves_captured_state() {
        let pos = BlockPos::new(3, 4, 5);
        let neighbor_pos = BlockPos::new(3, 5, 5);
        let neighbor_state = BlockStateId::new_or_air(42);
        let flags = BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT;
        let mut update = NeighborUpdate::Shape {
            direction: BlockDirection::Up,
            pos,
            neighbor_pos,
            neighbor_state,
            flags,
            update_limit: DEFAULT_UPDATE_LIMIT,
        };
        let (step, exhausted) = update.next_step();
        match step {
            Step::Shape {
                direction,
                pos: p,
                neighbor_pos: np,
                neighbor_state: ns,
                flags: f,
                update_limit: ul,
            } => {
                assert_eq!(direction, BlockDirection::Up);
                assert_eq!(p, pos);
                assert_eq!(np, neighbor_pos);
                assert_eq!(ns, neighbor_state);
                assert_eq!(f, flags);
                assert_eq!(ul, DEFAULT_UPDATE_LIMIT);
            }
            Step::Neighbor { .. } => panic!("expected Step::Shape"),
        }
        assert!(exhausted);
    }
}
