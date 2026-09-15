use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::entity::Entity;
use crate::entity::player::Player;
use crate::entity::projectile::fishing_bobber::FishingBobberEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct FishingRodItem;

impl ItemMetadata for FishingRodItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::FISHING_ROD.id])
    }
}

impl ItemBehaviour for FishingRodItem {
    fn use_stack(
        &self,
        stack: &pumpkin_data::item_stack::ItemStack,
        player: &Player,
        hand: pumpkin_util::Hand,
        _yaw: f32,
        _pitch: f32,
    ) {
        self.use_rod(stack, player, hand);
    }

    fn normal_use(&self, _item: &Item, player: &Player) {
        self.use_rod(
            &player.inventory.held_item(),
            player,
            pumpkin_util::Hand::Right,
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FishingRodItem {
    fn use_rod(
        &self,
        stack: &pumpkin_data::item_stack::ItemStack,
        player: &Player,
        hand: pumpkin_util::Hand,
    ) {
        let world = player.world();
        let bobber_id = player.fishing_bobber.load(Ordering::Relaxed);

        if bobber_id == -1 {
            // Cast
            // FishingRodItem.java:42-51: volume 0.5, pitch = 0.4F / (random.nextFloat() * 0.4F + 0.8F)
            let pitch = 0.4 / (rand::random::<f32>() * 0.4 + 0.8);
            world.play_sound_fine(
                Sound::EntityFishingBobberThrow,
                SoundCategory::Neutral,
                &player.position(),
                0.5,
                pitch,
            );

            // Position, rotation and velocity are computed inside `with_rod` from the
            // owner's own entity rotation (FishingHook.java:82-107), not the packet's
            // yaw/pitch - see its doc comment.
            let bobber_entity = Entity::new(
                world.clone(),
                player.position(),
                &EntityType::FISHING_BOBBER,
            );
            let bobber = FishingBobberEntity::with_rod(bobber_entity, player, stack);

            player
                .fishing_bobber
                .store(bobber.entity.entity_id, Ordering::Relaxed);

            let bobber_arc: Arc<FishingBobberEntity> = Arc::new(bobber);
            world.spawn_entity(bobber_arc);
        } else {
            // Reel in
            if let Some(bobber_base) = world.get_entity_by_id(bobber_id) {
                if let Some(bobber) = bobber_base.cast_any().downcast_ref::<FishingBobberEntity>() {
                    let result = bobber.reel_in(player);
                    player.damage_item_in_slot(
                        &if hand == pumpkin_util::Hand::Right {
                            pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND
                        } else {
                            pumpkin_data::data_component_impl::EquipmentSlot::OFF_HAND
                        },
                        result,
                    );
                }
                bobber_base.get_entity().remove();
            }
            player.fishing_bobber.store(-1, Ordering::Relaxed);

            // FishingRodItem.java:30-39: volume 1.0, pitch = 0.4F / (random.nextFloat() * 0.4F + 0.8F)
            let pitch = 0.4 / (rand::random::<f32>() * 0.4 + 0.8);
            world.play_sound_fine(
                Sound::EntityFishingBobberRetrieve,
                SoundCategory::Neutral,
                &player.position(),
                1.0,
                pitch,
            );
        }
    }
}
