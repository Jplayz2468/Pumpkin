use std::any::Any;
use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::experience_bottle::ThrownExperienceBottleEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct ExperienceBottleItem;

impl ItemMetadata for ExperienceBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::EXPERIENCE_BOTTLE.id])
    }
}

// ExperienceBottleItem.java:35: `Projectile.spawnProjectileFromRotation(..., player, -20.0F, 0.7F, 1.0F)`.
const PITCH_OFFSET: f32 = -20.0;
const POWER: f32 = 0.7;
const DIVERGENCE: f32 = 1.0;
const THROW_SOUND_VOLUME: f32 = 0.5;

impl ItemBehaviour for ExperienceBottleItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let world = player.world();
        let position = player.position();

        // ExperienceBottleItem.java:24-33: SoundSource.NEUTRAL, volume 0.5, pitch
        // 0.4F / (random.nextFloat() * 0.4F + 0.8F).
        world.play_sound_fine(
            Sound::EntityExperienceBottleThrow,
            SoundCategory::Neutral,
            &position,
            THROW_SOUND_VOLUME,
            0.4 / (rand::random::<f32>() * 0.4 + 0.8),
        );

        let entity = Entity::new(world.clone(), position, &EntityType::EXPERIENCE_BOTTLE);
        let bottle = ThrownExperienceBottleEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        bottle
            .thrown
            .set_velocity_from(pitch, yaw, PITCH_OFFSET, POWER, DIVERGENCE);
        world.spawn_entity(Arc::new(bottle));

        // Consume item
        let mut main_hand = player.inventory().held_item();
        let consumed = if !main_hand.is_empty() && main_hand.item.id == Item::EXPERIENCE_BOTTLE.id
        {
            main_hand.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory().set_held_item(main_hand);
            true
        } else {
            false
        };

        if !consumed {
            let mut off_hand = player.inventory().off_hand_item();
            if !off_hand.is_empty() && off_hand.item.id == Item::EXPERIENCE_BOTTLE.id {
                off_hand.decrement_unless_creative(player.gamemode.load(), 1);
                player
                    .inventory()
                    .set_stack_in_hand(pumpkin_util::Hand::Left, off_hand);
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
