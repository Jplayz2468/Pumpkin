use pumpkin_data::data_component_impl::ContainerImpl;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{array::from_fn, sync::Arc};

use crate::block::entities::BlockEntity;
use crate::block::viewer::{ViewerCountListener, ViewerCountTracker, ViewerCountTrackerExt};
use crate::world::World;
use pumpkin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};

pub struct ShulkerBoxBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    // Viewer
    pub viewers: ViewerCountTracker,
}

impl BlockEntity for ShulkerBoxBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut shulker_box = Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            viewers: ViewerCountTracker::new(),
        };

        pumpkin_inventory::sync_read_items_from_nbt(
            nbt,
            shulker_box
                .items
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );

        shulker_box
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_inventory_nbt(nbt, true);
    }

    fn refresh_viewers(&self, world: &Arc<World>, source: Option<i32>) {
        self.viewers
            .update_viewer_count_with_source(self, world, &self.position, source);
    }

    fn tick(&self, world: &Arc<World>) {
        self.viewers
            .update_viewer_count::<Self>(self, world, &self.position);
    }

    fn on_block_replaced(self: Arc<Self>, _world: &Arc<World>, _position: &BlockPos) {
        // Shulker boxes retain items when broken
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

    /// Vanilla's `blocks/shulker_box` loot table copies `minecraft:container`
    /// off the block entity, which is what carries the contents through the
    /// break -> item -> place cycle. An all-empty box adds no patch: the item
    /// already declares an empty container by default, so the drop reads empty
    /// just as it does on the reference server.
    fn write_dropped_stack_components(&self, stack: &mut ItemStack) {
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

    /// Restores the contents when a box carrying `minecraft:container` is
    /// placed, by a player or by a dispenser. The shulker box item declares an
    /// empty container by default, so an empty box clears the target exactly as
    /// Java's `getOrDefault(CONTAINER, ItemContainerContents.EMPTY).copyInto`
    /// does.
    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        let Some(container) = stack.get_data_component::<ContainerImpl>() else {
            return;
        };
        {
            let mut items = self
                .items
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            items.fill_with(|| ItemStack::EMPTY.clone());
            for (slot, stored) in &container.items {
                if let Some(target) = items.get_mut(*slot as usize) {
                    *target = stored.clone();
                }
            }
        }
        self.mark_dirty();
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(items) = self.items.try_read() {
            sync_write_items_to_nbt(items.as_slice(), &mut nbt);
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ViewerCountListener for ShulkerBoxBlockEntity {
    fn on_container_open(&self, world: &Arc<World>, position: &BlockPos) {
        Self::play_sound(world, position, 1);
    }

    fn on_container_close(&self, world: &Arc<World>, position: &BlockPos) {
        Self::play_sound(world, position, 0);
    }

    fn on_viewer_count_update(&self, world: &Arc<World>, position: &BlockPos, _old: u16, new: u16) {
        world.add_synced_block_event(*position, Self::OPEN_ANIMATION_EVENT_TYPE, new as u8);
    }
}

impl ShulkerBoxBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const OPEN_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:shulker_box";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            viewers: ViewerCountTracker::new(),
        }
    }

    pub fn update_viewers(&self, world: &Arc<World>) {
        let viewer_count = self.viewers.current.load(Ordering::Relaxed);
        Self::play_sound(world, &self.position, i32::from(viewer_count));
    }

    fn play_sound(world: &World, position: &BlockPos, viewer_count: i32) {
        let sound = if viewer_count > 0 {
            Sound::BlockShulkerBoxOpen
        } else {
            Sound::BlockShulkerBoxClose
        };

        world.play_sound(sound, SoundCategory::Blocks, &position.to_f64());
    }
}

