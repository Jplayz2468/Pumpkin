use crate::block::entities::BlockEntity;
use crate::world::World;
use pumpkin_data::data_component_impl::ContainerImpl;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_inventory::{Clearable, Inventory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::any::Any;
use std::array::from_fn;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, Weak};

pub struct ShelfBlockEntity {
    pub position: BlockPos,
    world: Mutex<Weak<World>>,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub align_items_to_bottom: AtomicBool,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
}

impl BlockEntity for ShelfBlockEntity {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_inventory_nbt(nbt, true);
        nbt.put_bool(
            "align_items_to_bottom",
            self.align_items_to_bottom.load(Ordering::Relaxed),
        );
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let align_items_to_bottom = nbt.get_bool("align_items_to_bottom").unwrap_or(false);
        let mut shelf = Self {
            position,
            world: Mutex::new(Weak::new()),
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            align_items_to_bottom: AtomicBool::new(align_items_to_bottom),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        };

        pumpkin_inventory::sync_read_items_from_nbt(
            nbt,
            shelf
                .items
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );

        shelf
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
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
        let mut nbt = NbtCompound::new();
        self.write_nbt(&mut nbt);
        Some(nbt)
    }

    fn set_world(&self, world: Weak<World>) {
        *self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = world;
    }

    fn apply_implicit_components(&self, stack: &ItemStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        if let Some(container) = stack.get_data_component::<ContainerImpl>() {
            for (slot, stack) in &container.items {
                if let Some(target) = items.get_mut(usize::from(*slot)) {
                    *target = stack.clone();
                }
            }
        }
        drop(items);
        self.dirty.store(true, Ordering::Relaxed);
    }

    fn on_block_replaced(self: Arc<Self>, world: &Arc<World>, position: &BlockPos) {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for item in items {
            world.scatter_stack(
                f64::from(position.0.x),
                f64::from(position.0.y),
                f64::from(position.0.z),
                item,
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ShelfBlockEntity {
    pub const INVENTORY_SIZE: usize = 3;
    pub const ID: &'static str = "minecraft:shelf";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            world: Mutex::new(Weak::new()),
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            align_items_to_bottom: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }
    pub fn swap_item_no_update(&self, slot: usize, mut stack: ItemStack) -> ItemStack {
        stack.item_count = stack.item_count.min(
            self.get_max_count_per_stack()
                .min(stack.get_max_stack_size()),
        );
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed = items[slot].split(self.get_max_count_per_stack());
        items[slot] = stack;
        removed
    }

    pub fn set_changed(&self, event: Option<&str>) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
        let world = self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade();
        if let Some(world) = world {
            let (block, state) = world.get_block_and_state_id(&self.position);
            world.update_neighbour_for_output_signal(&self.position, block);
            if let Some(event) = event {
                world.emit_game_event_from_entity(
                    event,
                    self.position.to_centered_f64(),
                    None,
                    Some(state),
                );
            }
            if let Some(entity) = world.get_block_entity(&self.position) {
                world.update_block_entity(&entity);
            }
        }
    }
}

impl Inventory for ShelfBlockEntity {
    fn size(&self) -> usize {
        Self::INVENTORY_SIZE
    }

    fn is_empty(&self) -> bool {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.iter().all(ItemStack::is_empty)
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot].clone()
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        self.items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[slot]
            .split(self.get_max_count_per_stack())
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        let result = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[slot]
            .split(amount);
        if !result.is_empty() {
            self.mark_dirty();
        }
        result
    }

    fn set_stack(&self, slot: usize, mut stack: ItemStack) {
        stack.item_count = stack.item_count.min(
            self.get_max_count_per_stack()
                .min(stack.get_max_stack_size()),
        );
        self.items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[slot] = stack;
        self.mark_dirty();
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        let current = self.get_stack(slot);
        current.is_empty()
            || current.item_count
                < self
                    .get_max_count_per_stack()
                    .min(stack.get_max_stack_size())
    }

    fn can_player_use(
        &self,
        player: &dyn pumpkin_inventory::screen_handler::InventoryPlayer,
    ) -> bool {
        player.can_use_block_inventory(self.position, self)
    }

    fn mark_dirty(&self) {
        self.set_changed(Some("block_activate"));
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for ShelfBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
    }
}
