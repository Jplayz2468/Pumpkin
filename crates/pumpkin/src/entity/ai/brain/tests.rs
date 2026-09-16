//! Tests pinning the parts of the Brain port whose exact semantics are easy to get wrong
//! and whose failure modes are silent: expiry timing, start gating, priority order and
//! sensor scheduling.

use super::behavior::{Behavior, BehaviorContext, BehaviorSlot, BehaviorStatus};
use super::memory::{MemoryMap, MemoryStatus, MemorySlot, MemoryValue};
use super::registry::{Activity, MemoryModuleType, SensorType};
use super::sensor::{Sensor, SensorContext, SensorSlot};
use super::Brain;

/// Deterministic stand-in for `level.getRandom().nextInt(bound)`.
fn zero_rng() -> impl FnMut(i32) -> i32 {
    |_| 0
}

// --- memory ---------------------------------------------------------------------

/// Vanilla `MemorySlot.tick` decrements *after* testing for expiry, so a memory set with
/// ttl `n` survives `n` ticks and is cleared on the next. Getting the order backwards
/// silently shortens every timed memory by one tick.
#[test]
fn memory_with_ttl_survives_exactly_that_many_ticks() {
    let mut slot = MemorySlot::empty();
    slot.set_with_ttl(MemoryValue::Bool(true), 3);

    for tick in 0..3 {
        slot.tick();
        assert!(slot.has_value(), "memory vanished early on tick {tick}");
    }
    slot.tick();
    assert!(!slot.has_value(), "memory outlived its ttl");
}

#[test]
fn memory_without_ttl_never_expires() {
    let mut slot = MemorySlot::empty();
    slot.set(MemoryValue::Unit);
    assert!(!slot.can_expire());
    for _ in 0..1_000 {
        slot.tick();
    }
    assert!(slot.has_value());
}

/// Vanilla distinguishes "this mob has no such memory" from "the memory exists and is
/// empty". An unregistered memory fails *every* status, `ValueAbsent` included.
#[test]
fn unregistered_memory_fails_every_status() {
    let map = MemoryMap::new();
    for status in [
        MemoryStatus::ValuePresent,
        MemoryStatus::ValueAbsent,
        MemoryStatus::Registered,
    ] {
        assert!(
            !map.check(MemoryModuleType::AttackTarget, status),
            "unregistered memory answered {status:?}"
        );
    }
}

#[test]
fn registered_but_empty_memory_is_absent_not_missing() {
    let mut map = MemoryMap::new();
    map.register(MemoryModuleType::AttackTarget);

    assert!(map.check(MemoryModuleType::AttackTarget, MemoryStatus::Registered));
    assert!(map.check(MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent));
    assert!(!map.check(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent));

    map.set(MemoryModuleType::AttackTarget, MemoryValue::EntityId(7));
    assert!(map.check(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent));
    assert!(!map.check(MemoryModuleType::AttackTarget, MemoryStatus::ValueAbsent));
}

// --- behaviours -----------------------------------------------------------------

struct Recorder {
    name: &'static str,
    log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    conditions: Vec<(MemoryModuleType, MemoryStatus)>,
    keep_running: bool,
    extra_start: bool,
}

impl Recorder {
    fn new(name: &'static str, log: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Self {
        Self {
            name,
            log,
            conditions: Vec::new(),
            keep_running: false,
            extra_start: true,
        }
    }
    fn requiring(mut self, c: Vec<(MemoryModuleType, MemoryStatus)>) -> Self {
        self.conditions = c;
        self
    }
    fn keeps_running(mut self) -> Self {
        self.keep_running = true;
        self
    }
    fn blocked(mut self) -> Self {
        self.extra_start = false;
        self
    }
    fn record(&self, what: &str) {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:{what}", self.name));
    }
}

impl Behavior<()> for Recorder {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }
    fn check_extra_start_conditions(&mut self, _ctx: &mut BehaviorContext<'_, ()>) -> bool {
        self.extra_start
    }
    fn start(&mut self, _ctx: &mut BehaviorContext<'_, ()>) {
        self.record("start");
    }
    fn tick(&mut self, _ctx: &mut BehaviorContext<'_, ()>) {
        self.record("tick");
    }
    fn stop(&mut self, _ctx: &mut BehaviorContext<'_, ()>) {
        self.record("stop");
    }
    fn can_still_use(&mut self, _ctx: &mut BehaviorContext<'_, ()>) -> bool {
        self.keep_running
    }
    fn debug_name(&self) -> &'static str {
        self.name
    }
}

