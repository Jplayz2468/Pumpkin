//! The frog's brain, ported from `FrogAi.java`.
//!
//! Six activities, the most of any species here, selected each tick in vanilla's order:
//! tongue, lay-spawn, long-jump, swim, idle.
//!
//! * **Core** -- panic, look and move sinks, and the cooldown counters.
//! * **Idle** / **Swim** -- the same shape, gated on `is_in_water` being absent or
//!   present. Idle heads for water, Swim heads for land.
//! * **LaySpawn** -- a pregnant frog finds a shoreline and places frogspawn.
//! * **LongJump** -- the ballistic jump, biased toward `frog_prefer_jump_to` blocks.
//! * **Tongue** -- eating slimes and magma cubes.
//!
//! Known gaps: vanilla's `RandomStroll.stroll(1.0, true)` variant and the
//! `triggerIf(Entity::isInWater)` weighting inside the swim gate are simplified here,
//! and the tongue's unreachable-target memory is not maintained. See the individual
//! behaviour files.

use crate::entity::ai::brain::behavior::{BehaviorContext, BehaviorSlot};
use crate::entity::ai::brain::behaviors::animal_panic::AnimalPanic;
use crate::entity::ai::brain::behaviors::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::entity::ai::brain::behaviors::croak::Croak;
use crate::entity::ai::brain::behaviors::follow_temptation::FollowTemptation;
use crate::entity::ai::brain::behaviors::gate::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::entity::ai::brain::behaviors::long_jump::{LongJump, LongJumpMidJump};
use crate::entity::ai::brain::behaviors::look_at_target_sink::LookAtTargetSink;
use crate::entity::ai::brain::behaviors::move_to_target_sink::MoveToTargetSink;
use crate::entity::ai::brain::behaviors::random_stroll::RandomStroll;
use crate::entity::ai::brain::behaviors::set_entity_look_target_sometimes::SetEntityLookTargetSometimes;
use crate::entity::ai::brain::behaviors::set_walk_target_from_look_target::SetWalkTargetFromLookTarget;
use crate::entity::ai::brain::behaviors::shoot_tongue::ShootTongue;
use crate::entity::ai::brain::behaviors::start_attacking::StartAttacking;
use crate::entity::ai::brain::behaviors::try_find_land::TryFindLand;
use crate::entity::ai::brain::behaviors::try_find_land_near_water::TryFindLandNearWater;
use crate::entity::ai::brain::behaviors::try_lay_spawn::TryLaySpawn;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::{Activity, MemoryModuleType};
use crate::entity::ai::brain::sensor::SensorSlot;
use crate::entity::ai::brain::sensors::hurt_by::HurtBySensor;
use crate::entity::ai::brain::sensors::is_in_water::IsInWaterSensor;
use crate::entity::ai::brain::sensors::nearest_players::NearestPlayersSensor;
use crate::entity::ai::brain::sensors::temptations::TemptingSensor;
use crate::entity::ai::brain::{Brain, MobActor, MobBrain};
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::Taggable;

const MEMORIES: &[MemoryModuleType] = &[
    MemoryModuleType::LookTarget,
    MemoryModuleType::WalkTarget,
    MemoryModuleType::NearestVisiblePlayer,
    MemoryModuleType::VisibleMobs,
    MemoryModuleType::TemptationCooldownTicks,
    MemoryModuleType::TemptingPlayer,
    MemoryModuleType::IsTempted,
    MemoryModuleType::BreedTarget,
    MemoryModuleType::IsPanicking,
    MemoryModuleType::HurtBy,
    MemoryModuleType::HurtByEntity,
    MemoryModuleType::AttackTarget,
    MemoryModuleType::AttackCoolingDown,
    MemoryModuleType::CantReachWalkTargetSince,
    MemoryModuleType::IsInWater,
    MemoryModuleType::IsPregnant,
    MemoryModuleType::LongJumpCoolingDown,
    MemoryModuleType::LongJumpMidJump,
    MemoryModuleType::NearestAttackable,
];

/// `FrogAi`'s target finder: whatever the attackable sensor last published.
fn find_attack_target(ctx: &BehaviorContext<'_, MobActor>) -> Option<i32> {
    match ctx.memories.get(MemoryModuleType::NearestAttackable) {
        Some(MemoryValue::EntityId(id)) => Some(*id),
        _ => None,
    }
}

