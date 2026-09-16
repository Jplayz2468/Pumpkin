use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::Slot;

use crate::inventory::Inventory;
use crate::inventory::SimpleInventory;
use pumpkin_data::data_component_impl::{MapIdImpl, MapPostProcessingImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::screen::WindowType;
use pumpkin_data::statistic::StatisticCategory;
use pumpkin_protocol::java::server::play::SlotActionType;

#[must_use]
pub fn is_map_item(stack: &ItemStack) -> bool {
    stack.get_data_component::<MapIdImpl>().is_some() || stack.item.id == Item::FILLED_MAP.id
}

#[must_use]
pub const fn is_additional_item(stack: &ItemStack) -> bool {
    stack.item.id == Item::PAPER.id
        || stack.item.id == Item::MAP.id
        || stack.item.id == Item::GLASS_PANE.id
}

pub struct CartographyTableScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    pub input_inventory: Arc<SimpleInventory>,
    pub output_inventory: Arc<SimpleInventory>,
}

impl CartographyTableScreenHandler {
    pub const MAP_SLOT: usize = 0;
    pub const ADDITIONAL_SLOT: usize = 1;
    pub const RESULT_SLOT: usize = 2;
    pub const INV_SLOT_START: usize = 3;
    pub const INV_SLOT_END: usize = 30;
    pub const USE_ROW_SLOT_START: usize = 30;
    pub const USE_ROW_SLOT_END: usize = 39;

    pub fn new(sync_id: u8, player_inventory: &Arc<PlayerInventory>) -> Self {
        let behaviour = ScreenHandlerBehaviour::new(sync_id, Some(WindowType::CartographyTable));
        let input_inventory = Arc::new(SimpleInventory::new(2));
        let output_inventory = Arc::new(SimpleInventory::new(1));

        let mut handler = Self {
            behaviour,
            input_inventory: input_inventory.clone(),
            output_inventory: output_inventory.clone(),
        };

        handler.add_slot(Arc::new(CartographyMapSlot::new(
            input_inventory.clone() as Arc<dyn Inventory>,
            0,
        )));
        handler.add_slot(Arc::new(CartographyAdditionalSlot::new(
            input_inventory.clone() as Arc<dyn Inventory>,
            1,
        )));
        handler.add_slot(Arc::new(CartographyResultSlot::new(
            output_inventory as Arc<dyn Inventory>,
            input_inventory as Arc<dyn Inventory>,
            0,
        )));

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    pub fn slots_changed(&mut self) {
        let map_stack = self.input_inventory.get_stack(0);
        let additional_stack = self.input_inventory.get_stack(1);
        let result_stack = self.output_inventory.get_stack(0);

        if result_stack.is_empty() || (!map_stack.is_empty() && !additional_stack.is_empty()) {
            if !map_stack.is_empty() && !additional_stack.is_empty() {
                self.setup_result_slot();
            }
        } else {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
        }
    }

    pub fn setup_result_slot(&mut self) {
        let mut map_stack = self.input_inventory.get_stack(0);
        let additional_stack = self.input_inventory.get_stack(1);

        if !map_stack.is_empty() && !additional_stack.is_empty() && is_map_item(&map_stack) {
            let result = if additional_stack.item.id == Item::PAPER.id {
                map_stack.item_count = 1;
                map_stack.set_data_component(MapPostProcessingImpl::SCALE);
                map_stack
            } else if additional_stack.item.id == Item::GLASS_PANE.id {
                map_stack.item_count = 1;
                map_stack.set_data_component(MapPostProcessingImpl::LOCK);
                map_stack
            } else if additional_stack.item.id == Item::MAP.id {
                map_stack.item_count = 2;
                map_stack
            } else {
                ItemStack::EMPTY.clone()
            };

            let current_result = self.output_inventory.get_stack(0);
            if !current_result.are_items_and_components_equal(&result) {
                self.output_inventory.set_stack(0, result);
            }
        } else {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
        }
    }
}

impl ScreenHandler for CartographyTableScreenHandler {
    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn on_slot_click(
        &mut self,
        slot_index: i32,
        button: i32,
        action_type: SlotActionType,
        player: &dyn InventoryPlayer,
    ) {
        self.internal_on_slot_click(slot_index, button, action_type, player);
        if (0..=2).contains(&slot_index) {
            self.slots_changed();
        }
    }

    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut clicked = ItemStack::EMPTY.clone();
        let slot = self.get_behaviour().slots.get(slot_index as usize).cloned();

