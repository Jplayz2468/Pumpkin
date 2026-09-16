use std::sync::Arc;

use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::ageable::AgeableMob;
use crate::entity::player::Player;
use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{animal::Animal, camel::CamelEntity},
};

/// A camel husk: the undead camel of the desert.
///
/// `CamelHusk.java` is a thin override of `Camel`. Everything about movement,
/// dashing, sitting and riding is shared; what differs is that it never breeds,
/// is never a baby, always despawns when far away, eats a different food, and
/// has its own sounds.
pub struct CamelHuskEntity {
    pub camel: Arc<CamelEntity>,
}

impl CamelHuskEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Arc::new(Self {
            camel: CamelEntity::new(entity),
        })
    }
}

impl CustomSound for CamelHuskEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCamelHuskDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCamelHuskHurt)
    }
}

impl Animal for CamelHuskEntity {
    /// `CamelHusk.isFood`: the husk's own tag, not the camel's.
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_CAMEL_HUSK_FOOD)
    }

    /// `CamelHusk.canFallInLove` is false, so feeding never starts breeding.
    /// The default would have entered love mode for any food item.
    fn animal_interact(
        &self,
        _player: &Arc<Player>,
        _item_stack: &mut ItemStack,
        _ambient_sound: Sound,
    ) -> bool {
        false
    }
}

impl AgeableMob for CamelHuskEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        self.camel.get_ageable_data()
    }

    /// `CamelHusk.canBeABaby` is false.
    fn is_baby(&self) -> bool {
        false
    }
}

impl Mob for CamelHuskEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        self.camel.get_mob_entity()
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn is_saddled(&self) -> bool {
        self.camel.is_saddled()
    }

    /// `CamelHusk` inherits `Camel`'s saddling and riding. It cannot delegate to
    /// `CamelEntity::mob_interact`, because that would finish by running the
    /// camel's own food rules -- cactus, and love mode -- so the shared branches
    /// are repeated here and the food path deliberately falls through.
    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use pumpkin_data::item::Item;
        use pumpkin_data::sound::SoundCategory;

        if item_stack.get_item() == &Item::SADDLE && !self.camel.is_saddled() {
            self.camel.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityCamelHuskSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if self.camel.is_saddled() && !self.is_food(item_stack) {
            let world = player.world();
            let entity = self.get_entity();
            if let Some(vehicle) = world.get_entity_by_id(entity.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                entity.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        // `CamelHusk` never falls in love and is never a baby, so neither the
        // love-mode nor the age-up branch of Animal.mobInteract can run.
        false
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.camel.mob_tick(caller);
    }

    fn post_tick(&self) {
        self.camel.post_tick();
    }

    fn mob_init_data_tracker(&self) {
        self.camel.mob_init_data_tracker();
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.camel.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.camel.mob_read_nbt(nbt);
    }
}
