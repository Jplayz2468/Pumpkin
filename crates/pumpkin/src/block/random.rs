use pumpkin_util::random::{RandomDeriver, RandomImpl, legacy_rand::LegacyRand};
use std::sync::Mutex;

/// Java's level random source can be used again by a nested block update.
/// Keep each draw atomic without holding the mutex across the callback itself.
pub enum BlockRandom<'a> {
    Shared(&'a Mutex<LegacyRand>),
    Owned(LegacyRand),
}

impl BlockRandom<'_> {
    fn with_random<T>(&mut self, draw: impl FnOnce(&mut LegacyRand) -> T) -> T {
        match self {
            Self::Shared(source) => draw(
                &mut source
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            ),
            Self::Owned(source) => draw(source),
        }
    }
}

impl RandomImpl for BlockRandom<'_> {
    fn split(&mut self) -> Self {
        Self::Owned(self.with_random(RandomImpl::split))
    }
    fn next_splitter(&mut self) -> RandomDeriver {
        self.with_random(RandomImpl::next_splitter)
    }
    fn next_i32(&mut self) -> i32 {
        self.with_random(RandomImpl::next_i32)
    }
    fn next_bounded_i32(&mut self, bound: i32) -> i32 {
        self.with_random(|random| random.next_bounded_i32(bound))
    }
    fn next_i64(&mut self) -> i64 {
        self.with_random(RandomImpl::next_i64)
    }
    fn next_bool(&mut self) -> bool {
        self.with_random(RandomImpl::next_bool)
    }
    fn next_f32(&mut self) -> f32 {
        self.with_random(RandomImpl::next_f32)
    }
    fn next_f64(&mut self) -> f64 {
        self.with_random(RandomImpl::next_f64)
    }
    fn next_gaussian(&mut self) -> f64 {
        self.with_random(RandomImpl::next_gaussian)
    }
}
