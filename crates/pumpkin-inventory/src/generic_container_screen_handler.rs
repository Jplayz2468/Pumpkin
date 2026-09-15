//! Generic container screen handler.
//!
//! This module provides a generic screen handler for simple containers like:
//! - Chests (single, double, ender chest)
//! - Hoppers
//! - Dispensers/Droppers
//! - Barrels
//!
//! These containers have a simple grid layout with no special behaviors
//! (no smelting, no crafting, just item storage).

use std::{any::Any, sync::Arc};

use crate::inventory::Inventory;
use pumpkin_data::{item_stack::ItemStack, screen::WindowType};

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour},
    slot::{NormalSlot, ShulkerBoxSlot},
};

/// Creates a generic 9x3 container (single chest).
///
/// Used for single chests, ender chests, and similar containers.
pub fn create_generic_9x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        9,
        player,
    )
}

/// Creates a shulker box container (9x3).
///
/// Identical to a single chest except that its slots refuse items that cannot
/// fit inside container items, which is how vanilla stops a player nesting one
/// shulker box inside another.
pub fn create_shulker_box_9x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new_with_slots(
        WindowType::Generic9x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        9,
        player,
        true,
    )
}

/// Creates a generic 9x6 container (double chest).
///
/// Used for double chests and similar large containers.
pub fn create_generic_9x6(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x6,
        sync_id,
        player_inventory,
        inventory,
        6,
        9,
        player,
    )
}

/// Creates a generic 3x3 container.
///
/// Used for dispensers, droppers, and similar containers.
pub fn create_generic_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
        player,
    )
}

/// Creates a crafter container (9 slots, 3x3 layout).
pub fn create_crafter_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Crafter3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
        player,
    )
}

/// Creates a hopper container (5 slots).
///
/// Hoppers have a single row of 5 slots.
pub fn create_hopper(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Hopper,
        sync_id,
        player_inventory,
        inventory,
        1,
        5,
        player,
    )
}

/// Generic container screen handler.
///
/// Handles simple grid-based containers without special behaviors.
/// The container grid is followed by the player's inventory (27 slots + 9 hotbar).
pub struct GenericContainerScreenHandler {
    /// The container's inventory.
    pub inventory: Arc<dyn Inventory>,
    /// Number of rows in the container grid.
    pub rows: u8,
    /// Number of columns in the container grid.
    pub columns: u8,
    /// Whether the opener is in spectator mode.
    pub is_spectator: bool,
    /// Whether container slots refuse items that cannot fit inside container
    /// items (shulker boxes).
    shulker_box_slots: bool,
    /// Core screen handler behavior (slots, sync ID, listeners).
    behaviour: ScreenHandlerBehaviour,
}

impl GenericContainerScreenHandler {
    /// Creates a new generic container screen handler.
    ///
    /// # Arguments
    /// - `screen_type` - The window type for this container
    /// - `sync_id` - The sync ID for client-server matching
    /// - `player_inventory` - The player's inventory
    /// - `inventory` - The container's inventory
    /// - `rows` - Number of rows in the container
    /// - `columns` - Number of columns in the container
    /// - `player` - Opener, for spectator checks and attributed container events
    fn new(
        screen_type: WindowType,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
        rows: u8,
        columns: u8,
        player: &dyn InventoryPlayer,
    ) -> Self {
        Self::new_with_slots(
            screen_type,
            sync_id,
            player_inventory,
            inventory,
            rows,
            columns,
            player,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_slots(
        screen_type: WindowType,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
        rows: u8,
        columns: u8,
        player: &dyn InventoryPlayer,
        shulker_box_slots: bool,
    ) -> Self {
        let is_spectator = player.is_spectator();
        let mut handler = Self {
            inventory,
            rows,
            columns,
            is_spectator,
            shulker_box_slots,
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(screen_type)),
        };

        if !is_spectator {
            handler.inventory.on_open_by(player);
        }

        handler.add_inventory_slots();
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    /// Adds slots for the container's inventory grid.
    fn add_inventory_slots(&mut self) {
        for i in 0..self.rows {
            for j in 0..self.columns {
                let index = (j + i * self.columns) as usize;
                if self.shulker_box_slots {
                    self.add_slot(Arc::new(ShulkerBoxSlot::new(self.inventory.clone(), index)));
                } else {
                    self.add_slot(Arc::new(NormalSlot::new(self.inventory.clone(), index)));
                }
            }
        }
    }
}

impl ScreenHandler for GenericContainerScreenHandler {
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
        if !self.is_spectator && !player.is_spectator() {
            self.inventory.on_close_by(player);
        }
    }

    /// Quick move logic for generic containers.
    ///
    /// - From container: Move to player inventory (end first)
    /// - From player inventory: Move to container (start first)
    fn quick_move(&mut self, _player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut stack_left = ItemStack::EMPTY.clone();
        // Assuming bounds check passed for slot_index by caller or within quick_move spec
        let slot = self.get_behaviour().slots[slot_index as usize].clone();

        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            stack_left = slot_stack.clone();

            if slot_index < i32::from(self.rows * 9) {
                // Move from inventory to player area (end)
                if !self.insert_item(
                    &mut slot_stack,
                    (self.rows * 9).into(),
                    self.get_behaviour().slots.len() as i32,
                    true,
                ) {
                    return ItemStack::EMPTY.clone();
                }
            } else if !self.insert_item(&mut slot_stack, 0, (self.rows * 9).into(), false) {
                // Move from player area to inventory (start)
                return ItemStack::EMPTY.clone();
            }

            // Check the resulting state of the slot stack after insert_item
            if slot_stack.is_empty() {
                slot.set_stack(ItemStack::EMPTY.clone());
            } else {
                slot.set_stack(slot_stack);
            }
        }

        stack_left
    }
}
