use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::{NormalSlot, Slot};

use crate::inventory::Inventory;
use crate::inventory::SimpleInventory;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::recipes::{RECIPES_STONECUTTING, StonecutterRecipe};
use pumpkin_data::screen::WindowType;
use pumpkin_data::statistic::StatisticCategory;
use pumpkin_protocol::java::server::play::SlotActionType;

pub struct StonecutterScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    pub input_inventory: Arc<SimpleInventory>,
    pub output_inventory: Arc<SimpleInventory>,
    pub selected_recipe: AtomicU8,
}

impl StonecutterScreenHandler {
    pub fn new(sync_id: u8, player_inventory: &Arc<PlayerInventory>) -> Self {
        let behaviour = ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Stonecutter));
        let input_inventory = Arc::new(SimpleInventory::new(1));
        let output_inventory = Arc::new(SimpleInventory::new(1));

        let mut handler = Self {
            behaviour,
            input_inventory: input_inventory.clone(),
            output_inventory: output_inventory.clone(),
            selected_recipe: AtomicU8::new(u8::MAX),
        };

        handler.add_slot(Arc::new(NormalSlot::new(
            input_inventory.clone() as Arc<dyn Inventory>,
            0,
        )));
        handler.add_slot(Arc::new(StonecutterOutputSlot::new(
            output_inventory as Arc<dyn Inventory>,
            input_inventory as Arc<dyn Inventory>,
            0,
        )));

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();

        handler.add_player_slots(&player_inventory);

        handler
    }

    fn update_output(&self) {
        let input_lock = self.input_inventory.get_stack(0);

        if input_lock.is_empty() {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
            self.selected_recipe.store(u8::MAX, Ordering::Relaxed);
            return;
        }

        let available_recipes = Self::get_available_recipes(&input_lock);
        let recipe_index = self.selected_recipe.load(Ordering::Relaxed);

        if recipe_index != u8::MAX && (recipe_index as usize) < available_recipes.len() {
            let recipe = available_recipes[recipe_index as usize];
            let item = Item::from_registry_key(recipe.result.id).unwrap_or(&Item::AIR);
            let result = ItemStack::new(recipe.result.count, item);
            self.output_inventory.set_stack(0, result);
        } else {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
        }
    }

    fn get_available_recipes(input: &ItemStack) -> Vec<&'static StonecutterRecipe> {
        let item = input.item;
        RECIPES_STONECUTTING
            .iter()
            .filter(|r| r.ingredient.match_item(item))
            .collect()
    }
}

impl ScreenHandler for StonecutterScreenHandler {
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

    /// Mirrors StonecutterMenu.removed: the offered result is discarded and the
    /// input is handed back. Without this the default close only drops the
    /// cursor stack and whatever sits in the input slot is destroyed.
    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.default_on_closed(player);
        self.output_inventory
            .set_stack(0, ItemStack::EMPTY.clone());
        self.drop_inventory(player, self.input_inventory.clone());
    }

    fn on_slot_click(
        &mut self,
        slot_index: i32,
        button: i32,
        action_type: SlotActionType,
        player: &dyn InventoryPlayer,
    ) {
        self.internal_on_slot_click(slot_index, button, action_type, player);
        if slot_index == 0 {
            self.update_output();
        }
    }

    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut stack = ItemStack::EMPTY.clone();
        let slot = self.get_behaviour().slots.get(slot_index as usize).cloned();

        if let Some(slot) = slot {
            let mut slot_stack = slot.get_cloned_stack();
            if !slot_stack.is_empty() {
                stack = slot_stack.clone();
                if slot_index < 2 {
                    // From Stonecutter to Player
                    if !self.insert_item(&mut slot_stack, 2, 38, true) {
                        return ItemStack::EMPTY.clone();
                    }
                    slot.on_quick_move_crafted(slot_stack.clone(), stack.clone());
                } else {
                    // From Player to Stonecutter
                    // Try input slot (0)
                    if !self.insert_item(&mut slot_stack, 0, 1, false) {
                        return ItemStack::EMPTY.clone();
                    }
                }

                if slot_stack.is_empty() {
                    slot.set_stack(ItemStack::EMPTY.clone());
                } else {
                    slot.set_stack(slot_stack.clone());
                }

                if slot_index == 1 {
                    let mut taken_stack = stack.clone();
                    taken_stack.set_count(stack.item_count - slot_stack.item_count);
                    slot.on_take_item(player, &taken_stack);
                }
            }
        }
        stack
    }
}

