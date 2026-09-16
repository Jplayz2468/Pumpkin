//! The Brain: vanilla's memory-and-activity driven AI, as opposed to the older
//! priority-goal system in `entity::ai::goal`.
//!
//! Port of `net/minecraft/world/entity/ai/Brain.java`. A brain holds:
//!
//! * **memories** -- typed, optionally expiring state ([`memory`])
//! * **sensors** -- periodic scans that refresh memories ([`sensor`])
//! * **behaviours** -- grouped by [`registry::Activity`] and ordered by priority
//!   ([`behavior`])
//!
//! Each tick runs, in vanilla's order (`Brain.tick`): expire memories, tick sensors,
//! start every eligible non-running behaviour, then tick every running behaviour.
//!
//! 22 mobs in 26.2 use a brain, including villagers and piglins. The Warden currently has
//! a hand-rolled equivalent in `entity::mob::warden_brain`; this module is the generic
//! machinery it and the rest should move onto.

pub mod behavior;
pub mod behaviors;
pub mod memory;
pub mod registry;
pub mod sensor;
pub mod sensors;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::world::World;

use behavior::{BehaviorContext, BehaviorSlot, BehaviorStatus, DeferredWrite};
use memory::{MemoryMap, MemoryStatus};
use registry::{Activity, MemoryModuleType};
use sensor::{SensorContext, SensorSlot};

/// Port of vanilla `Brain<E extends LivingEntity>`.
///
/// `A` is the actor behaviours act on. The live server passes [`MobActor`]; scheduling
/// tests pass `()`.
pub struct Brain<A: ?Sized> {
    memories: MemoryMap,
    sensors: Vec<SensorSlot<A>>,

    /// Behaviours keyed by priority, then by the activity that enables them.
    ///
    /// A `BTreeMap` rather than a hash map on purpose: vanilla stores this as a
    /// `TreeMap<Integer, Map<Activity, Set<BehaviorControl>>>` and iterates it in
    /// ascending priority order, so lower-priority behaviours get their chance to start
    /// only after higher-priority ones. Iteration order here is part of the contract.
    behaviors: BTreeMap<i32, BTreeMap<Activity, Vec<BehaviorSlot<A>>>>,

    /// Memory conditions that must hold for an activity to be eligible.
    activity_requirements: BTreeMap<Activity, Vec<(MemoryModuleType, MemoryStatus)>>,
    /// Memories erased when an activity stops being active.
    activity_memories_to_erase_when_stopped: BTreeMap<Activity, Vec<MemoryModuleType>>,

    core_activities: Vec<Activity>,
    active_activities: Vec<Activity>,
    default_activity: Activity,
}

impl<A: ?Sized> Brain<A> {
    #[must_use]
    pub fn new(default_activity: Activity) -> Self {
        Self {
            memories: MemoryMap::new(),
            sensors: Vec::new(),
            behaviors: BTreeMap::new(),
            activity_requirements: BTreeMap::new(),
            activity_memories_to_erase_when_stopped: BTreeMap::new(),
            core_activities: Vec::new(),
            active_activities: Vec::new(),
            default_activity,
        }
    }

    pub const fn memories(&self) -> &MemoryMap {
        &self.memories
    }

    pub const fn memories_mut(&mut self) -> &mut MemoryMap {
        &mut self.memories
    }

    pub fn register_memory(&mut self, memory: MemoryModuleType) {
        self.memories.register(memory);
    }

    pub fn add_sensor(&mut self, sensor: SensorSlot<A>) {
        self.sensors.push(sensor);
    }

    /// Vanilla `Brain.addActivity`.
    pub fn add_activity(
        &mut self,
        activity: Activity,
        behaviors: Vec<(i32, BehaviorSlot<A>)>,
        requirements: Vec<(MemoryModuleType, MemoryStatus)>,
        erase_when_stopped: Vec<MemoryModuleType>,
    ) {
        self.activity_requirements.insert(activity, requirements);
        if !erase_when_stopped.is_empty() {
            self.activity_memories_to_erase_when_stopped
                .insert(activity, erase_when_stopped);
        }
        for (priority, behavior) in behaviors {
            self.behaviors
                .entry(priority)
                .or_default()
                .entry(activity)
                .or_default()
                .push(behavior);
        }
    }

