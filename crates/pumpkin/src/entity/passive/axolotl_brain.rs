//! The axolotl's brain, ported from `AxolotlAi.java`.
//!
//! Four activities:
//!
//! * **Core** -- look and move sinks, `ValidatePlayDead`, and the temptation cooldown.
//! * **Idle** -- glance at players, make love, a follow gate of temptation and
//!   baby-follows-adult, target selection, `TryFindWater`, and a wander gate.
//! * **Fight** -- stop attacking when the target is invalid, close the distance, hit it,
//!   and drop the target if the axolotl starts breeding.
//! * **PlayDead** -- gated on the `play_dead_ticks` memory, which the hurt path sets.
//!
//! Known gap: vanilla's `Axolotl.onStopAttacking` also remembers a hunted target so the
//! axolotl is rewarded by a player who helped; that callback is not ported.

use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext, BehaviorSlot};
use crate::entity::ai::brain::behaviors::animal_make_love::AnimalMakeLove;
use crate::entity::ai::brain::behaviors::baby_follow_adult::BabyFollowAdult;
use crate::entity::ai::brain::behaviors::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::entity::ai::brain::behaviors::erase_memory_if::EraseMemoryIf;
use crate::entity::ai::brain::behaviors::follow_temptation::FollowTemptation;
use crate::entity::ai::brain::behaviors::gate::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::entity::ai::brain::behaviors::look_at_target_sink::LookAtTargetSink;
use crate::entity::ai::brain::behaviors::melee_attack::MeleeAttack;
use crate::entity::ai::brain::behaviors::move_to_target_sink::MoveToTargetSink;
use crate::entity::ai::brain::behaviors::play_dead::{PlayDead, ValidatePlayDead};
use crate::entity::ai::brain::behaviors::random_stroll::RandomStroll;
use crate::entity::ai::brain::behaviors::set_entity_look_target_sometimes::SetEntityLookTargetSometimes;
use crate::entity::ai::brain::behaviors::set_walk_target_from_attack_target::SetWalkTargetFromAttackTarget;
use crate::entity::ai::brain::behaviors::set_walk_target_from_look_target::SetWalkTargetFromLookTarget;
use crate::entity::ai::brain::behaviors::start_attacking::StartAttacking;
use crate::entity::ai::brain::behaviors::stop_attacking_if_target_invalid::StopAttackingIfTargetInvalid;
use crate::entity::ai::brain::behaviors::try_find_water::TryFindWater;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::{Activity, MemoryModuleType};
use crate::entity::ai::brain::sensor::SensorSlot;
use crate::entity::ai::brain::sensors::hurt_by::HurtBySensor;
use crate::entity::ai::brain::sensors::nearest_players::NearestPlayersSensor;
use crate::entity::ai::brain::sensors::temptations::TemptingSensor;
use crate::entity::ai::brain::{Brain, MobActor, MobBrain};
use pumpkin_data::entity::EntityType;
use pumpkin_data::tag::Taggable;

/// `AxolotlAi.ADULT_FOLLOW_RANGE`.
const ADULT_FOLLOW_RANGE: (i32, i32) = (5, 16);

const MEMORIES: &[MemoryModuleType] = &[
    MemoryModuleType::LookTarget,
    MemoryModuleType::WalkTarget,
    MemoryModuleType::NearestVisiblePlayer,
    MemoryModuleType::VisibleMobs,
    MemoryModuleType::NearestVisibleAdult,
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
    MemoryModuleType::PlayDeadTicks,
];

/// `AxolotlAi.findNearestValidAttackTarget`: the nearest visible mob in the axolotl's
/// hunt-target or always-hostile tags.
fn find_attack_target(ctx: &BehaviorContext<'_, MobActor>) -> Option<i32> {
    let candidates = match ctx.memories.get(MemoryModuleType::VisibleMobs) {
        Some(MemoryValue::EntityIds(ids)) => ids.clone(),
        _ => return None,
    };
    candidates.into_iter().find(|id| {
        ctx.actor.world.get_entity_by_id(*id).is_some_and(|entity| {
            let entity_type = entity.get_entity().entity_type;
            entity_type.has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_AXOLOTL_HUNT_TARGETS)
                || entity_type
                    .has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_AXOLOTL_ALWAYS_HOSTILES)
        })
    })
}