pub struct StonecutterOutputSlot {
    pub inventory: Arc<dyn Inventory>,
    pub input_inventory: Arc<dyn Inventory>,
    pub index: usize,
    pub id: AtomicU8,
}

impl StonecutterOutputSlot {
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

impl Slot for StonecutterOutputSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn on_take_item(&self, player: &dyn InventoryPlayer, stack: &ItemStack) {
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        self.input_inventory.remove_stack_specific(0, 1);
        self.mark_dirty();
    }

    fn can_insert(&self, _stack: &ItemStack) -> bool {
        false
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

    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {
        // Do nothing
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

/// Differential comparison against the real Java 26.2 stonecutting recipes.
/// Fixtures come from `tools/vanilla/StonecutterOracle.java`.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use serde_json::Value;

    /// The offered list is compared in order: the client selects a recipe by
    /// index, so a reordering silently hands the player a different item.
    #[test]
    fn stonecutter_offers_match_java() {
        let cases: Vec<Value> = serde_json::from_str(include_str!("stonecutter_cases.json"))
            .expect("stonecutter fixtures");
        assert!(cases.len() > 50, "fixture looks truncated");

        let mut mismatches: Vec<String> = Vec::new();
        for case in &cases {
            let id = case["input"].as_str().expect("input id");
            let item = Item::from_registry_key(id.trim_start_matches("minecraft:"))
                .unwrap_or_else(|| panic!("unknown item {id}"));
            let input = ItemStack::new(1, item);

            let rust: Vec<String> = StonecutterScreenHandler::get_available_recipes(&input)
                .iter()
                .map(|recipe| {
                    format!(
                        "{}x{}",
                        recipe.result.id.trim_start_matches("minecraft:"),
                        recipe.result.count
                    )
                })
                .collect();
            let java: Vec<String> = case["offered"]
                .as_array()
                .expect("offered")
                .iter()
                .map(|entry| {
                    format!(
                        "{}x{}",
                        entry["item"].as_str().expect("item").trim_start_matches("minecraft:"),
                        entry["count"].as_i64().expect("count")
                    )
                })
                .collect();

            if rust != java {
                mismatches.push(format!(
                    "{id}:\n  java ({}) = {}\n  rust ({}) = {}",
                    java.len(),
                    java.join(", "),
                    rust.len(),
                    rust.join(", ")
                ));
            }
        }

        assert!(
            mismatches.is_empty(),
            "{} of {} stonecutter inputs differ from Java:\n{}",
            mismatches.len(),
            cases.len(),
            mismatches.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

/// Item-conservation checks for the close path.
#[cfg(test)]
mod close_tests {
    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use crate::test_support::recording_player::RecordingPlayer;
    use std::sync::Mutex;

    /// The bug this guards: the stonecutter had no `on_closed`, so the default
    /// dropped only the cursor stack and the input was destroyed on close.
    #[test]
    fn closing_returns_the_input_and_clears_the_offer() {
        let player = RecordingPlayer::new();
        let inventory = Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ));
        let mut handler = StonecutterScreenHandler::new(1, &inventory);

        handler
            .input_inventory
            .set_stack(0, ItemStack::new(37, &Item::STONE));
        handler.selected_recipe.store(0, Ordering::Relaxed);
        handler.update_output();
        assert!(!handler.output_inventory.get_stack(0).is_empty());

        handler.on_closed(&player);

        assert!(
            handler.input_inventory.get_stack(0).is_empty(),
            "the input must not be left in the block"
        );
        assert!(handler.output_inventory.get_stack(0).is_empty());
        assert_eq!(
            player.total_of(&Item::STONE),
            37,
            "every input item must come back to the player"
        );
    }
}
