//! Brain memory storage.
//!
//! Port of vanilla `MemorySlot` and `MemoryMap`
//! (`net/minecraft/world/entity/ai/memory/`). A memory is a typed slot that is either
//! empty or holds a value, optionally with a time-to-live measured in ticks.

use std::collections::HashMap;

use super::registry::MemoryModuleType;

/// A value a mob can remember. Vanilla stores these as `Object` behind a typed
/// `MemoryModuleType`; Rust needs the set spelled out, so new memory shapes are added
/// here as mobs come to need them.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryValue {
    /// Memories that carry no payload and matter only by being present, e.g.
    /// `has_hunting_cooldown`.
    Unit,
    Bool(bool),
    Long(i64),
    Int(i32),
    Float(f32),
    Position(pumpkin_util::math::position::BlockPos),
    Vec3(pumpkin_util::math::vector3::Vector3<f64>),
    /// Vanilla's `WalkTarget`: where to go, how fast, and how close counts as arrived.
    /// Kept as one value because `MoveToTargetSink` needs all three together.
    WalkTarget {
        destination: pumpkin_util::math::vector3::Vector3<f64>,
        speed: f32,
        close_enough: i32,
    },
    /// The damage type of a `hurt_by` memory. Vanilla stores the whole `DamageSource`;
    /// the type is the part behaviours actually test, through tags like `panic_causes`.
    DamageType(pumpkin_data::damage::DamageType),
    /// An entity, referenced by id so a memory never keeps a removed entity alive.
    EntityId(i32),
    EntityIds(Vec<i32>),
    Uuid(uuid::Uuid),
}

/// Vanilla `MemorySlot.NEVER_EXPIRE` is `Long.MAX_VALUE`; the same sentinel is used here
/// so the "can expire" test is an equality check rather than an `Option` branch, matching
/// the reference exactly.
const NEVER_EXPIRE: i64 = i64::MAX;

/// Port of vanilla `MemorySlot`.
#[derive(Debug, Clone)]
pub struct MemorySlot {
    value: Option<MemoryValue>,
    time_to_live: i64,
}

impl Default for MemorySlot {
    fn default() -> Self {
        Self::empty()
    }
}

impl MemorySlot {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            value: None,
            time_to_live: NEVER_EXPIRE,
        }
    }

    /// Vanilla `MemorySlot.tick`.
    ///
    /// Note the ordering: a slot expires only once its ttl has already reached zero, so a
    /// memory set with ttl `n` is readable on `n` subsequent ticks and cleared on the
    /// one after. Decrementing before the check would cut every memory a tick short.
    pub fn tick(&mut self) {
        if self.has_value() && self.can_expire() {
            if self.has_expired() {
                self.clear();
            } else {
                self.time_to_live -= 1;
            }
        }
    }

    pub fn set(&mut self, value: MemoryValue) {
        self.set_with_ttl(value, NEVER_EXPIRE);
    }

    pub fn set_with_ttl(&mut self, value: MemoryValue, time_to_live: i64) {
        self.value = Some(value);
        self.time_to_live = time_to_live;
    }

    pub fn clear(&mut self) {
        self.value = None;
        self.time_to_live = NEVER_EXPIRE;
    }

    #[must_use]
    pub const fn has_value(&self) -> bool {
        self.value.is_some()
    }

    #[must_use]
    pub const fn value(&self) -> Option<&MemoryValue> {
        self.value.as_ref()
    }

    #[must_use]
    pub const fn can_expire(&self) -> bool {
        self.time_to_live != NEVER_EXPIRE
    }

    #[must_use]
    pub const fn has_expired(&self) -> bool {
        self.time_to_live <= 0
    }

    #[must_use]
    pub const fn time_to_live(&self) -> i64 {
        self.time_to_live
    }
}

/// Whether a behaviour requires a memory to be present, absent, or merely registered.
/// Port of vanilla `MemoryStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryStatus {
    ValuePresent,
    ValueAbsent,
    Registered,
}

/// The set of memories a brain owns. Port of vanilla `MemoryMap`.
///
/// A memory must be *registered* before it can be used: vanilla distinguishes "this mob
/// has no such memory" from "this mob has the memory and it is currently empty", and
/// `MemoryStatus::Registered` tests exactly that difference.
#[derive(Debug, Default, Clone)]
pub struct MemoryMap {
    slots: HashMap<MemoryModuleType, MemorySlot>,
}

impl MemoryMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, memory: MemoryModuleType) {
        self.slots.entry(memory).or_insert_with(MemorySlot::empty);
    }

    #[must_use]
    pub fn is_registered(&self, memory: MemoryModuleType) -> bool {
        self.slots.contains_key(&memory)
    }

    /// Vanilla `Brain.forgetOutdatedMemories`, which ticks every slot.
    pub fn tick(&mut self) {
        for slot in self.slots.values_mut() {
            slot.tick();
        }
    }

    /// Setting a memory on an unregistered type registers it, matching vanilla's
    /// `checkMemory` contract where a set implies the memory exists on this mob.
    pub fn set(&mut self, memory: MemoryModuleType, value: MemoryValue) {
        self.slots.entry(memory).or_default().set(value);
    }

    pub fn set_with_ttl(&mut self, memory: MemoryModuleType, value: MemoryValue, ttl: i64) {
        self.slots
            .entry(memory)
            .or_default()
            .set_with_ttl(value, ttl);
    }

    pub fn erase(&mut self, memory: MemoryModuleType) {
        if let Some(slot) = self.slots.get_mut(&memory) {
            slot.clear();
        }
    }

    #[must_use]
    pub fn get(&self, memory: MemoryModuleType) -> Option<&MemoryValue> {
        self.slots.get(&memory).and_then(MemorySlot::value)
    }

    #[must_use]
    pub fn has(&self, memory: MemoryModuleType) -> bool {
        self.slots.get(&memory).is_some_and(MemorySlot::has_value)
    }

    /// Vanilla `Brain.checkMemory`.
    ///
    /// An unregistered memory fails every status, including `ValueAbsent` -- vanilla
    /// requires the slot to exist before it will answer questions about it.
    #[must_use]
    pub fn check(&self, memory: MemoryModuleType, status: MemoryStatus) -> bool {
        let Some(slot) = self.slots.get(&memory) else {
            return false;
        };
        match status {
            MemoryStatus::Registered => true,
            MemoryStatus::ValuePresent => slot.has_value(),
            MemoryStatus::ValueAbsent => !slot.has_value(),
        }
    }
}
