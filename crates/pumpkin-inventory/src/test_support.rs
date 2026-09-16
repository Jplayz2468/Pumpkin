//! Shared replay helpers for the workstation differential tests.
//!
//! The Java probes in `tools/vanilla/` record each case's inputs and outputs in
//! one stack encoding; these helpers rebuild a stack from that encoding and
//! render a stack back into it, so a mismatch prints both sides identically.

pub mod java_parity {
    use pumpkin_data::data_component_impl::{EnchantmentsImpl, StoredEnchantmentsImpl, TrimImpl};
    use pumpkin_data::enchantment::Enchantment;
    use pumpkin_data::item::Item;
    use pumpkin_data::item_stack::ItemStack;
    use serde_json::Value;
    use std::borrow::Cow;

    fn enchantment(id: &str) -> &'static Enchantment {
        Enchantment::from_name(id.trim_start_matches("minecraft:"))
            .unwrap_or_else(|| panic!("unknown enchantment {id}"))
    }

    fn enchantment_list(value: &Value) -> Vec<(&'static Enchantment, i32)> {
        value
            .as_object()
            .expect("enchantment map")
            .iter()
            .map(|(id, level)| (enchantment(id), level.as_i64().expect("level") as i32))
            .collect()
    }

    /// Rebuilds the stack the Java probe placed in a slot.
    #[must_use]
    pub fn build(spec: &Value) -> ItemStack {
        if spec.is_null() {
            return ItemStack::EMPTY.clone();
        }
        let id = spec["item"].as_str().expect("item id");
        let item = Item::from_registry_key(id.trim_start_matches("minecraft:"))
            .unwrap_or_else(|| panic!("unknown item {id}"));
        let mut stack = ItemStack::new(spec["count"].as_i64().expect("count") as u8, item);

        let damage = spec["damage"].as_i64().expect("damage") as i32;
        if damage != 0 {
            stack.set_damage(damage);
        }
        let repair_cost = spec["repair_cost"].as_i64().expect("repair cost") as i32;
        if repair_cost != 0 {
            stack.set_repair_cost(repair_cost);
        }
        let enchantments = enchantment_list(&spec["enchantments"]);
        if !enchantments.is_empty() {
            stack.set_data_component(EnchantmentsImpl {
                enchantment: Cow::Owned(enchantments),
            });
        }
        let stored = enchantment_list(&spec["stored_enchantments"]);
        if !stored.is_empty() {
            stack.set_data_component(StoredEnchantmentsImpl {
                enchantment: Cow::Owned(stored),
            });
        }
        stack
    }

    fn join(mut names: Vec<String>) -> String {
        names.sort();
        names.join(",")
    }

    /// Renders a produced stack in the probe's encoding.
    #[must_use]
    pub fn describe(stack: &ItemStack) -> String {
        if stack.is_empty() {
            return "<empty>".to_string();
        }
        let encode = |pairs: Vec<(&'static Enchantment, i32)>| {
            join(pairs
                .into_iter()
                .map(|(e, level)| format!("{}={level}", e.name.trim_start_matches("minecraft:")))
                .collect())
        };
        let enchantments = stack
            .get_data_component::<EnchantmentsImpl>()
            .map(|c| encode(c.enchantment.iter().map(|(e, l)| (*e, *l)).collect()))
            .unwrap_or_default();
        let stored = stack
            .get_data_component::<StoredEnchantmentsImpl>()
            .map(|c| encode(c.enchantment.iter().map(|(e, l)| (*e, *l)).collect()))
            .unwrap_or_default();
        let trim = stack.get_data_component::<TrimImpl>().map_or_else(
            || "none".to_string(),
            |t| {
                let text = |tag: &pumpkin_nbt::tag::NbtTag| {
                    tag.extract_string().unwrap_or_default().to_string()
                };
                format!("{}/{}", text(&t.material), text(&t.pattern))
            },
        );
        format!(
            "{} x{} damage={} repair_cost={} enchants=[{}] stored=[{}] trim={trim}",
            stack.item.registry_key,
            stack.item_count,
            stack.get_damage(),
            stack.get_repair_cost(),
            enchantments,
            stored
        )
    }

    /// Renders the stack Java recorded, in the same encoding as [`describe`].
    #[must_use]
    pub fn expected(result: &Value) -> String {
        if result.is_null() {
            return "<empty>".to_string();
        }
        let encode = |value: &Value| {
            join(value
                .as_object()
                .expect("map")
                .iter()
                .map(|(id, level)| {
                    format!(
                        "{}={}",
                        id.trim_start_matches("minecraft:"),
                        level.as_i64().expect("level")
                    )
                })
                .collect())
        };
        let trim = match result.get("trim") {
            Some(value) if !value.is_null() => format!(
                "{}/{}",
                value["material"].as_str().unwrap_or_default(),
                value["pattern"].as_str().unwrap_or_default()
            ),
            _ => "none".to_string(),
        };
        format!(
            "{} x{} damage={} repair_cost={} enchants=[{}] stored=[{}] trim={trim}",
            result["item"]
                .as_str()
                .expect("item")
                .trim_start_matches("minecraft:"),
            result["count"].as_i64().expect("count"),
            result["damage"].as_i64().expect("damage"),
            result["repair_cost"].as_i64().expect("repair cost"),
            encode(&result["enchantments"]),
            encode(&result["stored_enchantments"])
        )
    }
}

