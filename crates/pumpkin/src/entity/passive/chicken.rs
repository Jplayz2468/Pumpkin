use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering, Ordering::Relaxed},
};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::{entity::EntityType, item::Item};
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
    variant,
};
use pumpkin_nbt::compound::NbtCompound;

/// Represents a Chicken, a passive mob that lays eggs and is immune to fall damage.
///
/// Wiki: <https://minecraft.wiki/w/Chicken>
pub struct ChickenEntity {
    pub mob_entity: MobEntity,
    pub variant: AtomicU8,
    /// Index into the `chicken_sound_variant` registry (classic, picky).
    pub sound_variant: AtomicI32,
    egg_lay_time: AtomicI32,
    pub is_chicken_jockey: AtomicBool,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

impl ChickenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let egg_lay_time = rand::rng().random_range(6000..12000);
        let chicken = Self {
            mob_entity,
            variant: AtomicU8::new(variant::TEMPERATURE_VARIANT_TEMPERATE),
            sound_variant: AtomicI32::new(0),
            egg_lay_time: AtomicI32::new(egg_lay_time),
            is_chicken_jockey: AtomicBool::new(false),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(chicken);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.4));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(
                3,
                Box::new(TemptGoal::with_tag(
                    1.0,
                    &pumpkin_data::tag::Item::MINECRAFT_CHICKEN_FOOD,
                )),
            );
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }
}

/// `chicken_sound_variant` registry size: classic, picky.
const CHICKEN_SOUND_VARIANTS: &[&str] = &["minecraft:classic", "minecraft:picky"];

impl ChickenEntity {
    /// Pushes both variant fields to the client.
    fn sync_variant(&self) {
        let entity = self.get_entity();
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::chicken::VARIANT,
            pumpkin_data::tracked_data::chicken::DATA_VARIANT_ID,
            VarInt(i32::from(self.variant.load(Ordering::Relaxed))),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::chicken::DATA_SOUND_VARIANT_ID,
            VarInt(self.sound_variant.load(Ordering::Relaxed)),
        );
    }
}

impl AgeableMob for ChickenEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for ChickenEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_CHICKEN_FOOD)
    }
}

/// The chicken behind an entity reference, if it is one.
fn chicken_of(entity: &dyn EntityBase) -> Option<&ChickenEntity> {
    entity.get_mob().and_then(Mob::as_chicken)
}

impl Mob for ChickenEntity {
    /// `Chicken.getBreedOffspring` (Chicken.java): the chick takes one parent's variant.
    fn mob_inherit_from_parents(&self, first: &dyn EntityBase, second: &dyn EntityBase) {
        let (Some(a), Some(b)) = (chicken_of(first), chicken_of(second)) else {
            return;
        };
        self.variant.store(
            variant::inherit_variant(
                a.variant.load(Ordering::Relaxed),
                b.variant.load(Ordering::Relaxed),
            ),
            Ordering::Relaxed,
        );
        self.sync_variant();
    }

    fn as_chicken(&self) -> Option<&ChickenEntity> {
        Some(self)
    }

    fn remove_when_far_away(&self, _: f64) -> bool {
        self.is_chicken_jockey.load(Relaxed)
    }

