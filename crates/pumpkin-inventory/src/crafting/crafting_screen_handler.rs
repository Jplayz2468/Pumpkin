//! Crafting screen handler implementation.
//!
//! This module provides screen handlers for crafting mechanics:
//! - [`CraftingScreenHandler`] - Trait for crafting screen handlers
//! - [`CraftingTableScreenHandler`] - The 3x3 crafting table UI
//! - [`ResultSlot`] - The special result slot that shows crafted items
//!
//! # Recipe Matching
//!
//! Crafting recipes are matched against the items in the crafting grid.
//! The system supports:
//! - Shaped recipes (specific patterns)
//! - Shapeless recipes (any arrangement)
//! - Transmute recipes (upgrading items)
//! - Special recipes (like decorated pots)

use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use super::recipe_provider::{GenericRecipe, RecipeProvider};
use super::recipes::{RecipeFinderScreenHandler, RecipeInputInventory};
use crate::crafting::crafting_inventory::CraftingInventory;
use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{
    InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenHandlerListener,
    offer_or_drop_stack,
};
use crate::slot::{NormalSlot, Slot};

use crate::inventory::Inventory;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::recipes::{CraftingRecipeTypes, RECIPES_CRAFTING};
use pumpkin_data::screen::WindowType;
use pumpkin_data::statistic::StatisticCategory;
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_protocol::codec::recipe::{DynamicRecipe, OwnedCraftingRecipe};

/// The result slot in a crafting screen.
pub struct ResultSlot {
    /// The crafting inventory (grid) that provides recipe input.
    pub inventory: Arc<dyn RecipeInputInventory>,
    /// Protocol ID for this slot (assigned by screen handler).
    pub id: AtomicU8,
    /// The cached result item stack.
    pub result: Arc<Mutex<ItemStack>>,
    /// Provider for dynamic recipes.
    pub recipe_provider: Option<Arc<dyn RecipeProvider>>,
}

pub struct RecipeResult {
    pub item_id: String,
    pub count: u8,
}

/// Checks if a recipe pattern is symmetrical horizontally.
fn is_symmetrical_horizontally(pattern: &[&str]) -> bool {
    let width = pattern.first().map_or(0, |s| s.len());
    for row in pattern {
        if row.len() != width {
            return false;
        }
        for j in 0..width / 2 {
            if row.chars().nth(j) != row.chars().nth(width - j - 1) {
                return false;
            }
        }
    }
    true
}

