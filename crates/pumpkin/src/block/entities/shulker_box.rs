use crate::entity::player::Player;
use pumpkin_data::BlockStateId;
use pumpkin_data::FacingExt;
use pumpkin_data::data_component_impl::DataComponentImpl;
use pumpkin_data::data_component_impl::{ContainerImpl, ContainerLootImpl, CustomNameImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::{boundingbox::BoundingBox, vector3::Vector3};
use pumpkin_util::text::TextComponent;
use pumpkin_world::world::BlockFlags;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, Weak};
use std::{array::from_fn, sync::Arc};

use crate::block::entities::BlockEntity;
use crate::block::viewer::ViewerCountTracker;
use crate::world::World;
use pumpkin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};

pub struct ShulkerBoxBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
    world: Mutex<Weak<World>>,
    loot: Mutex<Option<(String, i64)>>,
    custom_name: Mutex<Option<TextComponent>>,
    removed: AtomicBool,
    animation: Mutex<LidAnimation>,

    // Viewer
    pub viewers: ViewerCountTracker,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LidStatus {
    Closed,
    Opening,
    Opened,
    Closing,
}
struct LidAnimation {
    status: LidStatus,
    progress: f32,
}

impl BlockEntity for ShulkerBoxBlockEntity {
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

    fn set_world(&self, world: Weak<World>) {
        *self
            .world
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = world;
    }

    fn set_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
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