#[must_use]
pub fn build() -> MobBrain {
    let mut brain: MobBrain = Brain::new(Activity::Idle);
    for memory in MEMORIES {
        brain.register_memory(*memory);
    }
    brain.add_sensor(SensorSlot::new(Box::new(NearestPlayersSensor)));
    brain.add_sensor(SensorSlot::new(Box::new(HurtBySensor)));
    brain.add_sensor(SensorSlot::new(Box::new(IsInWaterSensor)));
    brain.add_sensor(SensorSlot::new(Box::new(TemptingSensor::new(
        &pumpkin_data::tag::Item::MINECRAFT_FROG_FOOD,
    ))));

    brain.add_activity(
        Activity::Core,
        vec![
            (0, BehaviorSlot::new(Box::new(AnimalPanic::new(2.0, true)))),
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
                    MemoryModuleType::LongJumpCoolingDown,
                ))),
            ),
        ],
        vec![],
        vec![],
    );

    // Idle: on land. Gated on not being in water and not mid-jump.
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
            (1, BehaviorSlot::new(Box::new(FollowTemptation::new(1.25, 2.5)))),
            (
                2,
                BehaviorSlot::new(Box::new(StartAttacking::new(find_attack_target))),
            ),
            (3, BehaviorSlot::new(Box::new(Croak::default()))),
            (
                5,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
                    OrderPolicy::Ordered,
                    RunningPolicy::TryAll,
                    vec![
                        BehaviorSlot::new(Box::new(RandomStroll::stroll(1.0))),
                        BehaviorSlot::new(Box::new(SetWalkTargetFromLookTarget::new(1.0, 3))),
                    ],
                ))),
            ),
        ],
        vec![
            (MemoryModuleType::LongJumpMidJump, MemoryStatus::ValueAbsent),
            (MemoryModuleType::IsInWater, MemoryStatus::ValueAbsent),
        ],
        vec![],
    );

    // Swim: in water. Heads for land rather than croaking.
    brain.add_activity(
        Activity::Swim,
        vec![
            (
                0,
                BehaviorSlot::new(Box::new(SetEntityLookTargetSometimes::new(
                    Some(&EntityType::PLAYER),
                    6.0,
                    (30, 60),
                ))),
            ),
            (1, BehaviorSlot::new(Box::new(FollowTemptation::new(1.25, 2.5)))),
            (
                2,
                BehaviorSlot::new(Box::new(StartAttacking::new(find_attack_target))),
            ),
            (3, BehaviorSlot::new(Box::new(TryFindLand::new(8, 1.5)))),
            (
                5,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
                    OrderPolicy::Ordered,
                    RunningPolicy::TryAll,
                    vec![
                        BehaviorSlot::new(Box::new(RandomStroll::swim(0.75))),
                        BehaviorSlot::new(Box::new(SetWalkTargetFromLookTarget::new(1.0, 3))),
                    ],
                ))),
            ),
        ],
        vec![
            (MemoryModuleType::LongJumpMidJump, MemoryStatus::ValueAbsent),
            (MemoryModuleType::IsInWater, MemoryStatus::ValuePresent),
        ],
        vec![],
    );

    brain.add_activity(
        Activity::LaySpawn,
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
                BehaviorSlot::new(Box::new(StartAttacking::new(find_attack_target))),
            ),
            (
                2,
                BehaviorSlot::new(Box::new(TryFindLandNearWater::new(8, 1.0))),
            ),
            (
                3,
                BehaviorSlot::new(Box::new(TryLaySpawn::new(&pumpkin_data::Block::FROGSPAWN))),
            ),
        ],
        vec![(MemoryModuleType::IsPregnant, MemoryStatus::ValuePresent)],
        vec![],
    );

    brain.add_activity(
        Activity::LongJump,
        vec![
            (
                0,
                BehaviorSlot::new(Box::new(LongJumpMidJump::new(
                    (10, 20),
                    Sound::EntityFrogStep,
                ))),
            ),
            (
                1,
                BehaviorSlot::new(Box::new(LongJump::new(
                    (10, 20),
                    4,
                    4,
                    1.5,
                    Sound::EntityFrogLongJump,
                    Some(&pumpkin_data::tag::Block::MINECRAFT_FROG_PREFER_JUMP_TO),
                    0.5,
                ))),
            ),
        ],
        vec![(
            MemoryModuleType::LongJumpCoolingDown,
            MemoryStatus::ValueAbsent,
        )],
        vec![],
    );

    brain.add_activity(
        Activity::Tongue,
        vec![
            (
                0,
                BehaviorSlot::new(Box::new(ShootTongue::new(
                    Sound::EntityFrogTongue,
                    Sound::EntityFrogEat,
                ))),
            ),
        ],
        vec![(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent)],
        vec![MemoryModuleType::AttackTarget],
    );

    brain.set_core_activities(vec![Activity::Core]);
    // FrogAi.updateActivity.
    brain.set_activity_priority(vec![
        Activity::Tongue,
        Activity::LaySpawn,
        Activity::LongJump,
        Activity::Swim,
        Activity::Idle,
    ]);
    brain.set_active_activity_if_possible(Activity::Idle);
    brain
}

/// Keeps the tag import meaningful for the preferred-jump gate above.
const _: fn(&'static pumpkin_data::Block) -> bool =
    |block| block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FROG_PREFER_JUMP_TO);
