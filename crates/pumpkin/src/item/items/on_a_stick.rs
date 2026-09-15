use std::any::Any;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;

pub struct CarrotOnAStickItem;
pub struct WarpedFungusOnAStickItem;

/// Shared `FoodOnAStickItem.use` logic (FoodOnAStickItem.java:23-39): boosts a ridden
/// entity of the matching type, damaging (and, on break, converting) whichever hand's
/// stack was actually used - not always the main hand.
fn use_on_a_stick(
    player: &Player,
    hand: pumpkin_util::Hand,
    vehicle_type: &EntityType,
    consume_item_damage: i32,
) {
    let vehicle_opt = player
        .get_entity()
        .vehicle
        .try_lock()
        .ok()
        .and_then(|guard| guard.clone());
    if let Some(vehicle) = vehicle_opt
        && vehicle.get_entity().entity_type.id == vehicle_type.id
        && let Some(steerable) = vehicle.get_item_steerable()
        && steerable.boost()
    {
        // FoodOnAStickItem.java:31-33: hurtAndConvertOnBreak damages the item in the
        // hand that was actually used and, on break, replaces it with a fishing rod.
        let slot = if hand == pumpkin_util::Hand::Right {
            EquipmentSlot::MAIN_HAND
        } else {
            EquipmentSlot::OFF_HAND
        };
        let before = player.inventory.get_stack_in_hand(hand);
        player.damage_item_in_slot(&slot, consume_item_damage);
        if !before.is_empty() && player.inventory.get_stack_in_hand(hand).is_empty() {
            player
                .inventory
                .set_stack_in_hand(hand, ItemStack::new(1, &Item::FISHING_ROD));
        }
    }
}

impl ItemMetadata for CarrotOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CARROT_ON_A_STICK.id])
    }
}

impl ItemBehaviour for CarrotOnAStickItem {
    fn use_stack(
        &self,
        _stack: &ItemStack,
        player: &Player,
        hand: pumpkin_util::Hand,
        _yaw: f32,
        _pitch: f32,
    ) {
        use_on_a_stick(player, hand, &EntityType::PIG, 7);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ItemMetadata for WarpedFungusOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WARPED_FUNGUS_ON_A_STICK.id])
    }
}

impl ItemBehaviour for WarpedFungusOnAStickItem {
    fn use_stack(
        &self,
        _stack: &ItemStack,
        player: &Player,
        hand: pumpkin_util::Hand,
        _yaw: f32,
        _pitch: f32,
    ) {
        use_on_a_stick(player, hand, &EntityType::STRIDER, 1);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