/// `BehaviorUtils.isBreeding`: used to drop both the attack target and the play-dead
/// timer once the axolotl starts courting.
fn is_breeding(ctx: &BehaviorContext<'_, MobActor>) -> bool {
    ctx.memories.has(MemoryModuleType::BreedTarget)
}

#[must_use]
pub fn build() -> MobBrain {
    let mut brain: MobBrain = Brain::new(Activity::Idle);
    for memory in MEMORIES {
        brain.register_memory(*memory);
    }
    brain.add_sensor(SensorSlot::new(Box::new(NearestPlayersSensor)));
    brain.add_sensor(SensorSlot::new(Box::new(HurtBySensor)));
    brain.add_sensor(SensorSlot::new(Box::new(TemptingSensor::new(
        &pumpkin_data::tag::Item::MINECRAFT_AXOLOTL_FOOD,
    ))));

    brain.add_activity(
        Activity::Core,
        vec![
            (0, BehaviorSlot::new(Box::new(LookAtTargetSink::new(45, 90)))),
            (0, BehaviorSlot::new(Box::new(MoveToTargetSink::default()))),
            (
                0,
                BehaviorSlot::new(Box::new(ValidatePlayDead::default())),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(CountDownCooldownTicks::new(
                    MemoryModuleType::TemptationCooldownTicks,
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
                BehaviorSlot::new(Box::new(AnimalMakeLove::new(&EntityType::AXOLOTL, 0.2, 2))),
            ),
            (
                2,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![],
                    OrderPolicy::Ordered,
                    RunningPolicy::RunOne,
                    vec![
                        BehaviorSlot::new(Box::new(FollowTemptation::new(1.25, 2.5))),
                        BehaviorSlot::new(Box::new(BabyFollowAdult::new(ADULT_FOLLOW_RANGE, 1.25))),
                    ],
                ))),
            ),
            (
                3,
                BehaviorSlot::new(Box::new(StartAttacking::new(find_attack_target))),
            ),
            (3, BehaviorSlot::new(Box::new(TryFindWater::new(6, 0.15)))),
            (
                4,
                BehaviorSlot::new(Box::new(GateBehavior::new(
                    vec![(MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent)],
                    OrderPolicy::Ordered,
                    RunningPolicy::RunOne,
                    vec![
                        BehaviorSlot::new(Box::new(RandomStroll::swim(0.5))),
                        BehaviorSlot::new(Box::new(SetWalkTargetFromLookTarget::new(0.5, 3))),
                    ],
                ))),
            ),
        ],
        vec![],
        vec![],
    );

    brain.add_activity(
        Activity::Fight,
        vec![
            (
                0,
                BehaviorSlot::new(Box::new(StopAttackingIfTargetInvalid::default())),
            ),
            (
                0,
                BehaviorSlot::new(Box::new(SetWalkTargetFromAttackTarget::new(1.0))),
            ),
            (0, BehaviorSlot::new(Box::new(MeleeAttack::new(20)))),
            (
                0,
                BehaviorSlot::new(Box::new(EraseMemoryIf::new(
                    is_breeding,
                    MemoryModuleType::AttackTarget,
                ))),
            ),
        ],
        vec![(MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent)],
        vec![MemoryModuleType::AttackTarget],
    );

    brain.add_activity(
        Activity::PlayDead,
        vec![
            (0, BehaviorSlot::new(Box::new(PlayDead::default()))),
            (
                1,
                BehaviorSlot::new(Box::new(EraseMemoryIf::new(
                    is_breeding,
                    MemoryModuleType::PlayDeadTicks,
                ))),
            ),
        ],
        vec![(MemoryModuleType::PlayDeadTicks, MemoryStatus::ValuePresent)],
        vec![MemoryModuleType::PlayDeadTicks],
    );

    brain.set_core_activities(vec![Activity::Core]);
    brain.set_active_activity_if_possible(Activity::Idle);
    brain
}

/// Keeps the `Behavior` import meaningful for the predicate signatures above.
const _: fn(&BehaviorContext<'_, MobActor>) -> bool = is_breeding;
