use super::BlockEntity;
use crate::world::World;
use pumpkin_data::{
    data_component_impl::{
        ContainerImpl, ContainerLootImpl, DataComponentImpl, PotDecorationsImpl,
    },
    item_stack::ItemStack,
};
use pumpkin_inventory::{Clearable, Inventory};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::position::BlockPos;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

pub struct DecoratedPotBlockEntity {
    pub position: BlockPos,
    pub sherds: Mutex<Option<Vec<NbtTag>>>,
    pub item: Mutex<Option<ItemStack>>,
    world: Mutex<Weak<World>>,
    loot: Mutex<Option<(String, i64)>>,
    dirty: AtomicBool,
    comparator_dirty: AtomicBool,
}

impl BlockEntity for DecoratedPotBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        let entity = Self::new(position);
        let decorations = nbt
            .get_list("sherds")
            .and_then(|items| PotDecorationsImpl::read_data(&NbtTag::List(items.to_vec())))
            .unwrap_or(PotDecorationsImpl::EMPTY);
        entity.set_decorations(decorations);
        let loot = nbt
            .get_string("LootTable")
            .map(|key| (key.to_string(), nbt.get_long("LootTableSeed").unwrap_or(0)));
        if loot.is_none() {
            *entity.item.lock().unwrap() = nbt
                .get_compound("item")
                .and_then(ItemStack::read_item_stack);
        }
        *entity.loot.lock().unwrap() = loot;
        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let decorations = self.decorations();
        if decorations != PotDecorationsImpl::EMPTY {
            if let NbtTag::List(sherds) = decorations.write_data() {
                nbt.put_list("sherds", sherds);
            }
        }
        let loot = self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some((table, seed)) = loot {
            nbt.put_string("LootTable", table);
            if seed != 0 {
                nbt.put_long("LootTableSeed", seed);
            }
        } else if let Some(item) = self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            && !item.is_empty()
        {
            let mut data = NbtCompound::new();
            item.write_item_stack(&mut data);
            nbt.put_compound("item", data);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut data = NbtCompound::new();
        self.write_nbt(&mut data);
        Some(data)
    }

    fn set_world(&self, world: Weak<World>) {
        *self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = world;
    }
    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }
    fn is_comparator_dirty(&self) -> bool {
        self.comparator_dirty.load(Ordering::Relaxed)
    }
    fn clear_comparator_dirty(&self) {
        self.comparator_dirty.store(false, Ordering::Relaxed);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        self.set_decorations(
            stack
                .get_data_component::<PotDecorationsImpl>()
                .cloned()
                .unwrap_or(PotDecorationsImpl::EMPTY),
        );
        *self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<ContainerImpl>()
            .and_then(|contents| {
                contents
                    .items
                    .iter()
                    .find(|(slot, _)| *slot == 0)
                    .map(|(_, stack)| stack.clone())
            });
        *self
            .loot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<ContainerLootImpl>()
            .map(|loot| (loot.loot_table.clone(), loot.seed));
        self.mark_dirty();
    }

    fn write_dropped_stack_components(&self, stack: &mut ItemStack) {
        // The block loot table copies decorations only; contents scatter separately.
        stack.set_data_component(self.decorations());
    }

    fn on_block_replaced(self: Arc<Self>, world: &Arc<World>, pos: &BlockPos) {
        let item = self.get_item().unwrap_or_else(|| ItemStack::EMPTY.clone());
        world.scatter_stack(
            f64::from(pos.0.x),
            f64::from(pos.0.y),
            f64::from(pos.0.z),
            item,
        );
    }
}

