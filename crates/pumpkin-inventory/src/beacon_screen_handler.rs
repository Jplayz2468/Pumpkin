use crate::{
    inventory::{Inventory, SimpleInventory},
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenProperty},
    slot::{NormalSlot, Slot},
    window_property::PropertyDelegate,
};
use pumpkin_data::{
    BlockId,
    item_stack::ItemStack,
    screen::WindowType,
    tag::{self, Taggable},
};
use pumpkin_util::math::position::BlockPos;
use std::{any::Any, sync::Arc};

pub fn create_beacon_handler(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    position: BlockPos,
    properties: Arc<dyn PropertyDelegate>,
) -> BeaconScreenHandler {
    let inventory: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(1));
    let mut handler = BeaconScreenHandler {
        inventory,
        position,
        properties,
        behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Beacon)),
    };
    handler.add_slot(Arc::new(PaymentSlot(NormalSlot::new(
        handler.inventory.clone(),
        0,
    ))));
    for index in 0..3 {
        handler.add_property(ScreenProperty::new(handler.properties.clone(), index));
    }
    let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
    handler.add_player_slots(&player_inventory);
    handler
}

/// Payment belongs to this menu, so simultaneous viewers cannot spend each other's item.
pub struct BeaconScreenHandler {
    pub inventory: Arc<dyn Inventory>,
    pub position: BlockPos,
    pub properties: Arc<dyn PropertyDelegate>,
    behaviour: ScreenHandlerBehaviour,
}

struct PaymentSlot(NormalSlot);
impl Slot for PaymentSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.0.get_inventory()
    }
    fn get_index(&self) -> usize {
        self.0.get_index()
    }
    fn set_id(&self, index: usize) {
        self.0.set_id(index);
    }
    fn mark_dirty(&self) {
        self.0.mark_dirty();
    }
    fn can_insert(&self, stack: &ItemStack) -> bool {
        stack
            .item
            .has_tag(&tag::Item::MINECRAFT_BEACON_PAYMENT_ITEMS)
    }
    fn get_max_item_count(&self) -> u8 {
        1
    }
}

impl ScreenHandler for BeaconScreenHandler {
    fn can_use(&self, player: &dyn InventoryPlayer) -> bool {
        player.can_use_block_type(self.position, BlockId::BEACON)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }
    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }
    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.default_on_closed(player);
        let remaining = self.inventory.remove_stack_specific(0, 1);
        if !remaining.is_empty() {
            player.drop_item(remaining, false);
        }
    }
    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let Some(slot) = usize::try_from(slot_index)
            .ok()
            .and_then(|i| self.behaviour.slots.get(i))
            .cloned()
        else {
            return ItemStack::EMPTY.clone();
        };
        let mut stack = slot.get_stack();
        if stack.is_empty() {
            return ItemStack::EMPTY.clone();
        }
        let clicked = stack.clone();
        let payment = self.behaviour.slots[0].clone();
        let (start, end, reverse) = if slot_index == 0 {
            (1, 37, true)
        } else if !payment.has_stack() && payment.can_insert(&stack) && stack.item_count == 1 {
            (0, 1, false)
        } else if (1..28).contains(&slot_index) {
            (28, 37, false)
        } else if (28..37).contains(&slot_index) {
            (1, 28, false)
        } else {
            (1, 37, false)
        };
        if !self.insert_item(&mut stack, start, end, reverse) {
            return ItemStack::EMPTY.clone();
        }
        if slot_index == 0 {
            slot.on_quick_move_crafted(stack.clone(), clicked.clone());
        }
        slot.set_stack(if stack.is_empty() {
            ItemStack::EMPTY.clone()
        } else {
            stack.clone()
        });
        if stack.item_count == clicked.item_count {
            return ItemStack::EMPTY.clone();
        }
        slot.on_take_item(player, &stack);
        clicked
    }
}
