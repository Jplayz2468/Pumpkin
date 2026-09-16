use crate::entity::player::Player;
use pumpkin_data::block_properties::BarrelLikeProperties;
use pumpkin_data::data_component_impl::{
    ContainerImpl, ContainerLootImpl, CustomNameImpl, DataComponentImpl,
};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, FacingExt, item_stack::ItemStack};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use std::any::Any;
use std::sync::{Mutex, RwLock, Weak};
use std::{
    array::from_fn,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::block::viewer::{ViewerCountListener, ViewerCountTracker, ViewerCountTrackerExt};
use crate::world::{BlockFlags, World};
use pumpkin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};

use super::BlockEntity;

pub struct BarrelBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    world: Mutex<Weak<World>>,
    loot: Mutex<Option<(String, i64)>>,
    custom_name: Mutex<Option<TextComponent>>,
    removed: AtomicBool,

    // Viewer
    viewers: ViewerCountTracker,
}

impl BlockEntity for BarrelBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        let mut entity = Self::new(position);
        let loot = nbt
            .get_string("LootTable")
            .map(|key| (key.to_owned(), nbt.get_long("LootTableSeed").unwrap_or(0)));
        if loot.is_none() {
            pumpkin_inventory::sync_read_items_from_nbt(
                nbt,
                entity
                    .items
                    .get_mut()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
        *entity
            .loot
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = loot;
        *entity
            .custom_name
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            nbt.get("CustomName").map(TextComponent::from_nbt);
        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(name) = self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            nbt.put("CustomName", CustomNameImpl { name }.write_data());
        }
        if let Some((table, seed)) = self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            nbt.put_string("LootTable", table);
            if seed != 0 {
                nbt.put_long("LootTableSeed", seed);
            }
        } else {
            let items = self
                .items
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if items.iter().any(|stack| !stack.is_empty()) {
                sync_write_items_to_nbt(items.as_slice(), nbt);
            }
        }
    }

    fn set_world(&self, world: Weak<World>) {
        *self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = world;
    }

    fn set_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    fn refresh_viewers(&self, world: &Arc<World>, source: Option<i32>) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers
            .update_viewer_count_with_source(self, world, &self.position, source);
    }

    fn tick(&self, world: &Arc<World>) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers
            .update_viewer_count::<Self>(self, world, &self.position);
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn is_comparator_dirty(&self) -> bool {
        self.comparator_dirty.load(Ordering::Relaxed)
    }

    fn clear_comparator_dirty(&self) {
        self.comparator_dirty.store(false, Ordering::Relaxed);
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        // Base BlockEntity.getUpdateTag is empty; inventories arrive through menus.
        Some(NbtCompound::new())
    }

    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        *self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<ContainerLootImpl>()
            .map(|loot| (loot.loot_table.clone(), loot.seed));
        *self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<CustomNameImpl>()
            .map(|name| name.name.clone());
        let container = stack.get_data_component::<ContainerImpl>();
        {
            let mut items = self
                .items
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            items.fill_with(|| ItemStack::EMPTY.clone());
            for (slot, stored) in container
                .into_iter()
                .flat_map(|container| container.items.iter())
            {
                if let Some(target) = items.get_mut(*slot as usize) {
                    *target = stored.clone();
                }
            }
        }
        self.mark_dirty();
    }

    fn write_dropped_stack_components(&self, stack: &mut ItemStack) {
        if let Some(name) = self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            stack.set_data_component(CustomNameImpl { name });
        }
        if let Some((loot_table, seed)) = self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            stack.set_data_component(ContainerLootImpl { loot_table, seed });
        }

        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let contents: Vec<(u8, ItemStack)> = items
            .iter()
            .enumerate()
            .filter(|(_, slot)| !slot.is_empty())
            .map(|(slot, stack)| (slot as u8, stack.clone()))
            .collect();
        if contents.is_empty() {
            return;
        }
        stack.set_data_component(ContainerImpl { items: contents });
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ViewerCountListener for BarrelBlockEntity {
    fn rechecks_viewers(&self) -> bool {
        true
    }

    fn on_container_open(&self, world: &Arc<World>, _position: &BlockPos) {
        self.play_sound(world, Sound::BlockBarrelOpen);
        self.set_open(world, true);
    }

    fn on_container_close(&self, world: &Arc<World>, _position: &BlockPos) {
        self.play_sound(world, Sound::BlockBarrelClose);
        self.set_open(world, false);
    }
}