/// Checks if a crafting recipe matches the current inventory state.
#[expect(clippy::too_many_lines)]
fn recipe_matches(
    recipe: GenericRecipe<'_>,
    input_height: usize,
    input_width: usize,
    top_x: usize,
    top_y: usize,
    count: usize,
    inventory: &dyn RecipeInputInventory,
) -> Option<RecipeResult> {
    match recipe {
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingShaped {
            key,
            pattern,
            result,
            ..
        }) => {
            #[allow(clippy::redundant_closure_for_method_calls)]
            if pattern.len() != input_height
                || pattern.first().map_or(0, |f| f.len()) != input_width
            {
                return None;
            }

            if count
                != pattern
                    .iter()
                    .map(|l| l.chars().filter(|c| *c != ' ').count())
                    .sum::<usize>()
            {
                return None;
            }

            let x_offset = top_x;
            let y_offset = top_y;

            let mut matched = true;
            'outer: for (y, row_str) in pattern.iter().enumerate() {
                for (x, current_key) in row_str.chars().enumerate() {
                    let slot = inventory
                        .get_stack((y + y_offset) * inventory.get_width() + (x + x_offset));
                    if current_key == ' ' {
                        if !slot.is_empty() {
                            matched = false;
                            break 'outer;
                        }
                        continue;
                    }

                    let Some(ingredient) = key
                        .iter()
                        .find_map(|(k, v)| (*k == current_key).then_some(v))
                    else {
                        matched = false;
                        break 'outer;
                    };

                    if !ingredient.match_item(slot.item) {
                        matched = false;
                        break 'outer;
                    }
                }
            }

            if !matched && !is_symmetrical_horizontally(pattern) {
                matched = true;
                'outer: for y in 0..pattern.len() {
                    for x in 0..pattern[y].len() {
                        let Some(current_key) = pattern[y].chars().nth(x) else {
                            matched = false;
                            break 'outer;
                        };
                        let slot = inventory.get_stack(
                            (y + y_offset) * inventory.get_height()
                                + (x_offset + input_width - 1 - x),
                        );
                        if current_key == ' ' {
                            if !slot.is_empty() {
                                matched = false;
                                break 'outer;
                            }
                            continue;
                        }
                        let Some(ingredient) = key
                            .iter()
                            .find_map(|(k, v)| (*k == current_key).then_some(v))
                        else {
                            matched = false;
                            break 'outer;
                        };
                        if !ingredient.match_item(slot.item) {
                            matched = false;
                            break 'outer;
                        }
                    }
                }
            }

            matched.then_some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingShapeless {
            ingredients,
            result,
            ..
        }) => {
            if count != ingredients.len() {
                return None;
            }
            let mut ingredient_used = vec![false; ingredients.len()];
            'next_slot: for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue 'next_slot;
                }
                for i in 0..ingredients.len() {
                    if !ingredient_used[i] && ingredients[i].match_item(slot.item) {
                        ingredient_used[i] = true;
                        continue 'next_slot;
                    }
                }
                return None;
            }
            Some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingTransmute {
            input,
            material,
            result,
            ..
        }) => {
            if count != 2 {
                return None;
            }
            'item_stack: for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue 'item_stack;
                }
                if !material.match_item(slot.item) && !input.match_item(slot.item) {
                    return None;
                }
            }
            Some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingDecoratedPot { .. }) => {
            if count != 4 || inventory.get_width() != 3 || inventory.get_height() != 3 {
                return None;
            }
            for position in (1..=7).step_by(2) {
                let slot = inventory.get_stack(position);
                if slot.is_empty()
                    || !slot
                        .item
                        .has_tag(&tag::Item::MINECRAFT_DECORATED_POT_INGREDIENTS)
                {
                    return None;
                }
            }
            Some(RecipeResult {
                item_id: "minecraft:decorated_pot".to_string(),
                count: 1,
            })
        }
        GenericRecipe::Dynamic(OwnedCraftingRecipe::Shaped {
            pattern,
            key,
            result,
            ..
        }) => {
            #[allow(clippy::redundant_closure_for_method_calls)]
            if pattern.len() != input_height
                || pattern.first().map_or(0, |f| f.len()) != input_width
            {
                return None;
            }
            if count
                != pattern
                    .iter()
                    .map(|l| l.chars().filter(|c| *c != ' ').count())
                    .sum::<usize>()
            {
                return None;
            }
            let x_offset = top_x;
            let y_offset = top_y;
            let mut matched = true;
            'outer: for (y, row_str) in pattern.iter().enumerate() {
                for (x, current_key) in row_str.chars().enumerate() {
                    let slot = inventory
                        .get_stack((y + y_offset) * inventory.get_width() + (x + x_offset));
                    if current_key == ' ' {
                        if !slot.is_empty() {
                            matched = false;
                            break 'outer;
                        }
                        continue;
                    }
                    let Some(ingredient) =
                        key.iter().find(|(k, _)| *k == current_key).map(|(_, v)| v)
                    else {
                        matched = false;
                        break 'outer;
                    };
                    if !ingredient.match_item(slot.item) {
                        matched = false;
                        break 'outer;
                    }
                }
            }
            matched.then_some(RecipeResult {
                item_id: result.item_id.clone(),
                count: result.count,
            })
        }
        GenericRecipe::Dynamic(OwnedCraftingRecipe::Shapeless {
            ingredients,
            result,
            ..
        }) => {
            if count != ingredients.len() {
                return None;
            }
            let mut ingredient_used = vec![false; ingredients.len()];
            'next_slot: for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue 'next_slot;
                }
                for i in 0..ingredients.len() {
                    if !ingredient_used[i] && ingredients[i].match_item(slot.item) {
                        ingredient_used[i] = true;
                        continue 'next_slot;
                    }
                }
                return None;
            }
            Some(RecipeResult {
                item_id: result.item_id.clone(),
                count: result.count,
            })
        }
        _ => None,
    }
}

