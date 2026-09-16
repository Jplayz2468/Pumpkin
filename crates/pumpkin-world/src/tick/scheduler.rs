use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
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
/// Each trigger bucket is a priority heap. Inactive chunks retain overdue ticks;
/// collection merges eligible chunk heads without reordering a chunk by priority
/// ahead of an earlier trigger time. Unprocessed ticks remain queued at the budget.
pub struct ChunkTickScheduler<T> {
    inner: Mutex<Option<Box<ChunkTickSchedulerInner<T>>>>,
    /// Absolute tick this chunk's queue has advanced to. Monotonic; never wraps.
    current_tick: AtomicU64,
}

struct ChunkTickSchedulerInner<T> {
    tick_queue: BTreeMap<u64, BinaryHeap<Reverse<OrderedTick<T>>>>,
    queued_ticks: FxHashSet<(BlockPos, T)>,
}

impl<'a, T: std::hash::Hash + Eq> ChunkTickScheduler<&'a T> {
    pub fn step_tick(&self) -> Vec<OrderedTick<&'a T>> {
        let due_at = self.advance_tick();
        let mut due = Vec::new();
        while let Some(tick) = self.poll_due(due_at) {
            due.push(tick);
        }
        due
    }

    fn advance_tick(&self) -> u64 {
        self.current_tick.fetch_add(1, Ordering::SeqCst)
    }

    fn peek_due(&self, due_at: u64) -> Option<(crate::tick::TickPriority, i64)> {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inner = guard.as_ref()?;
        let (trigger, queue) = inner.tick_queue.first_key_value()?;
        if *trigger > due_at {
            return None;
        }
        let Reverse(tick) = queue.peek()?;
        Some((tick.priority, tick.sub_tick_order))
    }

    fn poll_due(&self, due_at: u64) -> Option<OrderedTick<&'a T>> {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inner = guard.as_mut()?;
        let mut entry = inner.tick_queue.first_entry()?;
        if *entry.key() > due_at {
            return None;
        }
        let Reverse(tick) = entry.get_mut().pop()?;
        if entry.get().is_empty() {
            entry.remove();
        }
        inner.queued_ticks.remove(&(tick.position, tick.value));
        if inner.queued_ticks.is_empty() {
            *guard = None;
        }
        Some(tick)
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
            inner
                .tick_queue
                .entry(trigger)
                .or_default()
                .push(Reverse(OrderedTick {
                    priority: tick.priority,
                    sub_tick_order,
                    position: tick.position,
                    value: tick.value,
                }));
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
            queue.retain(|Reverse(tick)| !contains(&tick.position));
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
            ordered.extend(queue.iter().map(|Reverse(tick)| (tick, delay)));
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

/// LevelTicks' container merge: choose a chunk by its head's priority/sequence,
/// while keeping each chunk's trigger-time order. Inactive queues still age.
pub fn collect_ticks<T: std::hash::Hash + Eq + 'static>(
    queues: &[(&ChunkTickScheduler<&'static T>, bool)],
    limit: usize,
) -> Vec<OrderedTick<&'static T>> {
    let mut ready = BinaryHeap::new();
    let mut cutoffs = Vec::with_capacity(queues.len());
    for (index, (queue, active)) in queues.iter().enumerate() {
        let cutoff = queue.advance_tick();
        cutoffs.push(cutoff);
        if *active && let Some((priority, order)) = queue.peek_due(cutoff) {
            ready.push(Reverse((priority, order, index)));
        }
    }
    let mut result = Vec::new();
    while result.len() < limit {
        let Some(Reverse((_, _, index))) = ready.pop() else {
            break;
        };
        let (queue, _) = queues[index];
        if let Some(tick) = queue.poll_due(cutoffs[index]) {
            result.push(tick);
        }
        if let Some((priority, order)) = queue.peek_due(cutoffs[index]) {
            ready.push(Reverse((priority, order, index)));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tick::TickPriority;

    #[test]
    fn java_container_merge_activation_and_budget_traces() {
        // Unmodified 26.2 LevelTicks: 128 seeded cases, 48 scheduled ticks,
        // four chunks and 25 activation/budget changes per case.
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("chunk_tick_cases.json")).unwrap();
        for (case_index, case) in cases.as_array().unwrap().iter().enumerate() {
            let queues: [ChunkTickScheduler<&u8>; 4] =
                std::array::from_fn(|_| ChunkTickScheduler::default());
            for tick in case["ticks"].as_array().unwrap() {
                let number = |i: usize| tick[i].as_i64().unwrap();
                queues[number(0) as usize].schedule_tick(
                    &ScheduledTick {
                        position: BlockPos::new(number(1) as i32, number(2) as i32, 0),
                        delay: number(3) as u32,
                        priority: TickPriority::try_from(number(4) as i32).unwrap(),
                        value: &0u8,
                    },
                    number(5),
                );
            }
            for (time, step) in case["steps"].as_array().unwrap().iter().enumerate() {
                let mask = step["mask"].as_u64().unwrap();
                let active: Vec<_> = queues
                    .iter()
                    .enumerate()
                    .map(|(i, q)| (q, mask & (1 << i) != 0))
                    .collect();
                let actual = collect_ticks(&active, step["budget"].as_u64().unwrap() as usize);
                let orders: Vec<_> = actual.iter().map(|t| t.sub_tick_order).collect();
                let expected: Vec<_> = step["output"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_i64().unwrap())
                    .collect();
                assert_eq!(orders, expected, "case {case_index}, time {time}");
            }
            assert!(queues.iter().all(|q| !q.has_ticks()));
        }
    }

    #[test]
    fn inactive_and_budget_deferred_ticks_remain_deduplicated() {
        let queue = ChunkTickScheduler::default();
        let tick = ScheduledTick {
            delay: 0,
            priority: TickPriority::Normal,
            position: BlockPos::new(1, 64, 1),
            value: &0u8,
        };
        queue.schedule_tick(&tick, 0);
        assert!(collect_ticks(&[(&queue, false)], 65536).is_empty());
        queue.schedule_tick(&tick, 1);
        assert!(collect_ticks(&[(&queue, true)], 0).is_empty());
        assert!(queue.is_scheduled(tick.position, tick.value));
        let due = collect_ticks(&[(&queue, true)], 1);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].sub_tick_order, 0);
        assert!(!queue.is_scheduled(tick.position, tick.value));
        queue.schedule_tick(&tick, 2);
        assert_eq!(collect_ticks(&[(&queue, true)], 1)[0].sub_tick_order, 2);
    }

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