impl BarrelBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const ID: &'static str = "minecraft:barrel";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            viewers: ViewerCountTracker::new(),
            world: Mutex::new(Weak::new()),
            loot: Mutex::new(None),
            custom_name: Mutex::new(None),
            removed: AtomicBool::new(false),
        }
    }

    pub fn display_name(&self) -> TextComponent {
        self.custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| {
                pumpkin_macros::translate_cross!(
                    pumpkin_data::translation::java::CONTAINER_BARREL,
                    pumpkin_data::translation::bedrock::CONTAINER_BARREL
                )
            })
    }

    pub fn has_loot_table(&self) -> bool {
        self.loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    pub fn unpack_loot(&self, player: Option<&Player>) {
        let Some(world) = self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade()
        else {
            return;
        };
        let loot = self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some((key, seed)) = loot else {
            return;
        };
        if let Some(table) = pumpkin_data::loot_table::get_loot_table(&key) {
            let seed = if seed == 0 { world.rand_i64() } else { seed };
            crate::world::loot::fill_inventory_with_context(
                self,
                table,
                seed,
                &crate::world::loot::LootContextParameters {
                    position: Some(self.position.to_centered_f64()),
                    this_entity: player.map(|_| &pumpkin_data::entity::EntityType::PLAYER),
                    luck: player.map_or(0.0, |player| {
                        player
                            .living_entity
                            .get_attribute_value(&pumpkin_data::attributes::Attributes::LUCK)
                            as f32
                    }),
                    ..Default::default()
                },
            );
        }
        self.mark_dirty();
    }

    fn set_open(&self, world: &Arc<World>, open: bool) {
        let state = world.get_block_state(&self.position);
        let mut properties = BarrelLikeProperties::from_state_id(state.id);

        properties.open = open;

        world.set_block_state(
            &self.position,
            properties.to_state_id(&Block::BARREL),
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn play_sound(&self, world: &Arc<World>, sound: Sound) {
        let state = world.get_block_state(&self.position);
        let properties = BarrelLikeProperties::from_state_id(state.id);
        let direction = properties.facing.to_block_direction().to_offset();
        let position = Vector3::new(
            self.position.0.x as f64 + 0.5 + direction.x as f64 / 2.0,
            self.position.0.y as f64 + 0.5 + direction.y as f64 / 2.0,
            self.position.0.z as f64 + 0.5 + direction.z as f64 / 2.0,
        );
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &position,
            0.5,
            world.rand_f32() * 0.1 + 0.9,
        );
    }
}

impl Inventory for BarrelBlockEntity {
    fn size(&self) -> usize {
        Self::INVENTORY_SIZE
    }

    fn is_empty(&self) -> bool {
        self.unpack_loot(None);
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.iter().all(ItemStack::is_empty)
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        self.unpack_loot(None);
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot].clone()
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        self.unpack_loot(None);
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
        removed
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        self.unpack_loot(None);
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let res = if !items[slot].is_empty() && amount > 0 {
            items[slot].split(amount)
        } else {
            ItemStack::EMPTY.clone()
        };
        self.mark_dirty();
        res
    }

    fn set_stack(&self, slot: usize, mut stack: ItemStack) {
        self.unpack_loot(None);
        stack.item_count = stack.item_count.min(
            stack
                .get_max_stack_size()
                .min(self.get_max_count_per_stack()),
        );
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot] = stack;
        self.mark_dirty();
    }

    fn viewer_position(&self) -> Option<BlockPos> {
        Some(self.position)
    }

    fn on_open(&self) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers.open_container();
    }

    fn on_close(&self) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers.close_container();
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for BarrelBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        self.mark_dirty();
    }
}
