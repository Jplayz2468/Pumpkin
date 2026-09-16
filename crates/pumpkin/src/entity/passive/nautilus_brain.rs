//! The nautilus's brain, ported from `NautilusAi.java`.
//!
//! The first real brain mob in the server. Vanilla's nautilus has no goals at all: a
//! `Brain` with a core activity that always runs and an idle activity that wanders.
//!
//! Implemented from `NautilusAi.getActivities`:
//!
//! * **Core** -- `LookAtTargetSink(45, 90)`, `MoveToTargetSink`, and a
//!   `CountDownCooldownTicks` per cooldown memory.
//! * **Idle** -- a `GateBehavior` gated on having no walk target, trying
//!   `RandomStroll.swim(1.0)` then `SetWalkTargetFromLookTarget(1.0, 3)`.
//!
//! Not yet ported, and so not yet in this brain: `AnimalPanic`, `AnimalMakeLove`,
//! `FollowTemptation`, `StartAttacking` and the whole Fight activity with `ChargeAttack`.
//! A nautilus therefore swims and looks around but does not breed, follow a tempting
//! player or charge a target. Recorded in SURVIVAL_PARITY_BACKLOG.md.

use crate::entity::ai::brain::behavior::BehaviorSlot;
use crate::entity::ai::brain::behaviors::animal_panic::AnimalPanic;
use crate::entity::ai::brain::behaviors::charge_attack::ChargeAttack;
use crate::entity::ai::brain::behaviors::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::entity::ai::brain::behaviors::follow_temptation::FollowTemptation;
use crate::entity::ai::brain::behaviors::gate::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::entity::ai::brain::behaviors::look_at_target_sink::LookAtTargetSink;
use crate::entity::ai::brain::behaviors::move_to_target_sink::MoveToTargetSink;
use crate::entity::ai::brain::behaviors::random_stroll::RandomStroll;
use crate::entity::ai::brain::behaviors::start_attacking::StartAttacking;
use crate::entity::ai::brain::behaviors::set_walk_target_from_look_target::SetWalkTargetFromLookTarget;
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::{Activity, MemoryModuleType};
use crate::entity::ai::brain::sensor::SensorSlot;
use crate::entity::ai::brain::sensors::hurt_by::HurtBySensor;
use crate::entity::ai::brain::sensors::nearest_players::NearestPlayersSensor;
use crate::entity::ai::brain::sensors::temptations::TemptingSensor;
use crate::entity::ai::brain::{Brain, MobBrain};

/// Memories the nautilus brain reads or writes.
const MEMORIES: &[MemoryModuleType] = &[
    MemoryModuleType::LookTarget,
    MemoryModuleType::WalkTarget,
    MemoryModuleType::NearestVisiblePlayer,
    MemoryModuleType::TemptationCooldownTicks,
    MemoryModuleType::AttackTargetCooldown,
    MemoryModuleType::HurtBy,
    MemoryModuleType::HurtByEntity,
    MemoryModuleType::IsPanicking,
    MemoryModuleType::IsTempted,
    MemoryModuleType::TemptingPlayer,
    MemoryModuleType::BreedTarget,
    MemoryModuleType::AttackTarget,
    MemoryModuleType::ChargeCooldownTicks,
    MemoryModuleType::CantReachWalkTargetSince,
];

#[must_use]
pub fn build() -> MobBrain {
    let mut brain: MobBrain = Brain::new(Activity::Idle);
    for memory in MEMORIES {
        brain.register_memory(*memory);
    }
    brain.add_sensor(SensorSlot::new(Box::new(NearestPlayersSensor)));
    brain.add_sensor(SensorSlot::new(Box::new(HurtBySensor)));
    brain.add_sensor(SensorSlot::new(Box::new(TemptingSensor::new(
        &pumpkin_data::tag::Item::MINECRAFT_NAUTILUS_FOOD,
    ))));

    // NautilusAi.initCoreActivity, priority 0, always running.
    brain.add_activity(
        Activity::Core,
        vec![
            // `AnimalPanic(1.6)`; `swim` because a nautilus flees through water.
            (0, BehaviorSlot::new(Box::new(AnimalPanic::new(1.6, true)))),
            (0, BehaviorSlot::new(Box::new(LookAtTargetSink::new(45, 90)))),
            (
                0,
                BehaviorSlot::new(Box::new(MoveToTargetSink::default())),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::TemptationCooldownTicks,
                ))),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::AttackTargetCooldown,
                ))),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::ChargeCooldownTicks,
                ))),
            ),
        ],
        vec![],
        vec![],
    );

    // NautilusAi.initIdleActivity: the wandering gate, at vanilla's priority 4.
    brain.add_activity(
        Activity::Idle,
        vec![(
            // `FollowTemptation(1.3, baby ? 2.5 : 3.5)`; the adult distance is used, since
            // the brain has no handle on the mob's age here.
            2,
            BehaviorSlot::new(Box::new(FollowTemptation::new(1.3, 3.5))),
        ), (
            4,
            BehaviorSlot::new(Box::new(GateBehavior::new(
                vec![(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
                OrderPolicy::Ordered,
                RunningPolicy::TryAll,
                vec![
                    BehaviorSlot::new(Box::new(RandomStroll::swim(1.0))),
                    BehaviorSlot::new(Box::new(SetWalkTargetFromLookTarget::new(1.0, 3))),
                ],
            ))),
        )],
        vec![],
        vec![],
    );

    // NautilusAi.initFightActivity: `ChargeAttack(80, .., 0.6, 2.0, 12.0, 11.0,
    // NAUTILUS_DASH)`, gated on having a target and nothing more interesting to do.
    brain.add_activity(
        Activity::Fight,
        vec![(
            0,
            BehaviorSlot::new(Box::new(ChargeAttack::new(
                80,
                0.6,
                12.0,
                11.0,
                pumpkin_data::sound::Sound::EntityNautilusDash,
            ))),
        )],
        vec![
            (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
            (MemoryModuleType::TemptingPlayer, MemoryStatus::ValueAbsent),
            (MemoryModuleType::BreedTarget, MemoryStatus::ValueAbsent),
            (
                MemoryModuleType::ChargeCooldownTicks,
                MemoryStatus::ValueAbsent,
            ),
        ],
        vec![MemoryModuleType::AttackTarget],
    );

    brain.set_core_activities(vec![Activity::Core]);
    brain.set_active_activity_if_possible(Activity::Idle);
    brain
}