    /// Vanilla `Brain.setCoreActivities`. Core activities are always active and are never
    /// cleared by an activity switch.
    pub fn set_core_activities(&mut self, activities: Vec<Activity>) {
        self.core_activities = activities;
    }

    pub const fn set_default_activity(&mut self, activity: Activity) {
        self.default_activity = activity;
    }

    #[must_use]
    pub fn is_active(&self, activity: Activity) -> bool {
        self.active_activities.contains(&activity)
    }

    /// Vanilla `Brain.activityRequirementsAreMet`.
    ///
    /// An activity with no registered requirements is *not* eligible -- vanilla returns
    /// false when the key is absent, so an activity must be added before it can be
    /// selected.
    #[must_use]
    pub fn activity_requirements_are_met(&self, activity: Activity) -> bool {
        self.activity_requirements
            .get(&activity)
            .is_some_and(|requirements| {
                requirements
                    .iter()
                    .all(|&(memory, status)| self.memories.check(memory, status))
            })
    }

    /// Vanilla `Brain.setActiveActivity`.
    fn set_active_activity(&mut self, activity: Activity) {
        if self.is_active(activity) {
            return;
        }
        self.erase_memories_for_other_activities_than(activity);
        self.active_activities.clear();
        self.active_activities
            .extend(self.core_activities.iter().copied());
        self.active_activities.push(activity);
    }

    /// Vanilla `Brain.eraseMemoriesForOtherActivitesThan`.
    fn erase_memories_for_other_activities_than(&mut self, activity: Activity) {
        let to_erase: Vec<MemoryModuleType> = self
            .active_activities
            .iter()
            .filter(|&&active| active != activity)
            .filter_map(|active| self.activity_memories_to_erase_when_stopped.get(active))
            .flatten()
            .copied()
            .collect();
        for memory in to_erase {
            self.memories.erase(memory);
        }
    }

    /// Vanilla `Brain.setActiveActivityToFirstValid`: the first activity in the list whose
    /// requirements are met wins, and the rest are not considered.
    pub fn set_active_activity_to_first_valid(&mut self, activities: &[Activity]) {
        for &activity in activities {
            if self.activity_requirements_are_met(activity) {
                self.set_active_activity(activity);
                break;
            }
        }
    }

    /// Vanilla `Brain.setActiveActivityIfPossible`, falling back to the default activity.
    pub fn set_active_activity_if_possible(&mut self, activity: Activity) {
        if self.activity_requirements_are_met(activity) {
            self.set_active_activity(activity);
        } else {
            let default = self.default_activity;
            self.set_active_activity(default);
        }
    }

    /// Vanilla `Brain.tick`: expire memories, tick sensors, start eligible behaviours,
    /// then tick running ones.
    ///
    /// The order matters in both directions. A sensor writing a memory can start a
    /// behaviour in the same tick. And because the running pass comes after the start
    /// pass, a behaviour started this tick is *also* ticked this tick -- which is why a
    /// behaviour whose `can_still_use` is the default `false` starts and stops within a
    /// single brain tick rather than lasting until the next one.
    /// Returns any [`DeferredWrite`]s the behaviours queued for *other* mobs. The caller
    /// applies them once this brain's lock is released -- see `DeferredWrite`.
    pub fn tick(
        &mut self,
        actor: &A,
        time: i64,
        rng: &mut impl FnMut(i32) -> i32,
    ) -> Vec<DeferredWrite> {
        let mut deferred = Vec::new();
        self.memories.tick();
        self.tick_sensors(actor, time);
        self.start_each_non_running_behavior(actor, time, rng, &mut deferred);
        self.tick_each_running_behavior(actor, time, &mut deferred);
        deferred
    }

