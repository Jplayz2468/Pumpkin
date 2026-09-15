use crate::block::blocks::lectern::LecternBlock;
use crate::entity::{Entity, item::ItemEntity};
use crate::world::World;
use pumpkin_data::block_properties::LecternLikeProperties;
use pumpkin_data::data_component_impl::{
    BlockEntityDataImpl, WritableBookContentImpl, WrittenBookContentImpl,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::{BlockStateId, HorizontalFacingExt};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::{
    any::Any,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
};

use crate::block::entities::BlockEntity;
use pumpkin_inventory::{Clearable, Inventory};

pub struct LecternBlockEntity {
    pub position: BlockPos,
    world: Mutex<Weak<World>>,
    pub book: Arc<Mutex<ItemStack>>,
    pub page: AtomicI32,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
}

impl BlockEntity for LecternBlockEntity {
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
        let book_stack = nbt
            .get_compound("Book")
            .and_then(ItemStack::read_item_stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone());

        let page_count = Self::page_count_of(&book_stack);
        let page = nbt.get_int("Page").unwrap_or(0).max(0).min(page_count - 1);
        let book = Arc::new(Mutex::new(book_stack));

        Self {
            position,
            world: Mutex::new(Weak::new()),
            book,
            page: AtomicI32::new(page),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let book = self
            .book
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !book.is_empty() {
            let mut book_nbt = NbtCompound::default();
            book.write_item_stack(&mut book_nbt);
            nbt.put_compound("Book", book_nbt);
            nbt.put_int("Page", self.page.load(Ordering::Relaxed));
        }
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn get_automation_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        None
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

    fn set_world(&self, world: Weak<World>) {
        *self.world.lock().unwrap() = world;
    }

    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        if let Some(data) = stack.get_data_component::<BlockEntityDataImpl>() {
            if data.nbt.get_string("id").is_some_and(|id| id != Self::ID) {
                return;
            }
            let loaded = Self::from_nbt(&data.nbt, self.position);
            *self.book.lock().unwrap() = loaded.get_stack(0);
            self.page
                .store(loaded.page.load(Ordering::Relaxed), Ordering::Relaxed);
            self.mark_dirty();
        }
    }

    fn on_block_replaced_with_state(
        self: Arc<Self>,
        world: &Arc<World>,
        position: &BlockPos,
        old_state: BlockStateId,
    ) {
        let props = LecternLikeProperties::from_state_id(old_state);
        if props.has_book {
            let facing = props.facing.to_offset();
            let at = Vector3::new(
                f64::from(position.0.x) + 0.5 + f64::from(facing.x) * 0.25,
                f64::from(position.0.y) + 1.0,
                f64::from(position.0.z) + 0.5 + f64::from(facing.z) * 0.25,
            );
            world.spawn_entity(Arc::new(ItemEntity::new(
                Entity::new(world.clone(), at, &EntityType::ITEM),
                self.get_stack(0),
            )));
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LecternBlockEntity {
    pub const ID: &'static str = "minecraft:lectern";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            world: Mutex::new(Weak::new()),
            book: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            page: AtomicI32::new(0),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    pub fn set_book(&self, stack: ItemStack) {
        *self.book.lock().unwrap() = stack;
        self.page.store(0, Ordering::Relaxed);
        self.mark_dirty();
    }

    pub fn has_book(&self) -> bool {
        let book = self.book.lock().unwrap();
        book.get_data_component::<WrittenBookContentImpl>()
            .is_some()
            || book
                .get_data_component::<WritableBookContentImpl>()
                .is_some()
    }

    pub fn set_page(&self, page: i32) {
        let page = page.max(0).min(self.page_count() - 1);
        if self.page.swap(page, Ordering::Relaxed) != page {
            self.mark_dirty();
            let world = self.world.lock().unwrap().upgrade();
            if let Some(world) = world {
                LecternBlock::pulse(&world, &self.position);
            }
        }
    }

    fn on_book_removed(&self) {
        self.page.store(0, Ordering::Relaxed);
        let world = self.world.lock().unwrap().upgrade();
        if let Some(world) = world {
            LecternBlock::set_has_book(&world, &self.position, false, None);
        }
        self.mark_dirty();
    }

    /// Number of pages in a writable or written book, `0` for anything else.
    #[must_use]
    pub fn page_count_of(stack: &ItemStack) -> i32 {
        stack
            .get_data_component::<WrittenBookContentImpl>()
            .map(|content| content.pages.len())
            .or_else(|| {
                stack
                    .get_data_component::<WritableBookContentImpl>()
                    .map(|content| content.pages.len())
            })
            .map_or(0, |pages| pages as i32)
    }

    pub fn page_count(&self) -> i32 {
        Self::page_count_of(
            &self
                .book
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// Vanilla `LecternBlockEntity.getRedstoneSignal`: `floor(progress * 14) + 1`,
    /// or `0` without a book. A single-page book counts as fully read and emits 15.
    pub fn comparator_output(&self) -> u8 {
        let book = self
            .book
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let page_count = Self::page_count_of(&book);
        let progress = if page_count > 1 {
            self.page.load(Ordering::Relaxed) as f32 / (page_count - 1) as f32
        } else {
            1.0
        };
        (progress * 14.0).floor() as u8
            + u8::from(
                book.get_data_component::<WrittenBookContentImpl>()
                    .is_some()
                    || book
                        .get_data_component::<WritableBookContentImpl>()
                        .is_some(),
            )
    }
}

impl Inventory for LecternBlockEntity {
    fn can_player_use(
        &self,
        player: &dyn pumpkin_inventory::screen_handler::InventoryPlayer,
    ) -> bool {
        self.has_book() && player.can_use_block_inventory(self.position, self)
    }

    fn viewer_position(&self) -> Option<pumpkin_util::math::position::BlockPos> {
        Some(self.position)
    }

    fn size(&self) -> usize {
        1
    }

    fn is_empty(&self) -> bool {
        self.book
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        self.book
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        let removed = std::mem::replace(&mut *self.book.lock().unwrap(), ItemStack::EMPTY.clone());
        self.on_book_removed();
        removed
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        let (result, empty) = {
            let mut book = self.book.lock().unwrap();
            let result = book.split(amount);
            (result, book.is_empty())
        };
        if empty {
            self.on_book_removed();
        }
        result
    }

    // Lectern's menu container cannot accept replacement items; setBook is separate.
    fn set_stack(&self, _slot: usize, _stack: ItemStack) {}
    fn is_valid_slot_for(&self, _slot: usize, _stack: &ItemStack) -> bool {
        false
    }
    fn get_max_count_per_stack(&self) -> u8 {
        1
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for LecternBlockEntity {
    fn clear(&self) {
        self.set_book(ItemStack::EMPTY.clone());
    }
}
