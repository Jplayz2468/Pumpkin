use std::sync::atomic::{AtomicI64, AtomicU16, AtomicU64, Ordering};

#[derive(Debug)]
pub struct ViewerCountTracker {
    pub position: Option<pumpkin_util::math::position::BlockPos>,
    pub old: AtomicU16,
    pub current: AtomicU16,
    pub next_recheck: AtomicI64,
    pub max_interaction_range: AtomicU64,
}

impl Default for ViewerCountTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewerCountTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            position: None,
            old: AtomicU16::new(0),
            current: AtomicU16::new(0),
            next_recheck: AtomicI64::new(-1),
            max_interaction_range: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub const fn at(position: pumpkin_util::math::position::BlockPos) -> Self {
        Self {
            position: Some(position),
            old: AtomicU16::new(0),
            current: AtomicU16::new(0),
            next_recheck: AtomicI64::new(-1),
            max_interaction_range: AtomicU64::new(0),
        }
    }

    pub fn open_container(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    pub fn close_container(&self) {
        // A scheduled recheck can already have removed a stale viewer.
        let _ = self
            .current
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_sub(1))
            });
    }

    /// Returns the current number of players viewing this container
    pub fn get_viewer_count(&self) -> u16 {
        self.current.load(Ordering::Relaxed)
    }
}
