use std::sync::{Arc, Mutex, Weak, atomic::Ordering};

use super::warden_emergence::Emergence;
use crate::entity::{EntityBase, spawn::SpawnReason};
use pumpkin_data::{
    damage::DamageType,
    entity::{EntityPose, EntityType},
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::{codec::var_int::VarInt, java::client::play::Metadata};
use pumpkin_util::{
    math::boundingbox::EntityDimensions, random::RandomImpl, version::JavaMinecraftVersion,
};

pub fn dimensions(pose: EntityPose) -> EntityDimensions {
    if matches!(pose, EntityPose::Emerging | EntityPose::Digging) {
        EntityDimensions::new(EntityType::WARDEN.dimension[0], 1.0, 0.85)
    } else {
        EntityDimensions::new(
            EntityType::WARDEN.dimension[0],
            EntityType::WARDEN.dimension[1],
            EntityType::WARDEN.eye_height,
        )
    }
}

use crate::entity::{
    Entity,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct WardenEntity {
    pub mob_entity: MobEntity,
    pub emergence: Mutex<Emergence>,
    random: Mutex<pumpkin_util::random::legacy_rand::LegacyRand>,
}

impl WardenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let warden = Self {
            mob_entity,
            emergence: Mutex::new(Emergence::default()),
            random: Mutex::new(pumpkin_util::random::legacy_rand::LegacyRand::from_seed(
                rand::random(),
            )),
        };
        let mob_arc = Arc::new(warden);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.5)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    fn is_digging_or_emerging(&self) -> bool {
        matches!(
            self.get_entity().pose.load(),
            EntityPose::Emerging | EntityPose::Digging
        )
    }
}

impl Mob for WardenEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        false
    }

    fn mob_finalize_spawn(&self, reason: SpawnReason) {
        let triggered = reason == SpawnReason::Triggered;
        self.emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finalize_spawn(triggered);
        if triggered {
            let entity = self.get_entity();
            entity.set_pose(EntityPose::Emerging);
            entity.world.load().play_sound_fine(
                Sound::EntityWardenAgitated,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        // Mob.finalizeSpawn uses the entity RNG; only the spawn yaw and the
        // Behavior duration draw use the world RNG. Do not perturb later attempts.
        crate::entity::spawn::apply_base_spawn(
            self,
            &mut *self
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let transition = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tick(world.get_world_age(), self.mob_entity.is_no_ai(), |bound| {
                world
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .next_bounded_i32(bound);
            });
        if transition.start {
            entity.set_pose(EntityPose::Emerging);
            world.play_sound_fine(
                Sound::EntityWardenEmerge,
                SoundCategory::Hostile,
                &entity.pos.load(),
                5.0,
                1.0,
            );
        }
        if transition.stop && entity.pose.load() == EntityPose::Emerging {
            entity.set_pose(EntityPose::Standing);
        }
    }

    fn run_goal_ai(&self) -> bool {
        !self.is_digging_or_emerging()
    }

    fn mob_is_pushable(&self) -> bool {
        !self.is_digging_or_emerging() && self.mob_entity.living_entity.is_pushable()
    }

    fn pre_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) -> bool {
        !self.is_digging_or_emerging()
            || damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
    }

    fn mob_java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        let mut metadata = Vec::new();
        Metadata::new(
            tracked_data::entity::DATA_POSE,
            VarInt(self.get_entity().pose.load() as i32),
        )
        .write(&mut metadata, &version)
        .ok()?;
        metadata.push(255);
        Some(metadata.into_boxed_slice())
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        let state = self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut memories = NbtCompound::new();
        for (name, ttl) in [
            ("minecraft:is_emerging", state.emerging_memory),
            ("minecraft:dig_cooldown", state.dig_cooldown),
        ] {
            if let Some(ttl) = ttl {
                let mut memory = NbtCompound::new();
                memory.put_compound("value", NbtCompound::new());
                if ttl != i64::MAX {
                    memory.put_long("ttl", ttl);
                }
                memories.put_compound(name, memory);
            }
        }
        let mut brain = NbtCompound::new();
        brain.put_compound("memories", memories);
        nbt.put_compound("Brain", brain);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        let memories = nbt
            .get_compound("Brain")
            .and_then(|b| b.get_compound("memories"));
        let read = |name| {
            memories
                .and_then(|m| m.get_compound(name))
                .filter(|m| m.get_compound("value").is_some())
                .map(|m| m.get_long("ttl").unwrap_or(i64::MAX))
        };
        *self
            .emergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Emergence {
            emerging_memory: read("minecraft:is_emerging"),
            dig_cooldown: read("minecraft:dig_cooldown"),
            ..Emergence::default()
        };
        // Running behavior/pose are not saved by Java. The loaded Brain selects
        // its activity, then Emerging starts again on its next eligible AI step.
        self.get_entity().data.store(
            i32::from(self.get_entity().pose.load() == EntityPose::Emerging),
            Ordering::Relaxed,
        );
    }
}
