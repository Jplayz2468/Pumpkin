use super::BlockEntity;
use pumpkin_data::data_component_impl::{CustomNameImpl, DataComponentImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct EnchantingTableBlockEntity {
    pub position: BlockPos,
    pub custom_name: Mutex<Option<TextComponent>>,
    dirty: AtomicBool,
}

impl BlockEntity for EnchantingTableBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        Self {
            position,
            custom_name: Mutex::new(nbt.get("CustomName").map(TextComponent::from_nbt)),
            dirty: AtomicBool::new(false),
        }
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
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        Some(NbtCompound::new())
    }

    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        *self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<CustomNameImpl>()
            .map(|name| name.name.clone());
        self.dirty.store(true, Ordering::Relaxed);
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

impl EnchantingTableBlockEntity {
    pub const ID: &'static str = "minecraft:enchanting_table";
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            custom_name: Mutex::new(None),
            dirty: AtomicBool::new(false),
        }
    }

    pub fn display_name(&self) -> TextComponent {
        self.custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| {
                pumpkin_macros::translate_cross!(
                    pumpkin_data::translation::java::CONTAINER_ENCHANT,
                    pumpkin_data::translation::bedrock::CONTAINER_ENCHANT
                )
            })
    }
}
