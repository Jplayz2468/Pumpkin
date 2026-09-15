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
    Entity, EntityBase, variant,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use pumpkin_nbt::compound::NbtCompound;

const TEMPT_ITEMS: &[&Item] = &[
    &Item::WHEAT_SEEDS,
    &Item::MELON_SEEDS,
    &Item::PUMPKIN_SEEDS,
    &Item::BEETROOT_SEEDS,
    &Item::TORCHFLOWER_SEEDS,
    &Item::PITCHER_POD,
];

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
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
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
const CHICKEN_SOUND_VARIANTS: i32 = 2;

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
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
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
        nbt.put_int("sound_variant", self.sound_variant.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.is_chicken_jockey
            .store(nbt.get_bool("IsChickenJockey").unwrap_or(false), Relaxed);
        self.egg_lay_time
            .store(nbt.get_int("EggLayTime").unwrap_or(6000), Ordering::Relaxed);
        if let Some(variant_str) = nbt.get_string("variant") {
            self.variant.store(
                variant::temperature_variant_from_name(variant_str),
                Ordering::Relaxed,
            );
        }
        if let Some(sound) = nbt.get_int("sound_variant") {
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
            variant::random_sound_variant(CHICKEN_SOUND_VARIANTS),
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
        if !self.is_baby()
            && !self.is_chicken_jockey.load(Relaxed)
            && self.egg_lay_time.fetch_sub(1, Ordering::Relaxed) <= 1
        {
            let next_time = rand::rng().random_range(6000..12000);
            let world = entity.world.load_full();
            let pos = entity.block_pos.load();
            let entity_id = entity.entity_id;
            let mut drop_event =
                crate::plugin::api::events::entity::entity_drop_item::EntityDropItemEvent::new(
                    entity_id,
                    "minecraft:egg".to_string(),
                    1,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut drop_event);
            }
            if !drop_event.cancelled {
                world.drop_stack(&pos, ItemStack::new(1, &Item::EGG));
            }
            self.egg_lay_time.store(next_time, Ordering::Relaxed);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use super::animal::Animal;
        self.animal_interact(player, item_stack, Sound::EntityChickenAmbient)
    }
}