/// A player that accepts offered items and records dropped ones, so a test can
/// assert that a menu conserved every item it was given.
pub mod recording_player {
    use crate::entity_equipment::EntityEquipment;
    use crate::player::player_inventory::PlayerInventory;
    use crate::screen_handler::InventoryPlayer;
    use pumpkin_data::item::Item;
    use pumpkin_data::item_stack::ItemStack;
    use pumpkin_data::screen::WindowType;
    use pumpkin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Arc, Mutex};

    pub struct RecordingPlayer {
        pub inventory: Arc<PlayerInventory>,
        pub dropped: Mutex<Vec<ItemStack>>,
        pub experience: AtomicI32,
        pub creative: bool,
    }

    impl RecordingPlayer {
        #[must_use]
        pub fn new() -> Self {
            Self {
                inventory: Arc::new(PlayerInventory::new(
                    Arc::new(Mutex::new(EntityEquipment::new())),
                    Arc::new(rustc_hash::FxHashMap::default()),
                )),
                dropped: Mutex::new(Vec::new()),
                experience: AtomicI32::new(0),
                creative: false,
            }
        }

        /// Every copy of `item` the player ended up holding or dropping.
        #[must_use]
        pub fn total_of(&self, item: &'static Item) -> u32 {
            let mut total = 0u32;
            for index in 0..PlayerInventory::MAIN_SIZE {
                let stack = crate::inventory::Inventory::get_stack(&*self.inventory, index);
                if stack.item.id == item.id {
                    total += u32::from(stack.item_count);
                }
            }
            for stack in self
                .dropped
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
            {
                if stack.item.id == item.id {
                    total += u32::from(stack.item_count);
                }
            }
            total
        }
    }

    impl Default for RecordingPlayer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl InventoryPlayer for RecordingPlayer {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn drop_item(&self, item: ItemStack, _retain_ownership: bool) {
            self.dropped
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(item);
        }

        fn is_creative(&self) -> bool {
            self.creative
        }

        fn has_infinite_materials(&self) -> bool {
            self.creative
        }

        fn experience_level(&self) -> i32 {
            self.experience.load(Ordering::Relaxed)
        }

        fn add_experience_levels(&self, levels: i32) {
            self.experience.fetch_add(levels, Ordering::Relaxed);
        }

        fn enchantment_seed(&self) -> i32 {
            0
        }

        fn set_enchantment_seed(&self, _seed: i32) {}

        fn get_inventory(&self) -> Arc<PlayerInventory> {
            self.inventory.clone()
        }

        fn enqueue_inventory_packet(
            &self,
            _packet: &CSetContainerContent,
            _window_type: Option<WindowType>,
        ) {
        }

        fn enqueue_slot_packet(
            &self,
            _packet: &CSetContainerSlot,
            _window_type: Option<WindowType>,
            _total_slots: usize,
        ) {
        }

        fn enqueue_cursor_packet(&self, _packet: &CSetCursorItem) {}

        fn enqueue_property_packet(&self, _packet: &CSetContainerProperty) {}

        fn enqueue_slot_set_packet(&self, _packet: &CSetPlayerInventory) {}

        fn enqueue_set_held_item_packet(&self, _packet: &CSetSelectedSlot) {}

        fn enqueue_equipment_change(
            &self,
            _slot: &pumpkin_data::data_component_impl::EquipmentSlot,
            _stack: &ItemStack,
        ) {
        }

        fn award_experience(&self, _amount: i32) {}

        fn increment_stat(
            &self,
            _category: pumpkin_data::statistic::StatisticCategory,
            _stat_id: i32,
            _amount: i32,
        ) {
        }

        fn play_block_sound(&self, _sound: pumpkin_data::sound::Sound, _pitch: f32) {}
    }
}
