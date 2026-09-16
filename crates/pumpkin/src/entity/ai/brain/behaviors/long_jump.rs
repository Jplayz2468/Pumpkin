use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::long_jump_util::calculate_jump_vector_for_angle;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::entity::EntityPose;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::position::BlockPos;
use rand::RngExt;
use std::sync::atomic::Ordering::Relaxed;

/// Vanilla tries these launch angles in order, preferring the flattest that works.
const JUMP_ANGLES: [i32; 4] = [65, 70, 75, 80];
const TIME_OUT: i32 = 100;

/// Port of `LongJumpToRandomPos`, with `LongJumpToPreferredBlock`'s bias folded in.
///
/// Picks a landing spot within range, solves for a launch velocity that reaches it, and
/// launches. A preferred block tag biases the choice, which is how a frog favours lily
/// pads.
///
/// Known gaps: vanilla collects every candidate, weights them and re-picks on failure;
/// this takes the first spot that yields a valid trajectory. The trajectory collision
/// sweep is also absent -- see `long_jump_util`.
pub struct LongJump {
    time_between_jumps: (i32, i32),
    max_height: i32,
    max_width: i32,
    max_jump_velocity: f32,
    jump_sound: Sound,
    preferred_blocks: Option<&'static pumpkin_data::tag::Tag>,
    preferred_chance: f32,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl LongJump {
    #[must_use]
    pub fn new(
        time_between_jumps: (i32, i32),
        max_height: i32,
        max_width: i32,
        max_jump_velocity: f32,
        jump_sound: Sound,
        preferred_blocks: Option<&'static pumpkin_data::tag::Tag>,
        preferred_chance: f32,
    ) -> Self {
        Self {
            time_between_jumps,
            max_height,
            max_width,
            max_jump_velocity,
            jump_sound,
            preferred_blocks,
            preferred_chance,
            conditions: [
                (
                    MemoryModuleType::LongJumpCoolingDown,
                    MemoryStatus::ValueAbsent,
                ),
                (MemoryModuleType::LongJumpMidJump, MemoryStatus::ValueAbsent),
            ],
        }
    }
}

impl Behavior<MobActor> for LongJump {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (TIME_OUT, TIME_OUT)
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        ctx.actor
            .entity()
            .is_some_and(|entity| entity.get_entity().on_ground.load(Relaxed))
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(me) = ctx.actor.entity() else {
            return;
        };
        let entity = me.get_entity();
        let origin = BlockPos::floored(
            ctx.actor.position.x,
            ctx.actor.position.y,
            ctx.actor.position.z,
        );
        let world = &ctx.actor.world;

        let want_preferred = self
            .preferred_blocks
            .is_some_and(|_| rand::rng().random::<f32>() < self.preferred_chance);

        let mut fallback = None;
        for dx in -self.max_width..=self.max_width {
            for dz in -self.max_width..=self.max_width {
                for dy in -self.max_height..=self.max_height {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let landing = BlockPos::new(origin.0.x + dx, origin.0.y + dy, origin.0.z + dz);
                    let above = BlockPos::new(landing.0.x, landing.0.y + 1, landing.0.z);
                    // Somewhere to stand: solid block with clear air over it.
                    if world.get_block(&landing).id == pumpkin_data::Block::AIR.id
                        || world.get_block(&above).id != pumpkin_data::Block::AIR.id
                    {
                        continue;
                    }
                    let target = above.0.to_f64();
                    let Some(velocity) = JUMP_ANGLES.iter().find_map(|angle| {
                        calculate_jump_vector_for_angle(
                            entity,
                            target,
                            self.max_jump_velocity,
                            *angle,
                        )
                    }) else {
                        continue;
                    };

                    let preferred = self.preferred_blocks.is_some_and(|tag| {
                        use pumpkin_data::tag::Taggable;
                        world.get_block(&landing).has_tag(tag)
                    });
                    if want_preferred && !preferred {
                        if fallback.is_none() {
                            fallback = Some(velocity);
                        }
                        continue;
                    }

                    entity.set_velocity(velocity);
                    ctx.memories
                        .set(MemoryModuleType::LongJumpMidJump, MemoryValue::Bool(true));
                    world.play_sound(
                        self.jump_sound,
                        SoundCategory::Neutral,
                        &ctx.actor.position,
                    );
                    return;
                }
            }
        }

        // Nothing preferred was reachable; take whatever was.
        if let Some(velocity) = fallback {
            entity.set_velocity(velocity);
            ctx.memories
                .set(MemoryModuleType::LongJumpMidJump, MemoryValue::Bool(true));
            world.play_sound(self.jump_sound, SoundCategory::Neutral, &ctx.actor.position);
        }
    }

    fn debug_name(&self) -> &'static str {
        "long_jump"
    }
}

/// Port of `LongJumpMidJump`.
///
/// Holds the jumping pose while airborne, then on landing damps the horizontal velocity,
/// plays the landing sound and starts the cooldown before the next jump.
pub struct LongJumpMidJump {
    time_between_jumps: (i32, i32),
    landing_sound: Sound,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl LongJumpMidJump {
    #[must_use]
    pub fn new(time_between_jumps: (i32, i32), landing_sound: Sound) -> Self {
        Self {
            time_between_jumps,
            landing_sound,
            conditions: [
                (MemoryModuleType::LookTarget, MemoryStatus::Registered),
                (
                    MemoryModuleType::LongJumpMidJump,
                    MemoryStatus::ValuePresent,
                ),
            ],
        }
    }
}

impl Behavior<MobActor> for LongJumpMidJump {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn duration_range(&self) -> (i32, i32) {
        (TIME_OUT, TIME_OUT)
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        !ctx.actor
            .entity()
            .is_some_and(|entity| entity.get_entity().on_ground.load(Relaxed))
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        if let Some(me) = ctx.actor.entity() {
            me.get_entity().set_pose(EntityPose::LongJumping);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        if let Some(me) = ctx.actor.entity() {
            let entity = me.get_entity();
            if entity.on_ground.load(Relaxed) {
                let velocity = entity.velocity.load();
                entity.set_velocity(pumpkin_util::math::vector3::Vector3::new(
                    velocity.x * 0.1,
                    velocity.y,
                    velocity.z * 0.1,
                ));
                ctx.actor.world.play_sound(
                    self.landing_sound,
                    SoundCategory::Neutral,
                    &ctx.actor.position,
                );
            }
            entity.set_pose(EntityPose::Standing);
        }
        ctx.memories.erase(MemoryModuleType::LongJumpMidJump);
        let (min, max) = self.time_between_jumps;
        let cooldown = min + rand::rng().random_range(0..=(max - min).max(0));
        ctx.memories.set(
            MemoryModuleType::LongJumpCoolingDown,
            MemoryValue::Int(cooldown),
        );
    }

    fn debug_name(&self) -> &'static str {
        "long_jump_mid_jump"
    }
}
