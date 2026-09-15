use std::collections::BTreeMap;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;

use crate::tick::{OrderedTick, ScheduledTick};

/// Per-chunk scheduled-tick queue, keyed by absolute trigger tick.
///
/// This mirrors Java's `LevelChunkTicks`, where a `ScheduledTick` carries an absolute
/// `triggerTick` (`ScheduledTick.java`) and the queue is drained in `DRAIN_ORDER`
/// (trigger tick, then priority, then sub-tick order). An earlier implementation here
/// used a fixed 256-slot ring indexed by `(offset + delay) % 256`, which silently
/// capped every delay at 255 ticks -- vanilla routinely schedules far longer ones
/// (frogspawn hatches after 3600-12000 ticks, `FrogspawnBlock.java`; dried ghast
/// hydration steps every 5000, `DriedGhastBlock.java`). A `BTreeMap` keyed by trigger
/// tick has no such ceiling.
///
/// Because a drain removes exactly one trigger-tick bucket, the trigger time is implicit
/// within a bucket and `OrderedTick`'s `(priority, sub_tick_order)` ordering remains the
/// correct intra-tick comparator, unchanged from the ring implementation.
pub struct ChunkTickScheduler<T> {
    inner: Mutex<Option<Box<ChunkTickSchedulerInner<T>>>>,
    /// Absolute tick this chunk's queue has advanced to. Monotonic; never wraps.
    current_tick: AtomicU64,
}

struct ChunkTickSchedulerInner<T> {
    tick_queue: BTreeMap<u64, Vec<OrderedTick<T>>>,
    queued_ticks: FxHashSet<(BlockPos, T)>,
}

impl<'a, T: std::hash::Hash + Eq> ChunkTickScheduler<&'a T> {
    pub fn step_tick(&self) -> Vec<OrderedTick<&'a T>> {
        let due_at = self.current_tick.fetch_add(1, Ordering::SeqCst);

        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_mut() else {
            return Vec::new();
        };

        let res = inner.tick_queue.remove(&due_at).unwrap_or_default();

        if !res.is_empty() {
            for next_tick in &res {
                inner
                    .queued_ticks
                    .remove(&(next_tick.position, next_tick.value));
            }
            if inner.queued_ticks.is_empty() {
                *inner_guard = None;
            }
        }
        res
    }

    pub fn schedule_tick(&self, tick: &ScheduledTick<&'a T>, sub_tick_order: i64) {
        let trigger = self
            .current_tick
            .load(Ordering::SeqCst)
            .saturating_add(u64::from(tick.delay));
        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inner = inner_guard.get_or_insert_with(|| {
            Box::new(ChunkTickSchedulerInner {
                tick_queue: BTreeMap::new(),
                queued_ticks: FxHashSet::default(),
            })
        });

        if inner.queued_ticks.insert((tick.position, tick.value)) {
            inner.tick_queue.entry(trigger).or_default().push(OrderedTick {
                priority: tick.priority,
                sub_tick_order,
                position: tick.position,
                value: tick.value,
            });
        }
    }

    pub fn is_scheduled(&self, pos: BlockPos, value: &T) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|inner| inner.queued_ticks.contains(&(pos, value)))
    }

    pub fn clear_area(&self, min: &BlockPos, max: &BlockPos) {
        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_mut() else {
            return;
        };
        let contains = |position: &BlockPos| {
            position.0.x >= min.0.x
                && position.0.x < max.0.x
                && position.0.y >= min.0.y
                && position.0.y < max.0.y
                && position.0.z >= min.0.z
                && position.0.z < max.0.z
        };

        for queue in inner.tick_queue.values_mut() {
            queue.retain(|tick| !contains(&tick.position));
        }
        inner.tick_queue.retain(|_, queue| !queue.is_empty());
        inner
            .queued_ticks
            .retain(|(position, _)| !contains(position));
        let became_empty = inner.queued_ticks.is_empty();

        if became_empty {
            *inner_guard = None;
        }
    }

    pub fn has_ticks(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|inner| !inner.queued_ticks.is_empty())
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<ScheduledTick<&'a T>> {
        let now = self.current_tick.load(Ordering::SeqCst);
        let inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_ref() else {
            return Vec::new();
        };

        let mut ordered = Vec::with_capacity(inner.queued_ticks.len());
        for (trigger, queue) in &inner.tick_queue {
            // Saved delays are relative to now, as `ScheduledTick.toSavedTick` does.
            let delay = trigger.saturating_sub(now) as u32;
            ordered.extend(queue.iter().map(|tick| (tick, delay)));
        }
        // Java LevelChunkTicks::pack saves by sequence, not by trigger time.
        // Restoring the saved list then preserves its relative sequence numbers.
        ordered.sort_unstable_by_key(|(tick, _)| tick.sub_tick_order);
        ordered
            .into_iter()
            .map(|(tick, delay)| ScheduledTick {
                delay,
                priority: tick.priority,
                position: tick.position,
                value: tick.value,
            })
            .collect()
    }
}

