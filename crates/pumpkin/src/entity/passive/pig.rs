use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use pumpkin_protocol::codec::var_int::VarInt;

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::{entity::EntityType, item::Item};

use crate::entity::{
    Entity,
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

use crate::entity::EntityBase;
use crate::entity::item_steerable::{ItemBasedSteering, ItemSteerable};

/// Represents a Pig, a common passive mob that provides porkchops.
///
/// Wiki: <https://minecraft.wiki/w/Pig>
pub struct PigEntity {
    pub mob_entity: MobEntity,
    /// Index into the `pig_variant` registry (cold, temperate, warm).
    pub variant: AtomicU8,
    /// Index into the `pig_sound_variant` registry (big, classic, mini).
    pub sound_variant: AtomicI32,
    pub ageable_data: crate::entity::ageable::AgeableData,
    pub steering: ItemBasedSteering,
    pub saddled: std::sync::atomic::AtomicBool,
}

impl PigEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let pig = Self {
            mob_entity,
            variant: AtomicU8::new(variant::TEMPERATURE_VARIANT_TEMPERATE),
            sound_variant: AtomicI32::new(1),
            ageable_data: crate::entity::ageable::AgeableData::default(),
            steering: ItemBasedSteering::default(),
            saddled: std::sync::atomic::AtomicBool::new(false),
        };
        let mob_arc = Arc::new(pig);
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

            // Pig.java:81-89 — vanilla leaves priority 2 unused (Breed is 3, not 2); every
            // goal from BreedGoal onward is shifted one slot lower than the naive sequence.
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(3, BreedGoal::new(1.0));
            goal_selector.add_goal(
                4,
                Box::new(TemptGoal::new(1.2, &[&Item::CARROT_ON_A_STICK])),
            );
            goal_selector.add_goal(
                4,
                Box::new(TemptGoal::with_tag(
                    1.2,
                    &pumpkin_data::tag::Item::MINECRAFT_PIG_FOOD,
                )),
            );
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }
}

/// `pig_sound_variant` registry size: big, classic, mini.
const PIG_SOUND_VARIANTS: &[&str] = &["minecraft:big", "minecraft:classic", "minecraft:mini"];

impl PigEntity {
    /// Pushes both variant fields to the client.
    fn sync_variant(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::pig::DATA_VARIANT_ID,
            VarInt(i32::from(self.variant.load(Ordering::Relaxed))),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::pig::DATA_SOUND_VARIANT_ID,
            VarInt(self.sound_variant.load(Ordering::Relaxed)),
        );
    }
}

impl AgeableMob for PigEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for PigEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        // Pig.java:252-254 `isFood` only checks ItemTags.PIG_FOOD (carrot/potato/beetroot).
        // Carrot on a stick tempts (see PIG_FOOD/TemptGoal above) but is NOT breeding food in
        // vanilla, so it must not be included here.
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_PIG_FOOD)
    }
}

/// The pig behind an entity reference, if it is one.
fn pig_of(entity: &dyn EntityBase) -> Option<&PigEntity> {
    entity.get_mob().and_then(Mob::as_pig)
}

impl Mob for PigEntity {
    fn as_pig(&self) -> Option<&PigEntity> {
        Some(self)
    }

    /// `Pig.getBreedOffspring` (Pig.java): the piglet takes one parent's variant.
    fn mob_inherit_from_parents(&self, first: &dyn EntityBase, second: &dyn EntityBase) {
        let (Some(a), Some(b)) = (pig_of(first), pig_of(second)) else {
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

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    /// `Pig.finalizeSpawn` (Pig.java): biome-chosen variant, unweighted sound variant.
    fn mob_finalize_spawn(&self, _reason: crate::entity::spawn::SpawnReason) {
        let entity = self.get_entity();
        let world = entity.world.load();
        self.variant.store(
            variant::temperature_variant_at(&world, &entity.block_pos.load()),
            Ordering::Relaxed,
        );
        self.sound_variant.store(
            world.rand_bounded_i32(PIG_SOUND_VARIANTS.len() as i32),
            Ordering::Relaxed,
        );
        self.sync_variant();
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.variant.store(
            variant::temperature_variant_from_name(name),
            Ordering::Relaxed,
        );
        self.sync_variant();
    }

    fn mob_init_data_tracker(&self) {
        self.sync_variant();
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("Saddle", self.is_saddled());
        nbt.put_string(
            "variant",
            variant::temperature_variant_name(self.variant.load(Ordering::Relaxed)).to_string(),
        );
        if let Some(name) =
            PIG_SOUND_VARIANTS.get(self.sound_variant.load(Ordering::Relaxed) as usize)
        {
            nbt.put_string("sound_variant", (*name).to_owned());
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(saddle) = nbt.get_byte("Saddle") {
            self.set_saddled(saddle == 1);
        }
        if let Some(name) = nbt.get_string("variant") {
            self.variant.store(
                variant::temperature_variant_from_name(name),
                Ordering::Relaxed,
            );
        }
        let sound = nbt
            .get_string("sound_variant")
            .and_then(|name| {
                PIG_SOUND_VARIANTS.iter().position(|candidate| {
                    *candidate == name || candidate.strip_prefix("minecraft:") == Some(name)
                })
            })
            .map(|index| index as i32)
            .or_else(|| {
                nbt.get_int("sound_variant")
                    .filter(|id| (0..PIG_SOUND_VARIANTS.len() as i32).contains(id))
            });
        if let Some(sound) = sound {
            self.sound_variant.store(sound, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_item_steerable(&self) -> Option<&dyn ItemSteerable> {
        Some(self)
    }

    fn is_saddled(&self) -> bool {
        self.saddled.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn can_be_saddled(&self) -> bool {
        use crate::entity::ageable::AgeableMob;
        self.mob_entity.living_entity.entity.is_alive() && !self.is_baby()
    }

    fn set_saddled(&self, saddled: bool) {
        self.saddled
            .store(saddled, std::sync::atomic::Ordering::Relaxed);
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use super::animal::Animal;
        if item_stack.get_item() == &pumpkin_data::item::Item::SADDLE
            && self.can_be_saddled()
            && !self.is_saddled()
        {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.play_sound(
                Sound::EntityPigSaddle,
                pumpkin_data::sound::SoundCategory::Neutral,
                &pos,
            );
            return true;
        }

        if self.is_saddled()
            && !self.is_food(item_stack)
            && !self.get_entity().has_passengers()
            && !player.get_entity().is_sneaking()
        {
            let world = player.world();
            if let Some(vehicle) = world.get_entity_by_id(self.get_entity().entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                self.get_entity()
                    .add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }
        self.animal_interact(player, item_stack, Sound::EntityPigAmbient)
    }
}

impl ItemSteerable for PigEntity {
    fn boost(&self) -> bool {
        self.steering.boost()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
