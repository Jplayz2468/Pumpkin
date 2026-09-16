use crate::block::viewer::ViewerCountTrackerExt;
use crate::entity::player::Player;
use crate::world::World;
use pumpkin_data::data_component_impl::{
    ContainerImpl, ContainerLootImpl, CustomNameImpl, DataComponentImpl,
};
use pumpkin_inventory::{Inventory, sync_write_items_to_nbt};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::text::TextComponent;
use std::any::Any;
use std::array::from_fn;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock, Weak, atomic::AtomicBool};

use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::math::position::BlockPos;

use crate::{
    block::viewer::ViewerCountTracker, impl_block_entity_for_chest, impl_chest_helper_methods,
    impl_clearable_for_chest, impl_inventory_for_chest, impl_viewer_count_listener_for_chest,
};

pub struct TrappedChestBlockEntity {
    pub position: BlockPos,
    pub(crate) container_lock: super::container_lock::ContainerLock,
    components: super::components::BlockEntityComponents,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,

    // Viewer
    viewers: ViewerCountTracker,

    world: Mutex<Weak<World>>,
    loot: Mutex<Option<(String, i64)>>,
    custom_name: Mutex<Option<TextComponent>>,
    removed: AtomicBool,
}

impl TrappedChestBlockEntity {
    pub const INVENTORY_SIZE: usize = 27;
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:trapped_chest";
    pub const EMITS_REDSTONE: bool = true;
}

// Apply macros to generate trait implementations
impl_block_entity_for_chest!(TrappedChestBlockEntity);
impl_inventory_for_chest!(TrappedChestBlockEntity);
impl_clearable_for_chest!(TrappedChestBlockEntity);
impl_viewer_count_listener_for_chest!(TrappedChestBlockEntity);
impl_chest_helper_methods!(TrappedChestBlockEntity);