        if let Some(slot) = slot {
            let mut stack = slot.get_cloned_stack();
            if !stack.is_empty() {
                clicked = stack.clone();
                if slot_index == 2 {
                    if !self.insert_item(&mut stack, 3, 39, true) {
                        return ItemStack::EMPTY.clone();
                    }
                    slot.on_quick_move_crafted(stack.clone(), clicked.clone());
                    let mut taken_stack = clicked.clone();
                    taken_stack.set_count(clicked.item_count - stack.item_count);
                    slot.on_take_item(player, &taken_stack);
                } else if slot_index != 1 && slot_index != 0 {
                    if is_map_item(&stack) {
                        if !self.insert_item(&mut stack, 0, 1, false) {
                            return ItemStack::EMPTY.clone();
                        }
                    } else if !is_additional_item(&stack) {
                        if (3..30).contains(&slot_index) {
                            if !self.insert_item(&mut stack, 30, 39, false) {
                                return ItemStack::EMPTY.clone();
                            }
                        } else if (30..39).contains(&slot_index)
                            && !self.insert_item(&mut stack, 3, 30, false)
                        {
                            return ItemStack::EMPTY.clone();
                        }
                    } else if !self.insert_item(&mut stack, 1, 2, false) {
                        return ItemStack::EMPTY.clone();
                    }
                } else if !self.insert_item(&mut stack, 3, 39, false) {
                    return ItemStack::EMPTY.clone();
                }
                self.slots_changed();

                if stack.is_empty() {
                    slot.set_stack(ItemStack::EMPTY.clone());
                } else {
                    slot.set_stack(stack.clone());
                }

                if stack.item_count == clicked.item_count {
                    return ItemStack::EMPTY.clone();
                }

                self.slots_changed();
            }
        }

        clicked
    }

    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.default_on_closed(player);
        self.drop_inventory(player, self.input_inventory.clone());
    }
}

pub struct CartographyMapSlot {
    inventory: Arc<dyn Inventory>,
    index: usize,
    id: AtomicU8,
}

impl CartographyMapSlot {
    pub fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for CartographyMapSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn can_insert(&self, stack: &ItemStack) -> bool {
        is_map_item(stack)
    }

    fn get_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn get_cloned_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn has_stack(&self) -> bool {
        !self.inventory.get_stack(self.index).is_empty()
    }

    fn set_stack(&self, stack: ItemStack) {
        self.inventory.set_stack(self.index, stack);
    }

    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {}

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

pub struct CartographyAdditionalSlot {
    inventory: Arc<dyn Inventory>,
    index: usize,
    id: AtomicU8,
}

impl CartographyAdditionalSlot {
    pub fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for CartographyAdditionalSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn can_insert(&self, stack: &ItemStack) -> bool {
        is_additional_item(stack)
    }

    fn get_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn get_cloned_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn has_stack(&self) -> bool {
        !self.inventory.get_stack(self.index).is_empty()
    }

    fn set_stack(&self, stack: ItemStack) {
        self.inventory.set_stack(self.index, stack);
    }

    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {}

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

pub struct CartographyResultSlot {
    inventory: Arc<dyn Inventory>,
    input_inventory: Arc<dyn Inventory>,
    index: usize,
    id: AtomicU8,
}

impl CartographyResultSlot {
    pub fn new(
        inventory: Arc<dyn Inventory>,
        input_inventory: Arc<dyn Inventory>,
        index: usize,
    ) -> Self {
        Self {
            inventory,
            input_inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for CartographyResultSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn can_insert(&self, _stack: &ItemStack) -> bool {
        false
    }

    fn on_take_item(&self, player: &dyn InventoryPlayer, stack: &ItemStack) {
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        self.input_inventory.remove_stack_specific(0, 1);
        self.input_inventory.remove_stack_specific(1, 1);
        self.mark_dirty();
    }

    fn get_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn get_cloned_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn has_stack(&self) -> bool {
        !self.inventory.get_stack(self.index).is_empty()
    }

    fn set_stack(&self, stack: ItemStack) {
        self.inventory.set_stack(self.index, stack);
    }

    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {}

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

/// Comparison against the real Java 26.2 `CartographyTableMenu.setupResultSlot`.
///
/// Unlike the other workstations this has no generated fixture. Java's guards
/// read `MapItemSavedData`, which this handler cannot reach: it is constructed
/// with only a sync id and the player inventory, so it has no route to the
/// server's `MapManager`. The three resulting gaps are asserted as the current
/// behaviour below and recorded in `SURVIVAL_PARITY_BACKLOG.md`, rather than
/// dressed up as parity.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use pumpkin_data::data_component_impl::{MapIdImpl, MapPostProcessing};
    use std::sync::Mutex;

    fn handler() -> CartographyTableScreenHandler {
        let player_inventory = Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ));
        CartographyTableScreenHandler::new(1, &player_inventory)
    }

