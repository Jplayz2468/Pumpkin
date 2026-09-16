use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::move_to_target_sink::set_walk_target;
use crate::entity::ai::brain::behaviors::start_attacking::attack_target;
use crate::entity::ai::brain::memory::MemoryStatus;
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::entity::EntityPose;
use pumpkin_data::sound::{Sound, SoundCategory};
use std::sync::atomic::Ordering::Relaxed;

const TIME_OUT: i32 = 100;
const CATCH_ANIMATION_DURATION: i32 = 6;
const TONGUE_ANIMATION_DURATION: i32 = 10;
const EATING_DISTANCE: f64 = 1.75;
const EATING_MOVEMENT_FACTOR: f64 = 0.75;
const RECALCULATE_PATH_TICKS: i32 = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    MoveToTarget,
    CatchAnimation,
    EatAnimation,
    Done,
}

/// Port of the frog's `ShootTongue`.
///
/// A four-state machine: close the distance, snap the tongue out and drag the prey in,
/// eat it, then finish. The pose changes drive the client animation.
///
/// Known gap: vanilla first checks it can actually path to the target and, if not, files
/// it in an unreachable-targets memory for 100 ticks so the frog stops retrying. That
/// memory is not maintained here, so a frog may keep selecting prey it cannot reach.
pub struct ShootTongue {
    tongue_sound: Sound,
    eat_sound: Sound,
    state: State,
    eat_animation_timer: i32,
    calculate_path_counter: i32,
    conditions: [(MemoryModuleType, MemoryStatus); 4],
}

impl ShootTongue {
    #[must_use]
    pub fn new(tongue_sound: Sound, eat_sound: Sound) -> Self {
        Self {
            tongue_sound,
            eat_sound,
            state: State::Done,
            eat_animation_timer: 0,
            calculate_path_counter: 0,
            conditions: [
                (MemoryModuleType::WalkTarget, MemoryStatus::ValueAbsent),
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
                (MemoryModuleType::IsPanicking, MemoryStatus::ValueAbsent),
            ],
        }
    }

    fn eat_entity(&self, ctx: &BehaviorContext<'_, MobActor>) {
        let Some(me) = ctx.actor.entity() else {
            return;
        };
        ctx.actor.world.play_sound(
            self.eat_sound,
            SoundCategory::Neutral,
            &ctx.actor.position,
        );
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let Some(target) = ctx.actor.world.get_entity_by_id(target_id) else {
            return;
        };
        if let Some(mob) = me.get_mob() {
            mob.get_mob_entity().try_attack(&*me, &*target);
        }
    }
}

impl Behavior<MobActor> for ShootTongue {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (TIME_OUT, TIME_OUT)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        // Vanilla also refuses while croaking.
        ctx.actor
            .entity()
            .is_some_and(|entity| entity.get_entity().pose.load() != EntityPose::Croaking)
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        attack_target(ctx.memories).is_some()
            && self.state != State::Done
            && !ctx.memories.has(MemoryModuleType::IsPanicking)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let Some(target) = ctx.actor.world.get_entity_by_id(target_id) else {
            return;
        };
        let pos = target.get_entity().pos.load();
        set_walk_target(ctx.memories, pos, 2.0);
        self.calculate_path_counter = RECALCULATE_PATH_TICKS;
        self.state = State::MoveToTarget;
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let Some(target) = ctx.actor.world.get_entity_by_id(target_id) else {
            return;
        };
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();

        match self.state {
            State::MoveToTarget => {
                let distance = ctx
                    .actor
                    .position
                    .squared_distance_to(target_pos.x, target_pos.y, target_pos.z)
                    .sqrt();
                if distance < EATING_DISTANCE {
                    ctx.actor.world.play_sound(
                        self.tongue_sound,
                        SoundCategory::Neutral,
                        &ctx.actor.position,
                    );
                    if let Some(me) = ctx.actor.entity() {
                        me.get_entity().set_pose(EntityPose::UsingTongue);
                    }
                    // Drag the prey toward the frog.
                    let pull = (ctx.actor.position - target_pos).normalize() * EATING_MOVEMENT_FACTOR;
                    target_entity.set_velocity(pull);
                    self.eat_animation_timer = 0;
                    self.state = State::CatchAnimation;
                } else if self.calculate_path_counter <= 0 {
                    set_walk_target(ctx.memories, target_pos, 2.0);
                    self.calculate_path_counter = RECALCULATE_PATH_TICKS;
                } else {
                    self.calculate_path_counter -= 1;
                }
            }
            State::CatchAnimation => {
                self.eat_animation_timer += 1;
                if self.eat_animation_timer >= CATCH_ANIMATION_DURATION {
                    self.state = State::EatAnimation;
                    self.eat_entity(ctx);
                }
            }
            State::EatAnimation => {
                if self.eat_animation_timer >= TONGUE_ANIMATION_DURATION {
                    self.state = State::Done;
                } else {
                    self.eat_animation_timer += 1;
                }
            }
            State::Done => {}
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.erase(MemoryModuleType::AttackTarget);
        if let Some(me) = ctx.actor.entity() {
            me.get_entity().set_pose(EntityPose::Standing);
            // `eraseTongueTarget`: clears the synced id the client animates from.
            if let Some(frog) = me
                .cast_any()
                .downcast_ref::<crate::entity::passive::frog::FrogEntity>()
            {
                frog.tongue_target_id.store(-1, Relaxed);
            }
        }
        self.state = State::Done;
    }

    fn debug_name(&self) -> &'static str {
        "shoot_tongue"
    }
}