    fn refresh_viewers(&self, world: &Arc<World>, source: Option<i32>) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        let count = self.viewers.current.load(Ordering::Relaxed);
        let old = self.viewers.old.swap(count, Ordering::Relaxed);
        if count == old {
            return;
        }
        world.add_synced_block_event(self.position, Self::OPEN_ANIMATION_EVENT_TYPE, count as u8);
        if (old == 0 && count > 0) || count == 0 {
            world.emit_game_event_with_source(
                if count == 0 {
                    "container_close"
                } else {
                    "container_open"
                },
                self.position.to_centered_f64(),
                source,
            );
            Self::play_sound(world, &self.position, i32::from(count));
        }
    }

    fn tick(&self, world: &Arc<World>) {
        self.tick_lid(world);
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

    /// Restores the contents when a box carrying `minecraft:container` is
    /// placed, by a player or by a dispenser. The shulker box item declares an
    /// empty container by default, so an empty box clears the target exactly as
    /// Java's `getOrDefault(CONTAINER, ItemContainerContents.EMPTY).copyInto`
    /// does.
    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        if let Some(loot) = stack.get_data_component::<ContainerLootImpl>() {
            *self
                .loot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((loot.loot_table.clone(), loot.seed));
        }
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

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        // Base BlockEntity.getUpdateTag is empty; inventories arrive through menus.
        Some(NbtCompound::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
            world: Mutex::new(Weak::new()),
            loot: Mutex::new(None),
            custom_name: Mutex::new(None),
            removed: AtomicBool::new(false),
            animation: Mutex::new(LidAnimation {
                status: LidStatus::Closed,
                progress: 0.0,
            }),
        }
    }

    pub fn display_name(&self) -> TextComponent {
        self.custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| {
                pumpkin_macros::translate_cross!(
                    pumpkin_data::translation::java::CONTAINER_SHULKERBOX,
                    pumpkin_data::translation::bedrock::CONTAINER_SHULKERBOX
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
            crate::world::loot::fill_inventory_in_world(
                &world,
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

    pub fn is_closed(&self) -> bool {
        self.animation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            == LidStatus::Closed
    }

    pub fn trigger_event(&self, count: u8) {
        let mut animation = self
            .animation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if count == 0 {
            animation.status = LidStatus::Closing;
        }
        if count == 1 {
            animation.status = LidStatus::Opening;
        }
    }

    pub fn bounding_box(&self, state: BlockStateId) -> BoundingBox {
        let progress = self
            .animation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .progress;
        Self::progress_box(state, -1.0, 0.5 * progress)
    }

    // Shulker.getProgressDeltaAabb at block-local coordinates, size one.
    pub fn progress_box(state: BlockStateId, from: f32, to: f32) -> BoundingBox {
        let facing = pumpkin_data::block_properties::EndRodLikeProperties::from_state_id(state)
            .facing
            .to_block_direction()
            .to_offset();
        let low = f64::from(from.min(to));
        let high = f64::from(from.max(to));
        let mut min = Vector3::new(0.0, 0.0, 0.0);
        let mut max = Vector3::new(1.0, 1.0, 1.0);
        for (direction, min, max) in [
            (facing.x, &mut min.x, &mut max.x),
            (facing.y, &mut min.y, &mut max.y),
            (facing.z, &mut min.z, &mut max.z),
        ] {
            if direction > 0 {
                *min = 1.0 + low;
                *max = 1.0 + high;
            } else if direction < 0 {
                *min = -high;
                *max = -low;
            }
        }
        BoundingBox::new(min, max)
    }

    fn tick_lid(&self, world: &Arc<World>) {
        let (old, progress, opening, updates) = {
            let mut lid = self
                .animation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let old = lid.progress;
            let opening = lid.status == LidStatus::Opening;
            let mut updates = 0;
            match lid.status {
                LidStatus::Closed => lid.progress = 0.0,
                LidStatus::Opened => lid.progress = 1.0,
                LidStatus::Opening => {
                    lid.progress += 0.1;
                    if old == 0.0 {
                        updates += 1;
                    }
                    if lid.progress >= 1.0 {
                        lid.progress = 1.0;
                        lid.status = LidStatus::Opened;
                        updates += 1;
                    }
                }
                LidStatus::Closing => {
                    lid.progress -= 0.1;
                    if old == 1.0 {
                        updates += 1;
                    }
                    if lid.progress <= 0.0 {
                        lid.progress = 0.0;
                        lid.status = LidStatus::Closed;
                        updates += 1;
                    }
                }
            }
            (old, lid.progress, opening, updates)
        };
        let state = world.get_block_state_id(&self.position);
        let block = state.to_block();
        if !block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_SHULKER_BOXES) {
            return;
        }
        for _ in 0..updates {
            world
                .block_registry
                .update_neighbors(world, &self.position, BlockFlags::NOTIFY_ALL);
            world.update_neighbors_at(&self.position, block, None);
        }
        if opening {
            let bounds = Self::progress_box(state, old, progress).at_pos(self.position);
            let facing = pumpkin_data::block_properties::EndRodLikeProperties::from_state_id(state)
                .facing
                .to_block_direction()
                .to_offset();
            for entity in world.get_all_at_box(&bounds) {
                let base = entity.get_entity();
                if matches!(
                    base.entity_type.resource_name,
                    "area_effect_cloud"
                        | "marker"
                        | "interaction"
                        | "block_display"
                        | "item_display"
                        | "text_display"
                        | "ominous_item_spawner"
                ) {
                    continue;
                }
                if (entity.as_ref() as &dyn Any)
                    .downcast_ref::<crate::entity::decoration::armor_stand::ArmorStandEntity>()
                    .is_some_and(|stand| stand.is_marker())
                {
                    continue;
                }
                let movement = Vector3::new(
                    (bounds.max.x - bounds.min.x + 0.01) * f64::from(facing.x),
                    (bounds.max.y - bounds.min.y + 0.01) * f64::from(facing.y),
                    (bounds.max.z - bounds.min.z + 0.01) * f64::from(facing.z),
                );
                entity.move_entity(entity.as_ref(), movement);
            }
        }
    }

    fn play_sound(world: &World, position: &BlockPos, viewer_count: i32) {
        let sound = if viewer_count > 0 {
            Sound::BlockShulkerBoxOpen
        } else {
            Sound::BlockShulkerBoxClose
        };

        let pitch = world.rand_f32() * 0.1 + 0.9;
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &position.to_centered_f64(),
            0.5,
            pitch,
        );
    }
}

impl Inventory for ShulkerBoxBlockEntity {
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
        if !res.is_empty() {
            self.mark_dirty();
        }
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

impl Clearable for ShulkerBoxBlockEntity {
    fn clear(&self) {
        self.items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fill_with(|| ItemStack::EMPTY.clone());
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