    fn filled_map(id: i32) -> ItemStack {
        let mut stack = ItemStack::new(1, &Item::FILLED_MAP);
        stack.set_data_component(MapIdImpl { id });
        stack
    }

    fn result_for(map: ItemStack, additional: ItemStack) -> ItemStack {
        let mut handler = handler();
        handler.input_inventory.set_stack(0, map);
        handler.input_inventory.set_stack(1, additional);
        handler.setup_result_slot();
        handler.output_inventory.get_stack(0)
    }

    /// The three accepted combinations, which agree with Java for an unlocked
    /// map below the maximum scale.
    #[test]
    fn cartography_accepts_the_same_combinations_as_java() {
        let zoomed = result_for(filled_map(0), ItemStack::new(1, &Item::PAPER));
        assert_eq!(zoomed.item, &Item::FILLED_MAP);
        assert_eq!(zoomed.item_count, 1);
        assert_eq!(
            zoomed
                .get_data_component::<MapPostProcessingImpl>()
                .and_then(|c| c.processing),
            Some(MapPostProcessing::Scale)
        );

        let locked = result_for(filled_map(0), ItemStack::new(1, &Item::GLASS_PANE));
        assert_eq!(
            locked
                .get_data_component::<MapPostProcessingImpl>()
                .and_then(|c| c.processing),
            Some(MapPostProcessing::Lock)
        );

        let copied = result_for(filled_map(0), ItemStack::new(1, &Item::MAP));
        assert_eq!(copied.item_count, 2);

        // Anything else clears the result, as Java does.
        assert!(result_for(filled_map(0), ItemStack::new(1, &Item::STICK)).is_empty());
        assert!(result_for(ItemStack::EMPTY.clone(), ItemStack::new(1, &Item::PAPER)).is_empty());
        assert!(result_for(ItemStack::new(1, &Item::STICK), ItemStack::new(1, &Item::PAPER)).is_empty());
    }

    /// Known gap. Java requires saved map data and refuses to zoom a locked map
    /// or one already at scale 4, and refuses to lock an already-locked map.
    /// This handler consults none of that, so it offers all three regardless.
    /// The assertions below pin that difference so the day the guards land this
    /// test fails and gets updated deliberately.
    #[test]
    fn cartography_is_missing_javas_map_state_guards() {
        // An unfilled map carries no map id and so has no saved data at all;
        // Java's setupResultSlot returns without touching the result slot.
        let no_saved_data = result_for(
            ItemStack::new(1, &Item::FILLED_MAP),
            ItemStack::new(1, &Item::PAPER),
        );
        assert!(
            !no_saved_data.is_empty(),
            "known gap: an offer is made for a map with no saved data"
        );

        // Java gates both of these on `!mapData.locked` and `mapData.scale < 4`,
        // which need the server's map storage.
        let already_locked_source = filled_map(7);
        assert!(
            !result_for(already_locked_source.clone(), ItemStack::new(1, &Item::PAPER)).is_empty(),
            "known gap: zooming is offered without checking locked or scale"
        );
        assert!(
            !result_for(already_locked_source, ItemStack::new(1, &Item::GLASS_PANE)).is_empty(),
            "known gap: locking is offered without checking locked"
        );
    }
}