#[must_use]
pub fn match_crafting_recipe(
    inventory: &dyn RecipeInputInventory,
    provider: Option<&dyn RecipeProvider>,
) -> Option<RecipeResult> {
    let mut count: usize = 0;
    let inventory_width = inventory.get_width();
    let mut top_x = 9;
    let mut top_y = 9;
    let mut bottom_x = 0;
    let mut bottom_y = 0;
    for i in 0..inventory.size() {
        let x = i % inventory_width;
        let y = i / inventory_width;
        let slot = inventory.get_stack(i);
        if !slot.is_empty() {
            top_x = top_x.min(x);
            top_y = top_y.min(y);
            bottom_x = bottom_x.max(x);
            bottom_y = bottom_y.max(y);
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let input_width = bottom_x + 1 - top_x;
    let input_height = bottom_y + 1 - top_y;

    for recipe in RECIPES_CRAFTING {
        if let Some(result) = recipe_matches(
            GenericRecipe::Vanilla(recipe),
            input_height,
            input_width,
            top_x,
            top_y,
            count,
            inventory,
        ) {
            return Some(result);
        }
    }

    if let Some(provider) = provider {
        let dynamic = provider.get_dynamic_recipes();
        for recipe in &dynamic {
            if let DynamicRecipe::Crafting(crafting) = recipe
                && let Some(result) = recipe_matches(
                    GenericRecipe::Dynamic(crafting),
                    input_height,
                    input_width,
                    top_x,
                    top_y,
                    count,
                    inventory,
                )
            {
                return Some(result);
            }
        }
    }

    None
}

impl ResultSlot {
    pub fn new(
        inventory: Arc<dyn RecipeInputInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) -> Self {
        Self {
            inventory,
            id: AtomicU8::new(0),
            result: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            recipe_provider: provider,
        }
    }

    fn match_recipe(&self) -> Option<RecipeResult> {
        match_crafting_recipe(&*self.inventory, self.recipe_provider.as_deref())
    }

    fn refill_output(&self) -> ItemStack {
        let result = if let Some(matched) = self.match_recipe() {
            let key = matched
                .item_id
                .strip_prefix("minecraft:")
                .unwrap_or(&matched.item_id);
            let item = pumpkin_data::item::Item::from_registry_key(key)
                .unwrap_or(&pumpkin_data::item::Item::AIR);
            ItemStack::new(matched.count, item)
        } else {
            ItemStack::EMPTY.clone()
        };
        *self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = result.clone();
        result
    }
}

impl Slot for ResultSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }
    fn get_index(&self) -> usize {
        999
    }
    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }
    fn on_quick_move_crafted(&self, _stack: ItemStack, _stack_prev: ItemStack) {
        self.refill_output();
    }
    fn on_take_item(&self, player: &dyn InventoryPlayer, stack: &ItemStack) {
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        // Mirrors ResultSlot.onTake: consume one of each ingredient, then put
        // any crafting remainder back. Without this the buckets from a cake and
        // the bottles from a honey block are destroyed.
        for i in 0..self.inventory.size() {
            let consumed = self.inventory.get_stack(i);
            if consumed.is_empty() {
                continue;
            }
            let remainder = consumed.crafting_remainder();
            self.inventory.remove_stack_specific(i, 1);

            let Some(remainder) = remainder else {
                continue;
            };
            let mut replacement = ItemStack::new(1, remainder);
            let leftover = self.inventory.get_stack(i);
            if leftover.is_empty() {
                self.inventory.set_stack(i, replacement);
            } else if leftover.are_items_and_components_equal(&replacement) {
                replacement.item_count = replacement.item_count.saturating_add(leftover.item_count);
                self.inventory.set_stack(i, replacement);
            } else {
                offer_or_drop_stack(player, replacement);
            }
        }
        self.mark_dirty();
    }
    fn can_insert(&self, _stack: &ItemStack) -> bool {
        false
    }
    fn get_stack(&self) -> ItemStack {
        self.result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    fn get_cloned_stack(&self) -> ItemStack {
        self.result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    fn has_stack(&self) -> bool {
        !self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }
    fn set_stack(&self, _stack: ItemStack) {
        self.refill_output();
    }
    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {
        self.refill_output();
    }
    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
    fn get_max_item_count(&self) -> u8 {
        let mut count = u8::MAX;
        for i in 0..self.inventory.size() {
            let slot = self.inventory.get_stack(i);
            if !slot.is_empty() {
                count = count.min(slot.item_count);
            }
        }
        count
    }
    fn take_stack(&self, _amount: u8) -> ItemStack {
        if self.has_stack() {
            self.result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        } else {
            ItemStack::EMPTY.clone()
        }
    }
}

impl ScreenHandlerListener for ResultSlot {
    fn on_slot_update(&self, screen_handler: &ScreenHandlerBehaviour, slot: u8, _stack: ItemStack) {
        if (0..=(self.inventory.get_width() * self.inventory.get_height()))
            .contains(&(slot as usize))
        {
            let result = self.refill_output();
            let next_revision = screen_handler.next_revision();
            if let Some(sync_handler) = screen_handler.sync_handler.as_ref() {
                sync_handler.update_slot(screen_handler, 0, &result, next_revision);
            }
        }
    }
}

pub trait CraftingScreenHandler<I: RecipeInputInventory>:
    RecipeFinderScreenHandler + ScreenHandler
{
    fn add_recipe_slots(
        &mut self,
        crafing_inventory: Arc<dyn RecipeInputInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) {
        let result_slot = Arc::new(ResultSlot::new(crafing_inventory.clone(), provider));
        self.add_slot(result_slot.clone());
        let width = crafing_inventory.get_width();
        let height = crafing_inventory.get_height();
        for i in 0..width {
            for j in 0..height {
                let input_slot = NormalSlot::new(crafing_inventory.clone(), j + i * width);
                self.add_slot(Arc::new(input_slot));
            }
        }
        self.add_listener(result_slot);
    }
}

pub struct CraftingTableScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    crafting_inventory: Arc<dyn RecipeInputInventory>,
}