    fn get_base_experience_reward(&self) -> u32 {
        if self.is_chicken_jockey.load(Relaxed) {
            10
        } else {
            self.get_entity().entity_type.experience_reward
        }
    }
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("IsChickenJockey", self.is_chicken_jockey.load(Relaxed));
        nbt.put_int("EggLayTime", self.egg_lay_time.load(Ordering::Relaxed));
        nbt.put_string(
            "variant",
            variant::temperature_variant_name(self.variant.load(Ordering::Relaxed)).to_string(),
        );
        if let Some(name) =
            CHICKEN_SOUND_VARIANTS.get(self.sound_variant.load(Ordering::Relaxed) as usize)
        {
            nbt.put_string("sound_variant", (*name).to_owned());
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.is_chicken_jockey
            .store(nbt.get_bool("IsChickenJockey").unwrap_or(false), Relaxed);
        if let Some(time) = nbt.get_int("EggLayTime") {
            self.egg_lay_time.store(time, Ordering::Relaxed);
        }
        if let Some(variant_str) = nbt.get_string("variant") {
            self.variant.store(
                variant::temperature_variant_from_name(variant_str),
                Ordering::Relaxed,
            );
        }
        let sound = nbt
            .get_string("sound_variant")
            .and_then(|name| {
                CHICKEN_SOUND_VARIANTS.iter().position(|candidate| {
                    *candidate == name || candidate.strip_prefix("minecraft:") == Some(name)
                })
            })
            .map(|index| index as i32)
            .or_else(|| {
                nbt.get_int("sound_variant")
                    .filter(|id| (0..CHICKEN_SOUND_VARIANTS.len() as i32).contains(id))
            });
        if let Some(sound) = sound {
            self.sound_variant.store(sound, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.variant.store(
            variant::temperature_variant_from_name(name),
            Ordering::Relaxed,
        );
        self.sync_variant();
    }

    /// `Chicken.finalizeSpawn` (Chicken.java): the variant comes from the biome, the
    /// sound variant is an unweighted draw over its registry.
    fn mob_finalize_spawn(&self, _reason: crate::entity::spawn::SpawnReason) {
        let entity = self.get_entity();
        let world = entity.world.load();
        self.variant.store(
            variant::temperature_variant_at(&world, &entity.block_pos.load()),
            Ordering::Relaxed,
        );
        self.sound_variant.store(
            world.rand_bounded_i32(CHICKEN_SOUND_VARIANTS.len() as i32),
            Ordering::Relaxed,
        );
        self.sync_variant();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::chicken::BABY_ID, true);
        }
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::chicken::VARIANT,
            pumpkin_data::tracked_data::chicken::DATA_VARIANT_ID,
            VarInt(self.variant.load(Ordering::Relaxed) as i32),
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if self.mob_entity.living_entity.dead.load(Relaxed) {
            return;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let current_velocity = entity.velocity.load();
        let on_ground = entity.on_ground.load(Ordering::Relaxed);

        // TODO: move velocity logic to physics tick when implemented
        if (!on_ground) && current_velocity.y < 0.0 {
            entity.set_velocity(current_velocity.multiply(1.0, 0.6, 1.0));
        }
    }

    // Chicken.aiStep calls the ageable superclass before the egg timer.
    fn post_tick(&self) {
        if self.mob_entity.living_entity.dead.load(Relaxed) {
            return;
        }
        let entity = &self.mob_entity.living_entity.entity;
        if entity.is_alive()
            && self.mob_entity.living_entity.health.load() > 0.0
            && !self.is_baby()
            && !self.is_chicken_jockey.load(Relaxed)
            && self.egg_lay_time.fetch_sub(1, Ordering::Relaxed) <= 1
        {
            let world = entity.world.load_full();
            let egg = match self.variant.load(Relaxed) {
                variant::TEMPERATURE_VARIANT_COLD => &Item::BLUE_EGG,
                variant::TEMPERATURE_VARIANT_WARM => &Item::BROWN_EGG,
                _ => &Item::EGG,
            };
            let entity_id = entity.entity_id;
            let mut drop_event =
                crate::plugin::api::events::entity::entity_drop_item::EntityDropItemEvent::new(
                    entity_id,
                    format!("minecraft:{}", egg.registry_key),
                    1,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut drop_event);
            }
            if !drop_event.cancelled
                && drop_event.count > 0
                && let Some(dropped_item) = Item::from_registry_key(
                    drop_event
                        .item_name
                        .strip_prefix("minecraft:")
                        .unwrap_or(&drop_event.item_name),
                )
            {
                let dropped = crate::entity::item::ItemEntity::new(
                    Entity::new(world.clone(), entity.pos.load(), &EntityType::ITEM),
                    ItemStack::new(drop_event.count, dropped_item),
                );
                world.spawn_entity(Arc::new(dropped));
                let mut random = rand::rng();
                let pitch = (random.random::<f32>() - random.random::<f32>()) * 0.2 + 1.0;
                world.play_sound_fine(
                    Sound::EntityChickenEgg,
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    pitch,
                );
                world.emit_game_event_from_entity(
                    pumpkin_data::game_event::GameEvent::EntityPlace.name(),
                    entity.pos.load(),
                    Some(self),
                    None,
                );
            }
            self.egg_lay_time
                .store(rand::rng().random_range(6000..12000), Ordering::Relaxed);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use super::animal::Animal;
        self.animal_interact(player, item_stack, Sound::EntityChickenAmbient)
    }
}