impl DecoratedPotBlockEntity {
    pub const ID: &'static str = "minecraft:decorated_pot";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            sherds: Mutex::new(None),
            item: Mutex::new(None),
            world: Mutex::new(Weak::new()),
            loot: Mutex::new(None),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    pub fn decorations(&self) -> PotDecorationsImpl {
        self.sherds
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|sherds| PotDecorationsImpl::read_data(&NbtTag::List(sherds.clone())))
            .unwrap_or(PotDecorationsImpl::EMPTY)
    }

    fn set_decorations(&self, decorations: PotDecorationsImpl) {
        let sherds = if decorations == PotDecorationsImpl::EMPTY {
            None
        } else if let NbtTag::List(items) = decorations.write_data() {
            Some(items)
        } else {
            None
        };
        *self
            .sherds
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = sherds;
    }

    fn unpack_loot(&self) {
        let world = self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade();
        let Some(world) = world else {
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
        // Clear the table before filling; Inventory access below may re-enter this method.
        if let Some(table) = pumpkin_data::loot_table::get_loot_table(&key) {
            crate::world::loot::fill_inventory_in_world(
                &world,
                self,
                table,
                seed,
                &crate::world::loot::LootContextParameters {
                    position: Some(self.position.to_centered_f64()),
                    ..Default::default()
                },
            );
        }
        self.mark_dirty();
    }

    pub fn get_item(&self) -> Option<ItemStack> {
        self.unpack_loot();
        self.item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn take_item(&self) -> Option<ItemStack> {
        self.unpack_loot();
        let result = self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        self.mark_dirty();
        result
    }
    pub fn try_insert_item(&self, stack: &mut ItemStack, count: u8) -> bool {
        self.unpack_loot();
        if stack.is_empty() {
            return false;
        }
        let mut stored = self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let existing = stored.get_or_insert_with(|| ItemStack::EMPTY.clone());
        if !existing.is_empty() && !existing.are_items_and_components_equal(stack) {
            return false;
        }
        let room = if existing.is_empty() {
            stack.get_max_stack_size()
        } else {
            existing
                .get_max_stack_size()
                .saturating_sub(existing.item_count)
        };
        let count = count.min(stack.item_count).min(room);
        if count == 0 {
            return false;
        }
        let inserted = stack.split(count);
        if existing.is_empty() {
            *existing = inserted;
        } else {
            existing.item_count += count;
        }
        drop(stored);
        self.mark_dirty();
        true
    }

    pub fn get_comparator_output(&self) -> u8 {
        self.get_item()
            .filter(|item| !item.is_empty())
            .map_or(0, |item| {
                let limit = self
                    .get_max_count_per_stack()
                    .min(item.get_max_stack_size());
                1 + (f32::from(item.item_count) / f32::from(limit) * 14.0).floor() as u8
            })
    }
    pub fn wobble(&self, positive: bool) {
        if let Some(world) = self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade()
        {
            world.add_synced_block_event(self.position, 1, u8::from(!positive));
        }
    }
}

impl Inventory for DecoratedPotBlockEntity {
    fn size(&self) -> usize {
        1
    }
    fn is_empty(&self) -> bool {
        self.get_item().is_none_or(|item| item.is_empty())
    }
    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot == 0 {
            self.get_item().unwrap_or_else(|| ItemStack::EMPTY.clone())
        } else {
            ItemStack::EMPTY.clone()
        }
    }
    fn remove_stack(&self, slot: usize) -> ItemStack {
        self.remove_stack_specific(slot, self.get_max_count_per_stack())
    }
    fn remove_stack_specific(&self, slot: usize, count: u8) -> ItemStack {
        if slot != 0 {
            return ItemStack::EMPTY.clone();
        }
        self.unpack_loot();
        let mut item = self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = item
            .as_mut()
            .map_or_else(|| ItemStack::EMPTY.clone(), |item| item.split(count));
        if item.as_ref().is_some_and(ItemStack::is_empty) {
            *item = None;
        }
        drop(item);
        self.mark_dirty();
        result
    }
    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot == 0 {
            self.unpack_loot();
            *self
                .item
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stack);
            self.mark_dirty();
        }
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
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Clearable for DecoratedPotBlockEntity {
    fn clear(&self) {
        self.remove_stack(0);
    }
}