impl<'a, T: std::hash::Hash + Eq + 'static> FromIterator<ScheduledTick<&'a T>>
    for ChunkTickScheduler<&'a T>
{
    fn from_iter<I: IntoIterator<Item = ScheduledTick<&'a T>>>(iter: I) -> Self {
        let scheduler = Self::default();
        // Java LevelChunkTicks::unpack numbers restored ticks from -N to -1.
        // They must sort before newly scheduled ticks, whose level counter starts at zero.
        let ticks: Vec<_> = iter.into_iter().collect();
        let mut sub_tick_order = -(ticks.len() as i64);
        let iter = ticks.into_iter();

        let (lower, _) = iter.size_hint();
        if lower > 0 {
            let mut inner_guard = scheduler
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let inner = inner_guard.get_or_insert_with(|| {
                Box::new(ChunkTickSchedulerInner {
                    tick_queue: BTreeMap::new(),
                    queued_ticks: FxHashSet::default(),
                })
            });
            inner.queued_ticks.reserve(lower);
        }

        for tick in iter {
            scheduler.schedule_tick(&tick, sub_tick_order);
            sub_tick_order += 1;
        }
        scheduler
    }
}

impl<T> Default for ChunkTickScheduler<T> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
            current_tick: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tick::TickPriority;

    #[test]
    fn restored_ticks_precede_new_ticks_across_chunk_collection_orders() {
        // Java 26.2 LevelTicks oracle: saved X=0 precedes fresh X=16,
        // regardless of chunk registration or collection order.
        for reverse in [false, true] {
            let mut tick = ScheduledTick {
                delay: 2,
                priority: TickPriority::Normal,
                position: BlockPos::new(0, 64, 0),
                value: &0u8,
            };
            let restored: ChunkTickScheduler<&u8> = [tick.clone()].into_iter().collect();
            let fresh = ChunkTickScheduler::default();
            tick.position = BlockPos::new(16, 64, 0);
            fresh.schedule_tick(&tick, 0);
            for _ in 0..2 {
                assert!(restored.step_tick().is_empty());
                assert!(fresh.step_tick().is_empty());
            }
            let mut due = if reverse {
                let mut due = restored.step_tick();
                due.extend(fresh.step_tick());
                due
            } else {
                let mut due = fresh.step_tick();
                due.extend(restored.step_tick());
                due
            };
            due.sort_unstable();
            assert_eq!(
                due.iter()
                    .map(|t| (t.position.0.x, t.sub_tick_order))
                    .collect::<Vec<_>>(),
                [(0, -1), (16, 0)]
            );
        }
    }

    #[test]
    fn restored_sequence_preserves_saved_list_order() {
        let ticks = [9, 3, 7].map(|x| ScheduledTick {
            delay: 0,
            priority: TickPriority::Normal,
            position: BlockPos::new(x, 64, 0),
            value: &0u8,
        });
        let queue: ChunkTickScheduler<&u8> = ticks.into_iter().collect();
        let mut due = queue.step_tick();
        due.sort_unstable();
        assert_eq!(
            due.iter()
                .map(|t| (t.position.0.x, t.sub_tick_order))
                .collect::<Vec<_>>(),
            [(9, -3), (3, -2), (7, -1)]
        );
    }
    #[test]
    fn saving_keeps_sequence_order_and_remaining_delays() {
        let queue = ChunkTickScheduler::default();
        for (order, delay) in [20, 2, 1].into_iter().enumerate() {
            queue.schedule_tick(
                &ScheduledTick {
                    delay,
                    priority: TickPriority::Normal,
                    position: BlockPos::new(order as i32, 64, 0),
                    value: &0u8,
                },
                order as i64,
            );
        }
        let saved = || {
            queue
                .to_vec()
                .iter()
                .map(|tick| (tick.position.0.x, tick.delay))
                .collect::<Vec<_>>()
        };
        assert_eq!(saved(), [(0, 20), (1, 2), (2, 1)]);
        queue.step_tick();
        queue.step_tick();
        assert_eq!(saved(), [(0, 18), (1, 0)]);
    }

    #[test]
    fn long_delays_survive_beyond_the_old_ring_size() {
        // The previous 256-slot ring made a 5000-tick delay fire at 5000 % 256 = 136.
        // Vanilla schedules delays this long routinely (DriedGhastBlock hydration = 5000).
        let queue = ChunkTickScheduler::default();
        queue.schedule_tick(
            &ScheduledTick {
                delay: 5000,
                priority: TickPriority::Normal,
                position: BlockPos::new(0, 64, 0),
                value: &0u8,
            },
            0,
        );
        for _ in 0..5000 {
            assert!(queue.step_tick().is_empty());
        }
        assert_eq!(queue.step_tick().len(), 1);
    }
}