impl CraftingTableScreenHandler {
    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) -> Self {
        let crafting_inventory: Arc<dyn RecipeInputInventory> =
            Arc::new(CraftingInventory::new(3, 3));
        let mut crafting_table_handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Crafting)),
            crafting_inventory: crafting_inventory.clone(),
        };
        crafting_table_handler.add_recipe_slots(crafting_inventory, provider);
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        crafting_table_handler.add_player_slots(&player_inventory);
        crafting_table_handler
    }
}

impl RecipeFinderScreenHandler for CraftingTableScreenHandler {}

impl ScreenHandler for CraftingTableScreenHandler {
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
        self.drop_inventory(player, self.crafting_inventory.clone());
    }
    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let slot = self.get_behaviour().slots[slot_index as usize].clone();
        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            let stack_prev = slot_stack.clone();
            if slot_index == 0 {
                if !self.insert_item(&mut slot_stack, 10, 46, true) {
                    return ItemStack::EMPTY.clone();
                }
            } else if (1..=9).contains(&slot_index) {
                if !self.insert_item(&mut slot_stack, 10, 46, false) {
                    return ItemStack::EMPTY.clone();
                }
            } else if (10..46).contains(&slot_index) {
                if !self.insert_item(&mut slot_stack, 1, 10, false) {
                    if slot_index < 37 {
                        if !self.insert_item(&mut slot_stack, 37, 46, false) {
                            return ItemStack::EMPTY.clone();
                        }
                    } else if !self.insert_item(&mut slot_stack, 10, 37, false) {
                        return ItemStack::EMPTY.clone();
                    }
                }
            } else if !self.insert_item(&mut slot_stack, 10, 46, false) {
                return ItemStack::EMPTY.clone();
            }
            let stack = slot_stack.clone();
            drop(slot_stack);
            if stack.is_empty() {
                slot.set_stack_prev(ItemStack::EMPTY.clone(), stack_prev.clone());
            } else {
                slot.mark_dirty();
            }
            if stack.item_count == stack_prev.item_count {
                return ItemStack::EMPTY.clone();
            }

            let mut taken_stack = stack_prev.clone();
            taken_stack.set_count(stack_prev.item_count - stack.item_count);
            slot.on_take_item(player, &taken_stack);

            if slot_index == 0 {
                slot.on_quick_move_crafted(stack.clone(), stack_prev.clone());
                if !stack.is_empty() {
                    player.drop_item(stack, false);
                }
            }
            return stack_prev;
        }
        ItemStack::EMPTY.clone()
    }
}

