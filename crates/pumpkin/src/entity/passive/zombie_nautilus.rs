use std::sync::Arc;

use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::ageable::AgeableMob;
use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{animal::Animal, nautilus::NautilusEntity},
};

/// The undead nautilus.
///
/// `ZombieNautilus.java` extends the shared `AbstractNautilus`, so riding,
/// dashing and the rider's breathing effect are all inherited. It differs by
/// never breeding and by using its own sounds, which have separate underwater
/// and on-land variants.
///
/// Shares an existing gap with the living nautilus: vanilla drives both from a
/// brain that makes them swim and wander, which is not implemented here, so
/// neither moves on its own. Tracked in SURVIVAL_PARITY_BACKLOG.md.
pub struct ZombieNautilusEntity {
    pub nautilus: Arc<NautilusEntity>,
}

impl ZombieNautilusEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Arc::new(Self {
            nautilus: NautilusEntity::new(entity),
        })
    }

    /// Vanilla picks the land variant of each sound when out of water.
    fn under_water(&self) -> bool {
        self.nautilus
            .get_mob_entity()
            .living_entity
            .entity
            .touching_water
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl CustomSound for ZombieNautilusEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(if self.under_water() {
            Sound::EntityZombieNautilusDeath
        } else {
            Sound::EntityZombieNautilusDeathLand
        })
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(if self.under_water() {
            Sound::EntityZombieNautilusHurt
        } else {
            Sound::EntityZombieNautilusHurtLand
        })
    }
}

impl Animal for ZombieNautilusEntity {
    fn is_food(&self, item_stack: &pumpkin_data::item_stack::ItemStack) -> bool {
        self.nautilus.is_food(item_stack)
    }

    /// `ZombieNautilus.getBreedOffspring` returns null: it never breeds.
    fn animal_interact(
        &self,
        _player: &Arc<crate::entity::player::Player>,
        _item_stack: &mut pumpkin_data::item_stack::ItemStack,
        _ambient_sound: Sound,
    ) -> bool {
        false
    }
}

impl AgeableMob for ZombieNautilusEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        self.nautilus.get_ageable_data()
    }
}

impl Mob for ZombieNautilusEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        self.nautilus.get_mob_entity()
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    /// Riding, dashing and the rider's breathing effect are shared behaviour.
    fn mob_interact(
        &self,
        player: &Arc<crate::entity::player::Player>,
        item_stack: &mut pumpkin_data::item_stack::ItemStack,
    ) -> bool {
        self.nautilus.mob_interact(player, item_stack)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.nautilus.mob_tick(caller);
    }

    fn post_tick(&self) {
        self.nautilus.post_tick();
    }

    fn mob_init_data_tracker(&self) {
        self.nautilus.mob_init_data_tracker();
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.nautilus.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.nautilus.mob_read_nbt(nbt);
    }
}