impl Inventory for ShulkerBoxBlockEntity {
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
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
        self.mark_dirty();
        removed
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
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

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot] = stack;
        self.mark_dirty();
    }

    /// Java `ShulkerBoxBlockEntity.canPlaceItemThroughFace`: a shulker box
    /// never accepts another shulker box, so hoppers and droppers cannot nest
    /// them.
    fn is_valid_slot_for(&self, _slot: usize, stack: &ItemStack) -> bool {
        !pumpkin_data::Block::from_item_id(stack.item.id)
            .is_some_and(|block| block.is_tagged_with("minecraft:shulker_boxes") == Some(true))
    }

    fn viewer_position(&self) -> Option<BlockPos> {
        Some(self.position)
    }

    fn on_open(&self) {
        self.viewers.open_container();
    }

    fn on_close(&self) {
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

impl Clearable for ShulkerBoxBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        self.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;
    use pumpkin_data::item::Item;
    use pumpkin_util::math::vector3::Vector3;

    fn box_at() -> ShulkerBoxBlockEntity {
        ShulkerBoxBlockEntity::new(BlockPos(Vector3::new(0, 0, 0)))
    }

    fn contents(entity: &ShulkerBoxBlockEntity) -> Vec<(usize, u16, u8)> {
        let items = entity
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items
            .iter()
            .enumerate()
            .filter(|(_, stack)| !stack.is_empty())
            .map(|(slot, stack)| (slot, stack.item.id, stack.item_count))
            .collect()
    }

    #[test]
    fn shulker_drop_carries_every_occupied_slot_and_nothing_else() {
        let entity = box_at();
        entity.set_stack(0, ItemStack::new(7, &Item::DIAMOND));
        entity.set_stack(26, ItemStack::new(13, &Item::OAK_LOG));

        let mut dropped = ItemStack::new(1, &Item::RED_SHULKER_BOX);
        entity.write_dropped_stack_components(&mut dropped);

        let container = dropped
            .get_data_component::<ContainerImpl>()
            .expect("a filled box contributes minecraft:container");
        let mut carried: Vec<(u8, u16, u8)> = container
            .items
            .iter()
            .map(|(slot, stack)| (*slot, stack.item.id, stack.item_count))
            .collect();
        carried.sort_unstable();
        assert_eq!(
            carried,
            vec![(0, Item::DIAMOND.id, 7), (26, Item::OAK_LOG.id, 13)]
        );
    }

    #[test]
    fn shulker_drop_of_an_empty_box_carries_no_stored_items() {
        // The shulker box item already declares an empty minecraft:container by
        // default, so an empty box adds no patch and the drop reads as empty -
        // which is what the reference server writes for an empty box.
        let mut dropped = ItemStack::new(1, &Item::RED_SHULKER_BOX);
        box_at().write_dropped_stack_components(&mut dropped);
        assert!(
            dropped
                .get_data_component::<ContainerImpl>()
                .is_none_or(|container| container.items.is_empty())
        );

        // Mutation control: the same assertion must fail once a slot is used,
        // so a hook that never wrote anything could not pass this pair.
        let filled = box_at();
        filled.set_stack(5, ItemStack::new(1, &Item::DIAMOND));
        let mut second = ItemStack::new(1, &Item::RED_SHULKER_BOX);
        filled.write_dropped_stack_components(&mut second);
        assert_eq!(
            second
                .get_data_component::<ContainerImpl>()
                .map(|container| container.items.len()),
            Some(1)
        );
    }

    #[test]
    fn shulker_placement_restores_exactly_the_carried_slots() {
        let source = box_at();
        source.set_stack(0, ItemStack::new(7, &Item::DIAMOND));
        source.set_stack(26, ItemStack::new(13, &Item::OAK_LOG));
        let mut dropped = ItemStack::new(1, &Item::RED_SHULKER_BOX);
        source.write_dropped_stack_components(&mut dropped);

        let placed = box_at();
        // A slot the carried stack does not mention must not survive.
        placed.set_stack(3, ItemStack::new(1, &Item::STONE));
        placed.apply_components_from_item_stack(&dropped);

        assert_eq!(
            contents(&placed),
            vec![(0, Item::DIAMOND.id, 7), (26, Item::OAK_LOG.id, 13)]
        );
        assert!(placed.is_dirty());
    }

    #[test]
    fn shulker_placement_from_a_box_with_no_stored_items_clears_the_target() {
        // Java: applyImplicitComponents does
        // getOrDefault(CONTAINER, ItemContainerContents.EMPTY).copyInto(items),
        // so a box with nothing stored empties what it is applied to. The
        // shulker box item's default empty container gives the same result here.
        let entity = box_at();
        entity.set_stack(1, ItemStack::new(4, &Item::EMERALD));
        entity.apply_components_from_item_stack(&ItemStack::new(1, &Item::RED_SHULKER_BOX));
        assert_eq!(contents(&entity), Vec::new());
    }

    #[test]
    fn shulker_rejects_nested_boxes_from_every_side_but_accepts_ordinary_items() {
        let entity = box_at();
        for block in [
            &Block::SHULKER_BOX,
            &Block::WHITE_SHULKER_BOX,
            &Block::CYAN_SHULKER_BOX,
            &Block::BLACK_SHULKER_BOX,
        ] {
            let item = Item::from_registry_key(block.name)
                .expect("every shulker box block has a matching item");
            assert!(
                !entity.is_valid_slot_for(0, &ItemStack::new(1, item)),
                "{}",
                block.name
            );
        }
        for item in [&Item::DIAMOND, &Item::CHEST, &Item::BARREL, &Item::STONE] {
            assert!(
                entity.is_valid_slot_for(0, &ItemStack::new(1, item)),
                "{}",
                item.registry_key
            );
        }
    }
}
