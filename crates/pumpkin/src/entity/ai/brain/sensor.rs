//! Sensors: the periodic scans that refresh a brain's memories.
//!
//! Port of vanilla `Sensor` (`net/minecraft/world/entity/ai/sensing/Sensor.java`).

use super::memory::MemoryMap;

/// Vanilla's no-argument `Sensor()` constructor uses a 20-tick scan rate.
pub const DEFAULT_SCAN_RATE: i32 = 20;

/// A scan that writes memories. Implementations do the actual looking; the scheduling is
/// handled by [`SensorSlot`] so it cannot drift from the reference.
pub trait Sensor: Send + Sync {
    /// Vanilla `Sensor.doTick`.
    fn do_tick(&mut self, memories: &mut MemoryMap, time: i64);

    /// Memories this sensor is responsible for. Vanilla `Sensor.requires`; a brain
    /// registers these so they exist before anything reads them.
    fn requires(&self) -> &[super::registry::MemoryModuleType] {
        &[]
    }

    fn scan_rate(&self) -> i32 {
        DEFAULT_SCAN_RATE
    }

    fn debug_name(&self) -> &'static str;
}

/// Wraps a [`Sensor`] with vanilla's countdown scheduling.
pub struct SensorSlot {
    sensor: Box<dyn Sensor>,
    time_to_tick: i64,
}

impl SensorSlot {
    #[must_use]
    pub fn new(sensor: Box<dyn Sensor>) -> Self {
        Self {
            sensor,
            time_to_tick: 0,
        }
    }

    /// Vanilla `Sensor.randomlyDelayStart`, which staggers sensors across mobs so they do
    /// not all scan on the same tick.
    pub fn randomly_delay_start(&mut self, rng: &mut impl FnMut(i32) -> i32) {
        self.time_to_tick = i64::from(rng(self.sensor.scan_rate()));
    }

    /// Vanilla `Sensor.tick`:
    ///
    /// ```java
    /// if (--this.timeToTick <= 0L) { this.timeToTick = this.scanRate; ... }
    /// ```
    ///
    /// The decrement happens *before* the test, so a sensor constructed with the counter
    /// at zero scans on its very first tick and every `scanRate` ticks after.
    pub fn tick(&mut self, memories: &mut MemoryMap, time: i64) {
        self.time_to_tick -= 1;
        if self.time_to_tick <= 0 {
            self.time_to_tick = i64::from(self.sensor.scan_rate());
            self.sensor.do_tick(memories, time);
        }
    }

    #[must_use]
    pub fn debug_name(&self) -> &'static str {
        self.sensor.debug_name()
    }

    #[must_use]
    pub fn requires(&self) -> &[super::registry::MemoryModuleType] {
        self.sensor.requires()
    }
}
