use super::BlockEntity;
use pumpkin_data::data_component_impl::BannerPatternsImpl;
use pumpkin_data::data_component_impl::{CustomNameImpl, DataComponentImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use std::sync::Mutex;

pub struct BannerBlockEntity {
    pub position: BlockPos,
    components: super::components::BlockEntityComponents,
    pub custom_name: Mutex<Option<TextComponent>>,
    pub patterns: Mutex<Option<Vec<NbtTag>>>,
}

impl BlockEntity for BannerBlockEntity {
    fn component_storage(&self) -> Option<&super::components::BlockEntityComponents> {
        Some(&self.components)
    }
    fn implicit_component_types(&self) -> &'static [pumpkin_data::data_component::DataComponent] {
        use pumpkin_data::data_component::DataComponent::*;
        &[CustomName, BannerPatterns]
    }

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
        let custom_name = nbt.get("CustomName").map(TextComponent::from_nbt);
        let patterns = nbt.get_list("patterns").map(<[_]>::to_vec);
        let entity = Self {
            position,
            components: super::components::BlockEntityComponents::new(),
            custom_name: Mutex::new(custom_name),
            patterns: Mutex::new(patterns),
        };
        entity.components.read_nbt(nbt);
        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.components.write_nbt(nbt);
        if let Ok(name) = self.custom_name.lock()
            && let Some(name) = name.as_ref()
        {
            nbt.put(
                "CustomName",
                CustomNameImpl { name: name.clone() }.write_data(),
            );
        }
        if let Ok(pats) = self.patterns.lock()
            && let Some(pats) = pats.as_ref()
        {
            nbt.put_list("patterns", pats.clone());
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(name) = self.custom_name.try_lock()
            && let Some(ref name) = *name
        {
            nbt.put(
                "CustomName",
                CustomNameImpl { name: name.clone() }.write_data(),
            );
        }
        if let Ok(patterns) = self.patterns.try_lock()
            && let Some(ref pats) = *patterns
        {
            nbt.put_list("patterns", pats.clone());
        }
        Some(nbt)
    }

    fn collect_implicit_components(&self, stack: &mut ItemStack) {
        stack.remove_data_component(pumpkin_data::data_component::DataComponent::CustomName);
        if let Some(name) = self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            stack.set_data_component(CustomNameImpl { name });
        }
        let patterns = self
            .patterns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let data = NbtTag::List(patterns.clone().unwrap_or_default());
        stack.set_data_component(BannerPatternsImpl::read_data(&data).unwrap_or_default());
    }

    fn apply_implicit_components(&self, stack: &ItemStack) {
        *self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<CustomNameImpl>()
            .map(|name| name.name.clone());
        let patterns = stack
            .get_data_component::<BannerPatternsImpl>()
            .cloned()
            .unwrap_or_default()
            .write_data();
        *self
            .patterns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = match patterns {
            NbtTag::List(layers) => Some(layers),
            _ => None,
        };
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BannerBlockEntity {
    pub const ID: &'static str = "minecraft:banner";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            components: super::components::BlockEntityComponents::new(),
            custom_name: Mutex::new(None),
            patterns: Mutex::new(None),
        }
    }
}
