//! The camel's brain, ported from `CamelAi.java`.
//!
//! Implemented from `CamelAi.getActivities`:
//!
//! * **Core** -- `CamelPanic(2.0)`, `Swim(0.8)`, `LookAtTargetSink(45, 90)`,
//!   `MoveToTargetSink`, and a `CountDownCooldownTicks` for the temptation and gaze
//!   cooldowns.
//! * **Idle** -- glance at players, make love, a follow gate of temptation and
//!   baby-follows-adult, random looking, and a wander gate.
//!
//! Known gap: the camel has no sitting state in this server -- only saddled and dashing
//! flags -- so `CamelAi.RandomSitting` is absent, and the `triggerIf(not(refuseToMove))`
//! gates vanilla wraps around three idle behaviours are omitted rather than faked. With
//! no sitting, `refuseToMove` is always false and those gates would be vacuous anyway.
//! `CamelPanic`'s extra conditions -- refusing while mob-controlled, and standing up
//! before fleeing -- depend on the same missing state. Recorded in
//! SURVIVAL_PARITY_BACKLOG.md.

use crate::entity::ai::brain::behavior::BehaviorSlot;
use crate::entity::ai::brain::behaviors::animal_make_love::AnimalMakeLove;
use crate::entity::ai::brain::behaviors::animal_panic::AnimalPanic;
use crate::entity::ai::brain::behaviors::baby_follow_adult::BabyFollowAdult;
use crate::entity::ai::brain::behaviors::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::entity::ai::brain::behaviors::do_nothing::DoNothing;
use crate::entity::ai::brain::behaviors::follow_temptation::FollowTemptation;
use crate::entity::ai::brain::behaviors::gate::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::entity::ai::brain::behaviors::look_at_target_sink::LookAtTargetSink;
use crate::entity::ai::brain::behaviors::move_to_target_sink::MoveToTargetSink;
use crate::entity::ai::brain::behaviors::random_look_around::RandomLookAround;
use crate::entity::ai::brain::behaviors::random_stroll::RandomStroll;
use crate::entity::ai::brain::behaviors::set_entity_look_target_sometimes::SetEntityLookTargetSometimes;
use crate::entity::ai::brain::behaviors::set_walk_target_from_look_target::SetWalkTargetFromLookTarget;
use crate::entity::ai::brain::behaviors::swim::Swim;
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::{Activity, MemoryModuleType};
use crate::entity::ai::brain::sensor::SensorSlot;
use crate::entity::ai::brain::sensors::hurt_by::HurtBySensor;
use crate::entity::ai::brain::sensors::nearest_players::NearestPlayersSensor;
use crate::entity::ai::brain::sensors::temptations::TemptingSensor;
use crate::entity::ai::brain::{Brain, MobBrain};
use pumpkin_data::entity::EntityType;

/// `CamelAi.ADULT_FOLLOW_RANGE`.
const ADULT_FOLLOW_RANGE: (i32, i32) = (5, 16);

const MEMORIES: &[MemoryModuleType] = &[
    MemoryModuleType::LookTarget,
    MemoryModuleType::WalkTarget,
    MemoryModuleType::NearestVisiblePlayer,
    MemoryModuleType::VisibleMobs,
    MemoryModuleType::NearestVisibleAdult,
    MemoryModuleType::TemptationCooldownTicks,
    MemoryModuleType::GazeCooldownTicks,
    MemoryModuleType::TemptingPlayer,
    MemoryModuleType::IsTempted,
    MemoryModuleType::BreedTarget,
    MemoryModuleType::IsPanicking,
    MemoryModuleType::HurtBy,
    MemoryModuleType::HurtByEntity,
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
        &pumpkin_data::tag::Item::MINECRAFT_CAMEL_FOOD,
    ))));

    brain.add_activity(
        Activity::Core,
        vec![
            (0, BehaviorSlot::new(Box::new(AnimalPanic::new(2.0, false)))),
            (0, BehaviorSlot::new(Box::new(Swim::new(0.8)))),
            (0, BehaviorSlot::new(Box::new(LookAtTargetSink::new(45, 90)))),
            (0, BehaviorSlot::new(Box::new(MoveToTargetSink::default()))),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::TemptationCooldownTicks,
                ))),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::GazeCooldownTicks,
                ))),
            ),
        ],
        vec![],
        vec![],
    );

    brain.add_activity(
        Activity::Idle,
        vec![
            (
                0,
                BehaviorSlot::new(Box::new(SetEntityLookTargetSometimes::new(
                    Some(&EntityType::PLAYER),
                    6.0,
                    (30, 60),
                ))),
            ),
            (
                1,
                BehaviorSlot::new(Box::new(AnimalMakeLove::new(&EntityType::CAMEL, 1.0, 2))),
            ),
            (
                2,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![],
                    OrderPolicy::Ordered,
                    RunningPolicy::RunOne,
                    vec![
                        BehaviorSlot::new(Box::new(FollowTemptation::new(2.5, 3.5))),
                        BehaviorSlot::new(Box::new(BabyFollowAdult::new(
                            ADULT_FOLLOW_RANGE,
                            2.5,
                        ))),
                    ],
                ))),
            ),
            (
                3,
                BehaviorSlot::new(Box::new(RandomLookAround::new((150, 250), 30.0, 0.0, 0.0))),
            ),
            (
                4,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
                    OrderPolicy::Ordered,
                    RunningPolicy::RunOne,
                    vec![
                        BehaviorSlot::new(Box::new(RandomStroll::stroll(2.0))),
                        BehaviorSlot::new(Box::new(SetWalkTargetFromLookTarget::new(2.0, 3))),
                        BehaviorSlot::new(Box::new(DoNothing::new(30, 60))),
                    ],
                ))),
            ),
        ],
        vec![],
        vec![],
    );

    brain.set_core_activities(vec![Activity::Core]);
    // CamelAi.updateActivity passes only IDLE.
    brain.set_activity_priority(vec![Activity::Idle]);
    brain.set_active_activity_if_possible(Activity::Idle);
    brain
}
