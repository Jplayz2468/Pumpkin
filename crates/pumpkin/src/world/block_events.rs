use crate::block::BlockEvent;
use rustc_hash::FxHashSet;
use std::collections::VecDeque;

/// Java's linked set: insertion order, deduplication while pending, and removal
/// before dispatch so a handler may enqueue the same event again.
#[derive(Default)]
pub(super) struct BlockEventQueue {
    pending: VecDeque<BlockEvent>,
    unique: FxHashSet<BlockEvent>,
}

impl BlockEventQueue {
    pub fn push_back(&mut self, event: BlockEvent) {
        if self.unique.insert(event) {
            self.pending.push_back(event);
        }
    }

    pub fn pop_front(&mut self) -> Option<BlockEvent> {
        let event = self.pending.pop_front()?;
        self.unique.remove(&event);
        Some(event)
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.unique.clear();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&BlockEvent) -> bool) {
        self.pending.retain(|event| {
            if keep(event) {
                true
            } else {
                self.unique.remove(event);
                false
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::math::position::BlockPos;

    #[test]
    fn ordered_pending_duplicates_and_reentrant_events() {
        let mut queue = BlockEventQueue::default();
        let a = BlockEvent {
            block_id: 1,
            pos: BlockPos::new(0, 64, 0),
            r#type: 0,
            data: 0,
        };
        let b = BlockEvent { block_id: 2, ..a };
        queue.push_back(a);
        queue.push_back(b);
        queue.push_back(a);
        assert_eq!(queue.pop_front(), Some(a));
        queue.push_back(a);
        assert_eq!(queue.pop_front(), Some(b));
        assert_eq!(queue.pop_front(), Some(a));
        assert_eq!(queue.pop_front(), None);
        queue.push_back(a);
        queue.retain(|_| false);
        queue.push_back(a);
        assert_eq!(queue.pop_front(), Some(a));
        queue.push_back(b);
        queue.clear();
        queue.push_back(b);
        assert_eq!(queue.pop_front(), Some(b));
    }
}