    fn tick_sensors(&mut self, actor: &A, time: i64) {
        for sensor in &mut self.sensors {
            let mut ctx = SensorContext {
                actor,
                memories: &mut self.memories,
                time,
            };
            sensor.tick(&mut ctx);
        }
    }

    /// Vanilla `Brain.startEachNonRunningBehavior`.
    fn start_each_non_running_behavior(
        &mut self,
        actor: &A,
        time: i64,
        rng: &mut impl FnMut(i32) -> i32,
        deferred: &mut Vec<DeferredWrite>,
    ) {
        let active = self.active_activities.clone();
        for by_activity in self.behaviors.values_mut() {
            for (activity, slots) in by_activity.iter_mut() {
                if !active.contains(activity) {
                    continue;
                }
                for slot in slots.iter_mut() {
                    if slot.status() != BehaviorStatus::Stopped {
                        continue;
                    }
                    let (min, max) = slot.duration_range();
                    // Vanilla: minDuration + random.nextInt(maxDuration + 1 - minDuration)
                    let duration = min + rng(max + 1 - min);
                    let mut ctx = BehaviorContext {
                        actor,
                        memories: &mut self.memories,
                        time,
                        deferred,
                    };
                    slot.try_start(&mut ctx, duration);
                }
            }
        }
    }

    /// Vanilla `Brain.tickEachRunningBehavior`.
    fn tick_each_running_behavior(&mut self, actor: &A, time: i64, deferred: &mut Vec<DeferredWrite>) {
        for by_activity in self.behaviors.values_mut() {
            for slots in by_activity.values_mut() {
                for slot in slots.iter_mut() {
                    if slot.status() != BehaviorStatus::Running {
                        continue;
                    }
                    let mut ctx = BehaviorContext {
                        actor,
                        memories: &mut self.memories,
                        time,
                        deferred,
                    };
                    slot.tick_or_stop(&mut ctx);
                }
            }
        }
    }

    /// Vanilla `Brain.stopAll`.
    pub fn stop_all(&mut self, actor: &A, time: i64) {
        // A brain being torn down has no tick to hand deferred writes back to, so any a
        // stopping behaviour queues are dropped.
        let mut deferred = Vec::new();
        let deferred = &mut deferred;
        for by_activity in self.behaviors.values_mut() {
            for slots in by_activity.values_mut() {
                for slot in slots.iter_mut() {
                    if slot.status() != BehaviorStatus::Running {
                        continue;
                    }
                    let mut ctx = BehaviorContext {
                        actor,
                        memories: &mut self.memories,
                        time,
                        deferred,
                    };
                    slot.do_stop(&mut ctx);
                }
            }
        }
    }

    /// Names of the currently running behaviours, for the `/debug` brain dump and tests.
    #[must_use]
    pub fn running_behavior_names(&self) -> Vec<&'static str> {
        self.behaviors
            .values()
            .flat_map(BTreeMap::values)
            .flatten()
            .filter(|slot| slot.status() == BehaviorStatus::Running)
            .map(BehaviorSlot::debug_name)
            .collect()
    }
}

/// What behaviours act on in the live server.
///
/// Owned rather than borrowed on purpose. `Brain<A>` names `A` in its stored fields, so a
/// borrowing actor would force a lifetime through every one of them -- and a bare
/// `dyn Trait` actor defaults to `'static`, which a borrowing host can never satisfy. The
/// mob is carried as an entity id and resolved through the world when a behaviour needs
/// the whole mob, which also means a memory can never keep a removed entity alive.
pub struct MobActor {
    pub world: std::sync::Arc<World>,
    pub mob_id: i32,
    /// The mob's position at the start of this tick, since nearly every sensor wants it.
    pub position: pumpkin_util::math::vector3::Vector3<f64>,
}

impl MobActor {
    /// Resolves the mob this brain belongs to, if it is still in the world.
    #[must_use]
    pub fn entity(&self) -> Option<std::sync::Arc<dyn crate::entity::EntityBase>> {
        self.world.get_entity_by_id(self.mob_id)
    }
}

/// The brain type mobs actually carry.
pub type MobBrain = Brain<MobActor>;