fn log() -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
    std::sync::Arc::new(std::sync::Mutex::new(Vec::new()))
}

#[test]
fn behavior_does_not_start_without_its_required_memories() {
    let entries = log();
    let mut slot = BehaviorSlot::new(Box::new(Recorder::new("b", entries.clone()).requiring(
        vec![(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent)],
    )));
    let mut memories = MemoryMap::new();
    memories.register(MemoryModuleType::AttackTarget);

    let mut deferred = Vec::new();

    let mut ctx = BehaviorContext {
        actor: &(),
        memories: &mut memories,
        time: 0,

        deferred: &mut deferred,
    };
    assert!(!slot.try_start(&mut ctx, 10));
    assert_eq!(slot.status(), BehaviorStatus::Stopped);

    ctx.memories
        .set(MemoryModuleType::AttackTarget, MemoryValue::EntityId(1));
    assert!(slot.try_start(&mut ctx, 10));
    assert_eq!(slot.status(), BehaviorStatus::Running);
    assert_eq!(&*entries.lock().unwrap(), &["b:start"]);
}

#[test]
fn extra_start_conditions_can_veto_a_start() {
    let entries = log();
    let mut slot = BehaviorSlot::new(Box::new(Recorder::new("b", entries.clone()).blocked()));
    let mut memories = MemoryMap::new();
    let mut deferred = Vec::new();
    let mut ctx = BehaviorContext {
        actor: &(),
        memories: &mut memories,
        time: 0,
        deferred: &mut deferred,
    };
    assert!(!slot.try_start(&mut ctx, 10));
    assert!(entries.lock().unwrap().is_empty());
}

/// Vanilla's `canStillUse` defaults to false, so a behaviour that does not opt in runs for
/// a single tick and then stops -- it is not the timeout that ends it.
#[test]
fn behavior_stops_after_one_tick_by_default() {
    let entries = log();
    let mut slot = BehaviorSlot::new(Box::new(Recorder::new("b", entries.clone())));
    let mut memories = MemoryMap::new();
    let mut deferred = Vec::new();
    let mut ctx = BehaviorContext {
        actor: &(),
        memories: &mut memories,
        time: 0,
        deferred: &mut deferred,
    };
    slot.try_start(&mut ctx, 100);
    slot.tick_or_stop(&mut ctx);

    assert_eq!(slot.status(), BehaviorStatus::Stopped);
    assert_eq!(&*entries.lock().unwrap(), &["b:start", "b:stop"]);
}

/// A behaviour that keeps returning true still stops once the timestamp passes the
/// duration rolled at start.
#[test]
fn behavior_times_out_even_while_it_wants_to_continue() {
    let entries = log();
    let mut slot =
        BehaviorSlot::new(Box::new(Recorder::new("b", entries.clone()).keeps_running()));
    let mut memories = MemoryMap::new();

    {
        let mut deferred = Vec::new();
        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
            deferred: &mut deferred,
        };
        slot.try_start(&mut ctx, 2);
    }
    for time in [1, 2] {
        let mut deferred = Vec::new();
        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time,
            deferred: &mut deferred,
        };
        slot.tick_or_stop(&mut ctx);
        assert_eq!(slot.status(), BehaviorStatus::Running, "stopped at {time}");
    }
    let mut deferred = Vec::new();
    let mut ctx = BehaviorContext {
        actor: &(),
        memories: &mut memories,
        time: 3,
        deferred: &mut deferred,
    };
    slot.tick_or_stop(&mut ctx);
    assert_eq!(slot.status(), BehaviorStatus::Stopped);
    assert_eq!(entries.lock().unwrap().last().unwrap(), "b:stop");
}

// --- brain ----------------------------------------------------------------------

fn brain_with(entries: &std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Brain<()> {
    let mut brain: Brain<()> = Brain::new(Activity::Idle);
    brain.add_activity(
        Activity::Idle,
        vec![
            (
                10,
                BehaviorSlot::new(Box::new(
                    Recorder::new("low", entries.clone()).keeps_running(),
                )),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(
                    Recorder::new("high", entries.clone()).keeps_running(),
                )),
            ),
        ],
        vec![],
        vec![],
    );
    brain
}

