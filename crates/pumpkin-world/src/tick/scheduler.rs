use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;

use crate::tick::{MAX_TICK_DELAY, OrderedTick, ScheduledTick};

pub struct ChunkTickScheduler<T> {
    inner: Mutex<Option<Box<ChunkTickSchedulerInner<T>>>>,
    offset: AtomicUsize,
}

struct ChunkTickSchedulerInner<T> {
    tick_queue: [Vec<OrderedTick<T>>; MAX_TICK_DELAY],
    queued_ticks: FxHashSet<(BlockPos, T)>,
}

impl<'a, T: std::hash::Hash + Eq> ChunkTickScheduler<&'a T> {
    pub fn step_tick(&self) -> Vec<OrderedTick<&'a T>> {
        // Atomic update for the offset
        let current_offset = self.offset.fetch_add(1, Ordering::SeqCst) % MAX_TICK_DELAY;
        let next_offset = (current_offset + 1) % MAX_TICK_DELAY;
        self.offset.store(next_offset, Ordering::SeqCst);

        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_mut() else {
            return Vec::new();
        };

        let res = std::mem::take(&mut inner.tick_queue[current_offset]);

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
        let offset = self.offset.load(Ordering::SeqCst);
        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inner = inner_guard.get_or_insert_with(|| {
            Box::new(ChunkTickSchedulerInner {
                tick_queue: std::array::from_fn(|_| Vec::new()),
                queued_ticks: FxHashSet::default(),
            })
        });

        if inner.queued_ticks.insert((tick.position, tick.value)) {
            let index = (offset + tick.delay as usize) % MAX_TICK_DELAY;

            inner.tick_queue[index].push(OrderedTick {
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

        for queue in &mut inner.tick_queue {
            queue.retain(|tick| !contains(&tick.position));
        }
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
        let offset = self.offset.load(Ordering::SeqCst);
        let inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_ref() else {
            return Vec::new();
        };

        let mut ordered = Vec::with_capacity(inner.queued_ticks.len());
        for i in 0..MAX_TICK_DELAY {
            let index = (offset + i) % MAX_TICK_DELAY;
            ordered.extend(inner.tick_queue[index].iter().map(|tick| (tick, i as u8)));
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
                    tick_queue: std::array::from_fn(|_| Vec::new()),
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
            offset: AtomicUsize::new(0),
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
}
