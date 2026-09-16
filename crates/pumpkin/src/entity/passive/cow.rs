use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::{entity::EntityType, item::Item};

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

/// Represents a Cow, a common passive mob that provides milk, leather, and beef.
///
/// Wiki: <https://minecraft.wiki/w/Cow>
pub struct CowEntity {
    pub mob_entity: MobEntity,
    /// Index into the `cow_variant` registry (cold, temperate, warm).
    pub variant: AtomicU8,
    /// Index into the `cow_variant_sound` registry (classic, moody).
    pub sound_variant: AtomicI32,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

/// `cow_sound_variant` registry size: classic, moody.
const COW_SOUND_VARIANTS: &[&str] = &["minecraft:classic", "minecraft:moody"];

impl CowEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let cow = Self {
            mob_entity,
            variant: AtomicU8::new(variant::TEMPERATURE_VARIANT_TEMPERATE),
            sound_variant: AtomicI32::new(0),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(cow);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(
                3,
                Box::new(TemptGoal::with_tag(
                    1.25,
                    &pumpkin_data::tag::Item::MINECRAFT_COW_FOOD,
                )),
            );
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
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

impl CowEntity {
    /// Pushes both variant fields to the client.
    fn sync_variant(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::cow::DATA_VARIANT_ID,
            VarInt(i32::from(self.variant.load(Ordering::Relaxed))),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cow::DATA_SOUND_VARIANT_ID,
            VarInt(self.sound_variant.load(Ordering::Relaxed)),
        );
    }
}

impl AgeableMob for CowEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for CowEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_COW_FOOD)
    }
}

/// The cow behind an entity reference, if it is one.
fn cow_of(entity: &dyn EntityBase) -> Option<&CowEntity> {
    entity.get_mob().and_then(Mob::as_cow)
}

impl Mob for CowEntity {
    fn as_cow(&self) -> Option<&CowEntity> {
        Some(self)
    }

    /// `Cow.getBreedOffspring` (Cow.java): the calf takes one parent's variant at random.
    fn mob_inherit_from_parents(&self, first: &dyn EntityBase, second: &dyn EntityBase) {
        let (Some(a), Some(b)) = (cow_of(first), cow_of(second)) else {
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

    /// `Cow.finalizeSpawn` (Cow.java): the variant is chosen from the biome via
    /// `VariantUtils.selectVariantToSpawn`, the sound variant is an unweighted draw.
    fn mob_finalize_spawn(&self, _reason: crate::entity::spawn::SpawnReason) {
        let entity = self.get_entity();
        let world = entity.world.load();
        self.variant.store(
            variant::temperature_variant_at(&world, &entity.block_pos.load()),
            Ordering::Relaxed,
        );
        self.sound_variant.store(
            world.rand_bounded_i32(COW_SOUND_VARIANTS.len() as i32),
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
        nbt.put_string(
            "variant",
            variant::temperature_variant_name(self.variant.load(Ordering::Relaxed)).to_string(),
        );
        if let Some(name) =
            COW_SOUND_VARIANTS.get(self.sound_variant.load(Ordering::Relaxed) as usize)
        {
            nbt.put_string("sound_variant", (*name).to_owned());
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(name) = nbt.get_string("variant") {
            self.variant.store(
                variant::temperature_variant_from_name(name),
                Ordering::Relaxed,
            );
        }
        let sound = nbt
            .get_string("sound_variant")
            .and_then(|name| {
                COW_SOUND_VARIANTS.iter().position(|candidate| {
                    *candidate == name || candidate.strip_prefix("minecraft:") == Some(name)
                })
            })
            .map(|index| index as i32)
            .or_else(|| {
                nbt.get_int("sound_variant")
                    .filter(|id| (0..COW_SOUND_VARIANTS.len() as i32).contains(id))
            });
        if let Some(sound) = sound {
            self.sound_variant.store(sound, Ordering::Relaxed);
        }
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.get_item() == &Item::BUCKET && !self.is_baby() {
            crate::item::items::bucket::exchange_bucket_stack(
                player,
                item_stack,
                &Item::MILK_BUCKET,
            );
            let entity = &self.mob_entity.living_entity.entity;
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityCowMilk,
                pumpkin_data::sound::SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }
        self.animal_interact(player, item_stack, Sound::EntityCowAmbient)
    }
}