impl CraftingScreenHandler<CraftingInventory> for CraftingTableScreenHandler {}

/// Differential comparison against the real Java 26.2 crafting recipes.
/// Fixtures come from `tools/vanilla/CraftingOracle.java`.
#[cfg(test)]
mod java_parity_tests {
    use super::*;
    use crate::crafting::crafting_inventory::CraftingInventory;
    use pumpkin_data::item::Item;
    use serde_json::Value;

    /// Scope: matching and the produced item/count. `RecipeResult` carries no
    /// remainders, so the bucket/bottle returns are not covered here.
    #[test]
    fn crafting_results_match_java() {
        let cases: Vec<Value> = serde_json::from_str(include_str!("crafting_cases.json"))
            .expect("crafting fixtures");
        assert!(cases.len() > 1000, "fixture looks truncated");

        let mut mismatches: Vec<String> = Vec::new();
        for case in &cases {
            let width = case["width"].as_u64().expect("width") as usize;
            let grid = case["grid"].as_array().expect("grid");

            // Java sizes the input to the recipe; pumpkin derives the bounding
            // box itself, so the same items are placed at the grid origin.
            let inventory = CraftingInventory::new(3, 3);
            for (index, entry) in grid.iter().enumerate() {
                let Some(id) = entry.as_str() else { continue };
                let item = Item::from_registry_key(id.trim_start_matches("minecraft:"))
                    .unwrap_or_else(|| panic!("unknown item {id}"));
                let slot = (index / width) * 3 + (index % width);
                inventory.set_stack(slot, ItemStack::new(1, item));
            }

            let rust = match_crafting_recipe(&inventory, None).map(|result| {
                format!(
                    "{}x{}",
                    result.item_id.trim_start_matches("minecraft:"),
                    result.count
                )
            });
            let java = case["result"].as_object().map(|result| {
                format!(
                    "{}x{}",
                    result["item"].as_str().expect("item").trim_start_matches("minecraft:"),
                    result["count"].as_i64().expect("count")
                )
            });

            if rust != java {
                mismatches.push(format!(
                    "{}: java={:?} rust={:?}\n  grid={}",
                    case["recipe"].as_str().unwrap_or("?"),
                    java,
                    rust,
                    case["grid"]
                ));
            }
        }

        assert!(
            mismatches.is_empty(),
            "{} of {} crafting recipes differ from Java:\n{}",
            mismatches.len(),
            cases.len(),
            mismatches.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

/// Item-conservation checks for taking a crafted result.
#[cfg(test)]
mod remainder_tests {
    use super::*;
    use crate::test_support::recording_player::RecordingPlayer;
    use pumpkin_data::item::Item;

    fn grid_with(items: &[(usize, &'static Item)]) -> Arc<CraftingInventory> {
        let inventory = Arc::new(CraftingInventory::new(3, 3));
        for (slot, item) in items {
            inventory.set_stack(*slot, ItemStack::new(1, item));
        }
        inventory
    }

    fn take(inventory: Arc<CraftingInventory>, player: &RecordingPlayer) -> ItemStack {
        let slot = ResultSlot::new(inventory, None);
        // The result is computed lazily; the framework refills it when the grid
        // changes, before the player can take anything.
        let crafted = slot.refill_output();
        assert!(!crafted.is_empty(), "the grid should match a recipe");
        slot.on_take_item(player, &crafted);
        crafted
    }

    /// The bug this guards: taking the result only decremented each ingredient,
    /// so a cake silently destroyed three iron buckets.
    #[test]
    fn crafting_a_cake_returns_the_buckets() {
        let inventory = grid_with(&[
            (0, &Item::MILK_BUCKET),
            (1, &Item::MILK_BUCKET),
            (2, &Item::MILK_BUCKET),
            (3, &Item::SUGAR),
            (4, &Item::EGG),
            (5, &Item::SUGAR),
            (6, &Item::WHEAT),
            (7, &Item::WHEAT),
            (8, &Item::WHEAT),
        ]);
        let player = RecordingPlayer::new();
        let crafted = take(inventory.clone(), &player);

        assert_eq!(crafted.item, &Item::CAKE);
        // Java puts the remainder back into the slot it came from.
        for slot in 0..3 {
            let left = inventory.get_stack(slot);
            assert_eq!(left.item, &Item::BUCKET, "slot {slot} should hold a bucket");
            assert_eq!(left.item_count, 1);
        }
        // Ingredients without a remainder are simply consumed.
        for slot in 3..9 {
            assert!(inventory.get_stack(slot).is_empty(), "slot {slot} not consumed");
        }
    }

    /// Four bottles in, four bottles back.
    #[test]
    fn crafting_a_honey_block_returns_the_bottles() {
        let inventory = grid_with(&[
            (0, &Item::HONEY_BOTTLE),
            (1, &Item::HONEY_BOTTLE),
            (3, &Item::HONEY_BOTTLE),
            (4, &Item::HONEY_BOTTLE),
        ]);
        let player = RecordingPlayer::new();
        let crafted = take(inventory.clone(), &player);

        assert_eq!(crafted.item, &Item::HONEY_BLOCK);
        for slot in [0, 1, 3, 4] {
            assert_eq!(inventory.get_stack(slot).item, &Item::GLASS_BOTTLE);
        }
    }

    /// A remainder that cannot go back to its slot must reach the player rather
    /// than be dropped on the floor of the void.
    #[test]
    fn a_remainder_that_cannot_fit_goes_to_the_player() {
        let inventory = Arc::new(CraftingInventory::new(3, 3));
        // Two milk buckets stacked in one slot: one is consumed, the other stays,
        // so the bucket has nowhere to go in that slot.
        let mut stacked = ItemStack::new(2, &Item::MILK_BUCKET);
        stacked.item_count = 2;
        inventory.set_stack(0, stacked);
        inventory.set_stack(1, ItemStack::new(1, &Item::MILK_BUCKET));
        inventory.set_stack(2, ItemStack::new(1, &Item::MILK_BUCKET));
        inventory.set_stack(3, ItemStack::new(1, &Item::SUGAR));
        inventory.set_stack(4, ItemStack::new(1, &Item::EGG));
        inventory.set_stack(5, ItemStack::new(1, &Item::SUGAR));
        inventory.set_stack(6, ItemStack::new(1, &Item::WHEAT));
        inventory.set_stack(7, ItemStack::new(1, &Item::WHEAT));
        inventory.set_stack(8, ItemStack::new(1, &Item::WHEAT));

        let player = RecordingPlayer::new();
        take(inventory.clone(), &player);

        // Slot 0 still holds the surviving milk bucket, so its bucket went to
        // the player; the other two went back into their own slots.
        assert_eq!(inventory.get_stack(0).item, &Item::MILK_BUCKET);
        assert_eq!(inventory.get_stack(1).item, &Item::BUCKET);
        assert_eq!(inventory.get_stack(2).item, &Item::BUCKET);
        assert_eq!(
            player.total_of(&Item::BUCKET),
            1,
            "the displaced bucket must not be destroyed"
        );
    }
}