/// Vanilla iterates `availableBehaviorsByPriority` as a `TreeMap`, ascending, so a lower
/// priority number starts first. A hash map here would make this order arbitrary.
#[test]
fn behaviors_start_in_ascending_priority_order() {
    let entries = log();
    let mut brain = brain_with(&entries);
    brain.set_active_activity_if_possible(Activity::Idle);
    brain.tick(&(), 0, &mut zero_rng());

    let starts: Vec<String> = entries
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.ends_with(":start"))
        .cloned()
        .collect();
    assert_eq!(starts, vec!["high:start", "low:start"]);
}

/// An activity that was never added has no requirements registered, and vanilla treats a
/// missing requirement set as "not met" rather than "no conditions, therefore fine".
#[test]
fn unknown_activity_is_never_eligible() {
    let brain: Brain<()> = Brain::new(Activity::Idle);
    assert!(!brain.activity_requirements_are_met(Activity::Fight));
}

#[test]
fn activity_requirements_gate_selection_and_fall_back_to_default() {
    let entries = log();
    let mut brain = brain_with(&entries);
    brain.add_activity(
        Activity::Fight,
        vec![(
            0,
            BehaviorSlot::new(Box::new(Recorder::new("fight", entries.clone()))),
        )],
        vec![(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent)],
        vec![],
    );
    brain.register_memory(MemoryModuleType::AttackTarget);

    // No attack target: Fight is ineligible, so the default activity is used instead.
    brain.set_active_activity_if_possible(Activity::Fight);
    assert!(brain.is_active(Activity::Idle));
    assert!(!brain.is_active(Activity::Fight));

    brain
        .memories_mut()
        .set(MemoryModuleType::AttackTarget, MemoryValue::EntityId(4));
    brain.set_active_activity_if_possible(Activity::Fight);
    assert!(brain.is_active(Activity::Fight));
    assert!(!brain.is_active(Activity::Idle));
}

/// Core activities stay active across an activity switch; that is the whole point of them.
#[test]
fn core_activities_survive_an_activity_switch() {
    let entries = log();
    let mut brain = brain_with(&entries);
    brain.add_activity(Activity::Core, vec![], vec![], vec![]);
    brain.set_core_activities(vec![Activity::Core]);

    brain.set_active_activity_if_possible(Activity::Idle);
    assert!(brain.is_active(Activity::Core));
    assert!(brain.is_active(Activity::Idle));
}

/// Vanilla `eraseMemoriesForOtherActivitesThan`: leaving an activity clears the memories
/// it declared, so stale state cannot leak into the next activity.
#[test]
fn switching_activity_erases_the_old_activitys_memories() {
    let entries = log();
    let mut brain = brain_with(&entries);
    brain.add_activity(
        Activity::Fight,
        vec![],
        vec![],
        vec![MemoryModuleType::AttackTarget],
    );
    brain.register_memory(MemoryModuleType::AttackTarget);
    brain
        .memories_mut()
        .set(MemoryModuleType::AttackTarget, MemoryValue::EntityId(9));

    brain.set_active_activity_if_possible(Activity::Fight);
    assert!(brain.memories().has(MemoryModuleType::AttackTarget));

    brain.set_active_activity_if_possible(Activity::Idle);
    assert!(
        !brain.memories().has(MemoryModuleType::AttackTarget),
        "leaving Fight must erase its declared memories"
    );
}

#[test]
fn first_valid_activity_wins_and_later_ones_are_ignored() {
    let entries = log();
    let mut brain = brain_with(&entries);
    brain.add_activity(Activity::Fight, vec![], vec![], vec![]);
    brain.add_activity(Activity::Panic, vec![], vec![], vec![]);

    brain.set_active_activity_to_first_valid(&[Activity::Fight, Activity::Panic]);
    assert!(brain.is_active(Activity::Fight));
    assert!(!brain.is_active(Activity::Panic));
}

// --- sensors --------------------------------------------------------------------

struct CountingSensor {
    scans: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    rate: i32,
}

impl Sensor<()> for CountingSensor {
    fn do_tick(&mut self, ctx: &mut SensorContext<'_, ()>) {
        self.scans
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ctx.memories
            .set(MemoryModuleType::AttackTarget, MemoryValue::Unit);
    }
    fn scan_rate(&self) -> i32 {
        self.rate
    }
    fn debug_name(&self) -> &'static str {
        "counting"
    }
}

