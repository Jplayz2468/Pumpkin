use pumpkin_data::Block;
use pumpkin_data::block_properties::ChiseledBookshelfLikeProperties;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::{
    data_component_impl::ContainerImpl,
    tag::{self, Taggable},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::{Mutex, RwLock, Weak};
use std::{
    array::from_fn,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
};
use tracing::warn;

use crate::{
    block::entities::BlockEntity,
    world::{BlockFlags, World},
};
use pumpkin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};

pub struct ChiseledBookshelfBlockEntity {
    pub position: BlockPos,
    world: Mutex<Weak<World>>,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub last_interacted_slot: AtomicI32,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
}

const LAST_INTERACTED_SLOT: &str = "last_interacted_slot";

impl BlockEntity for ChiseledBookshelfBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut bookshelf = Self {
            position,
            world: Mutex::new(Weak::new()),
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            last_interacted_slot: AtomicI32::new(-1),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        };
        pumpkin_inventory::sync_read_items_from_nbt(
            nbt,
            bookshelf
                .items
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        if let Some(slot) = nbt.get_int(LAST_INTERACTED_SLOT) {
            bookshelf
                .last_interacted_slot
                .store(slot, Ordering::Relaxed);
        }

        bookshelf
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sync_write_items_to_nbt(items.as_slice(), nbt);
        nbt.put_int(
            LAST_INTERACTED_SLOT,
            i32::from(self.last_interacted_slot.load(Ordering::Relaxed)),
        );
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
            for (slot, item) in &container.items {
                if let Some(target) = items.get_mut(usize::from(*slot)) {
                    *target = item.clone();
                }
            }
        }
        self.mark_dirty();
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

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self as Arc<dyn Inventory>)
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ChiseledBookshelfBlockEntity {
    pub const INVENTORY_SIZE: usize = 6;
    pub const ID: &'static str = "minecraft:chiseled_bookshelf";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            world: Mutex::new(Weak::new()),
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            last_interacted_slot: AtomicI32::new(-1),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    pub fn update_state(
        &self,
        mut properties: ChiseledBookshelfLikeProperties,
        world: &Arc<World>,
        slot: usize,
    ) {
        if slot >= Self::INVENTORY_SIZE {
            warn!("Invalid chiseled bookshelf slot: {slot}");
            return;
        }
        self.last_interacted_slot
            .store(slot as i32, Ordering::Relaxed);
        let occupied = {
            let items = self
                .items
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            items.each_ref().map(|stack| !stack.is_empty())
        };
        [
            properties.slot_0_occupied,
            properties.slot_1_occupied,
            properties.slot_2_occupied,
            properties.slot_3_occupied,
            properties.slot_4_occupied,
            properties.slot_5_occupied,
        ] = occupied;
        let state = properties.to_state_id(&Block::CHISELED_BOOKSHELF);
        world.set_block_state(&self.position, state, BlockFlags::NOTIFY_ALL);
        world.emit_game_event_from_entity(
            "block_change",
            self.position.to_centered_f64(),
            None,
            Some(state),
        );
        self.mark_dirty();
    }

    fn changed_slot(&self, slot: usize) {
        let world = self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade();
        if let Some(world) = world {
            let state = world.get_block_state_id(&self.position);
            if state.to_block() == &Block::CHISELED_BOOKSHELF {
                self.update_state(
                    ChiseledBookshelfLikeProperties::from_state_id(state),
                    &world,
                    slot,
                );
            }
        }
    }

    pub fn set_book(&self, slot: usize, stack: ItemStack) {
        self.set_stack(slot, stack);
    }
    pub fn remove_book(&self, slot: usize, amount: u8) -> ItemStack {
        self.remove_stack_specific(slot, amount)
    }
}

impl Inventory for ChiseledBookshelfBlockEntity {
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
        // ListBackedContainer.removeItemNoUpdate removes at most the container limit.
        self.items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)[slot]
            .split(1)
    }

    fn remove_stack_specific(&self, slot: usize, _amount: u8) -> ItemStack {
        // ChiseledBookShelfBlockEntity.removeItem ignores the requested count.
        let removed = {
            let mut items = self
                .items
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone())
        };
        if !removed.is_empty() {
            self.changed_slot(slot);
        }
        removed
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if stack.is_empty() {
            self.remove_stack_specific(slot, 1);
        } else if stack.item.has_tag(&tag::Item::MINECRAFT_BOOKSHELF_BOOKS) {
            self.items
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)[slot] = stack;
            self.changed_slot(slot);
        }
    }

    fn get_max_count_per_stack(&self) -> u8 {
        1
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        stack.item.has_tag(&tag::Item::MINECRAFT_BOOKSHELF_BOOKS) && self.get_stack(slot).is_empty()
    }

    fn can_transfer_to(&self, into: &dyn Inventory, _slot: usize, stack: &ItemStack) -> bool {
        (0..into.size()).any(|slot| {
            let target = into.get_stack(slot);
            target.is_empty()
                || (ItemStack::are_items_and_components_equal(stack, &target)
                    && u16::from(target.item_count) + u16::from(stack.item_count)
                        <= u16::from(
                            into.get_max_count_per_stack()
                                .min(target.get_max_stack_size()),
                        ))
        })
    }

    fn can_player_use(
        &self,
        player: &dyn pumpkin_inventory::screen_handler::InventoryPlayer,
    ) -> bool {
        player.can_use_block_inventory(self.position, self)
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for ChiseledBookshelfBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        self.mark_dirty();
    }
}
