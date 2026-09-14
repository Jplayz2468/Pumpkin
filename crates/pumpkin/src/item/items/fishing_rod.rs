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
        yaw: f32,
        pitch: f32,
    ) {
        self.use_rod(stack, player, hand, yaw, pitch);
    }

    fn normal_use(&self, _item: &Item, player: &Player) {
        self.use_rod(
            &player.inventory.held_item(),
            player,
            pumpkin_util::Hand::Right,
            player.living_entity.entity.yaw.load(),
            player.living_entity.entity.pitch.load(),
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
        yaw: f32,
        pitch: f32,
    ) {
        let world = player.world();
        let bobber_id = player.fishing_bobber.load(Ordering::Relaxed);

        if bobber_id == -1 {
            // Cast
            world.play_sound(
                Sound::EntityFishingBobberThrow,
                SoundCategory::Neutral,
                &player.position(),
            );

            let bobber_entity = Entity::new(
                world.clone(),
                player.position(),
                &EntityType::FISHING_BOBBER,
            );
            let bobber = FishingBobberEntity::with_rod(bobber_entity, player, stack);

            let look_vec = pumpkin_util::math::vector3::Vector3::new(
                -f64::from(yaw.to_radians().sin() * pitch.to_radians().cos()),
                -f64::from(pitch.to_radians().sin()),
                f64::from(yaw.to_radians().cos() * pitch.to_radians().cos()),
            );
            bobber
                .entity
                .velocity
                .store(look_vec.multiply(1.5, 1.5, 1.5));

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

            world.play_sound(
                Sound::EntityFishingBobberRetrieve,
                SoundCategory::Neutral,
                &player.position(),
            );
        }
    }
}