/// Vanilla's `if (--this.timeToTick <= 0L)` decrements before testing, so a fresh sensor
/// scans on its first tick and then once per `scanRate`.
#[test]
fn sensor_scans_immediately_then_on_its_scan_rate() {
    let scans = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut slot = SensorSlot::new(Box::new(CountingSensor {
        scans: scans.clone(),
        rate: 4,
    }));
    let mut memories = MemoryMap::new();

    slot.tick(&mut SensorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
        });
    assert_eq!(scans.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Next three ticks are within the scan interval.
    for _ in 0..3 {
        slot.tick(&mut SensorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
        });
    }
    assert_eq!(scans.load(std::sync::atomic::Ordering::Relaxed), 1);

    slot.tick(&mut SensorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
        });
    assert_eq!(scans.load(std::sync::atomic::Ordering::Relaxed), 2);
}

/// A sensor writing a memory must be able to start a behaviour in the *same* tick --
/// vanilla ticks sensors before starting behaviours, and reversing that would delay every
/// sensor-driven reaction by a tick.
///
/// The behaviour also stops within that same tick: vanilla's running pass follows the
/// start pass, so a just-started behaviour is immediately offered `tickOrStop`, and the
/// default `canStillUse` of false ends it there.
#[test]
fn sensor_output_is_visible_to_behaviors_in_the_same_tick() {
    let entries = log();
    let scans = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut brain: Brain<()> = Brain::new(Activity::Idle);
    brain.register_memory(MemoryModuleType::AttackTarget);
    brain.add_sensor(SensorSlot::new(Box::new(CountingSensor {
        scans,
        rate: 20,
    })));
    brain.add_activity(
        Activity::Idle,
        vec![(
            0,
            BehaviorSlot::new(Box::new(Recorder::new("needs_memory", entries.clone()).requiring(
                vec![(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent)],
            ))),
        )],
        vec![],
        vec![],
    );
    brain.set_active_activity_if_possible(Activity::Idle);

    brain.tick(&(), 0, &mut zero_rng());
    assert_eq!(
        &*entries.lock().unwrap(),
        &["needs_memory:start", "needs_memory:stop"],
        "the sensor's memory must be visible to the start pass in the same tick"
    );
}

// --- registries -----------------------------------------------------------------

/// The generated sets must match the jar's registries; a regeneration that dropped or
/// renamed entries would otherwise go unnoticed until a mob misbehaved.
#[test]
fn generated_registries_match_the_jar_counts() {
    assert_eq!(MemoryModuleType::ALL.len(), 116);
    assert_eq!(SensorType::ALL.len(), 24);
    assert_eq!(Activity::ALL.len(), 26);

    assert_eq!(Activity::Core.name(), "core");
    assert_eq!(MemoryModuleType::AttackTarget.name(), "attack_target");
}

// ── behaviour ports ─────────────────────────────────────────────────────────
//
// Only the actor-agnostic behaviours are covered here. The rest are
// `Behavior<MobActor>` and need a live world to resolve the mob, so they are
// exercised on the server rather than in unit tests.

mod deferred {
    use super::*;
    use crate::entity::ai::brain::behavior::DeferredWrite;
    use crate::entity::ai::brain::memory::MemoryValue;

    /// Queues a write for another mob on start.
    struct Matchmaker {
        partner: i32,
    }

    impl Behavior<()> for Matchmaker {
        fn start(&mut self, ctx: &mut BehaviorContext<'_, ()>) {
            ctx.defer_set(
                self.partner,
                MemoryModuleType::BreedTarget,
                MemoryValue::EntityId(7),
            );
        }
        fn debug_name(&self) -> &'static str {
            "matchmaker"
        }
    }

    /// The point of the whole mechanism: a behaviour cannot touch another brain during
    /// the tick, so the write comes back out of `tick` for the caller to apply once the
    /// lock is released.
    #[test]
    fn a_write_for_another_mob_comes_back_out_of_tick() {
        let mut brain: Brain<()> = Brain::new(Activity::Idle);
        brain.register_memory(MemoryModuleType::BreedTarget);
        brain.add_activity(
            Activity::Idle,
            vec![(0, BehaviorSlot::new(Box::new(Matchmaker { partner: 42 })))],
            vec![],
            vec![],
        );
        brain.set_active_activity_if_possible(Activity::Idle);

        let mut rng = |_: i32| 0;
        let deferred: Vec<DeferredWrite> = brain.tick(&(), 0, &mut rng);

        assert_eq!(deferred.len(), 1, "the queued write must survive the tick");
        assert_eq!(deferred[0].mob_id, 42);
        assert_eq!(deferred[0].memory, MemoryModuleType::BreedTarget);
        assert!(
            deferred[0].value.is_some(),
            "a set carries a value; an erase would not"
        );
        // And it must not have been applied to our own memories.
        assert!(
            !brain.memories_mut().has(MemoryModuleType::BreedTarget),
            "a deferred write is for the other mob, not this one"
        );
    }
}

mod behaviors {
    use super::*;
    use crate::entity::ai::brain::behaviors::count_down_cooldown_ticks::CountDownCooldownTicks;
    use crate::entity::ai::brain::behaviors::gate::{GateBehavior, OrderPolicy, RunningPolicy};
    use crate::entity::ai::brain::memory::MemoryValue;

    fn memories_with(memory: MemoryModuleType, value: MemoryValue) -> MemoryMap {
        let mut memories = MemoryMap::new();
        memories.register(memory);
        memories.set(memory, value);
        memories
    }

    /// `CountDownCooldownTicks` is how every brain cooldown expires: one per tick, then
    /// the memory is erased so the gated behaviour becomes eligible again.
    #[test]
    fn cooldown_counts_down_one_per_tick_then_erases_itself() {
        let mut slot: BehaviorSlot<()> =
            BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                MemoryModuleType::ChargeCooldownTicks,
            )));
        let mut memories = memories_with(
            MemoryModuleType::ChargeCooldownTicks,
            MemoryValue::Int(3),
        );

        let mut deferred = Vec::new();

        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,

            deferred: &mut deferred,
        };
        assert!(slot.try_start(&mut ctx, 100));

        for expected in [2, 1, 0] {
            let mut deferred = Vec::new();
            let mut ctx = BehaviorContext {
                actor: &(),
                memories: &mut memories,
                time: 0,
                deferred: &mut deferred,
            };
            slot.tick_or_stop(&mut ctx);
            assert_eq!(
                memories.get(MemoryModuleType::ChargeCooldownTicks),
                Some(&MemoryValue::Int(expected)),
                "cooldown should tick down one per tick"
            );
        }

        // At zero the behaviour stops, and stopping erases the memory.
        let mut deferred = Vec::new();
        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
            deferred: &mut deferred,
        };
        slot.tick_or_stop(&mut ctx);
        assert_eq!(slot.status(), BehaviorStatus::Stopped);
        assert!(
            !memories.has(MemoryModuleType::ChargeCooldownTicks),
            "an expired cooldown must be erased, not left at zero"
        );
    }

    /// `RunOne` stops at the first child that starts; `TryAll` starts every child that
    /// will. Getting this backwards would make a gate run far more than vanilla does.
    #[test]
    fn gate_running_policy_decides_how_many_children_start() {
        for (policy, expected) in [
            (RunningPolicy::RunOne, vec!["a:start"]),
            (RunningPolicy::TryAll, vec!["a:start", "b:start"]),
        ] {
            let entries = log();
            let mut slot: BehaviorSlot<()> = BehaviorSlot::new(Box::new(GateBehavior::new(
                vec![],
                OrderPolicy::Ordered,
                policy,
                vec![
                    BehaviorSlot::new(Box::new(Recorder::new("a", entries.clone()))),
                    BehaviorSlot::new(Box::new(Recorder::new("b", entries.clone()))),
                ],
            )));
            let mut memories = MemoryMap::new();
            let mut deferred = Vec::new();
            let mut ctx = BehaviorContext {
                actor: &(),
                memories: &mut memories,
                time: 0,
                deferred: &mut deferred,
            };
            assert!(slot.try_start(&mut ctx, 100));
            assert_eq!(*entries.lock().unwrap(), expected);
        }
    }

    /// A gate stops when its last child does, so an idle activity does not sit "running"
    /// with nothing underneath it.
    #[test]
    fn gate_stops_once_no_child_is_still_running() {
        let entries = log();
        let mut slot: BehaviorSlot<()> = BehaviorSlot::new(Box::new(GateBehavior::new(
            vec![],
            OrderPolicy::Ordered,
            RunningPolicy::TryAll,
            vec![BehaviorSlot::new(Box::new(Recorder::new(
                "only",
                entries.clone(),
            )))],
        )));
        let mut memories = MemoryMap::new();
        let mut deferred = Vec::new();
        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
            deferred: &mut deferred,
        };
        assert!(slot.try_start(&mut ctx, 100));

        // The child runs a single tick by default, so the gate has nothing left.
        let mut deferred = Vec::new();
        let mut ctx = BehaviorContext {
            actor: &(),
            memories: &mut memories,
            time: 0,
            deferred: &mut deferred,
        };
        slot.tick_or_stop(&mut ctx);
        assert_eq!(slot.status(), BehaviorStatus::Stopped);
    }
}
