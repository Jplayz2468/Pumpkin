use super::BlockEntity;
use pumpkin_data::data_component_impl::{CustomNameImpl, DataComponentImpl};
use pumpkin_data::data_component_impl::{NoteBlockSoundImpl, ProfileImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use std::sync::Mutex;

pub struct SkullBlockEntity {
    pub position: BlockPos,
    components: super::components::BlockEntityComponents,
    pub note_block_sound: Mutex<Option<String>>,
    pub profile: Mutex<Option<NbtTag>>,
    pub custom_name: Mutex<Option<TextComponent>>,
}

impl BlockEntity for SkullBlockEntity {
    fn component_storage(&self) -> Option<&super::components::BlockEntityComponents> {
        Some(&self.components)
    }
    fn implicit_component_types(&self) -> &'static [pumpkin_data::data_component::DataComponent] {
        use pumpkin_data::data_component::DataComponent::*;
        &[CustomName, Profile, NoteBlockSound]
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
        let note_block_sound = nbt.get_string("note_block_sound").map(ToString::to_string);
        let profile = nbt.get("profile").cloned();
        let custom_name = nbt
            .get("custom_name")
            .or_else(|| nbt.get("CustomName"))
            .map(TextComponent::from_nbt);
        let entity = Self {
            position,
            components: super::components::BlockEntityComponents::new(),
            note_block_sound: Mutex::new(note_block_sound),
            profile: Mutex::new(profile),
            custom_name: Mutex::new(custom_name),
        };
        entity.components.read_nbt(nbt);
        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.components.write_nbt(nbt);
        if let Ok(sound) = self.note_block_sound.lock()
            && let Some(sound) = sound.as_ref()
        {
            nbt.put_string("note_block_sound", sound.clone());
        }
        if let Ok(prof) = self.profile.lock()
            && let Some(prof) = prof.as_ref()
        {
            nbt.put("profile", prof.clone());
        }
        if let Ok(name) = self.custom_name.lock()
            && let Some(name) = name.as_ref()
        {
            nbt.put(
                "custom_name",
                CustomNameImpl { name: name.clone() }.write_data(),
            );
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(sound) = self.note_block_sound.try_lock()
            && let Some(ref sound) = *sound
        {
            nbt.put_string("note_block_sound", sound.clone());
        }
        if let Ok(profile) = self.profile.try_lock()
            && let Some(ref prof) = *profile
        {
            nbt.put("profile", prof.clone());
        }
        if let Ok(name) = self.custom_name.try_lock()
            && let Some(ref name) = *name
        {
            nbt.put(
                "custom_name",
                CustomNameImpl { name: name.clone() }.write_data(),
            );
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
        stack.remove_data_component(pumpkin_data::data_component::DataComponent::Profile);
        stack.remove_data_component(pumpkin_data::data_component::DataComponent::NoteBlockSound);
        if let Some(sound) = self
            .note_block_sound
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            stack.set_data_component(NoteBlockSoundImpl { sound });
        }
        if let Some(data) = self
            .profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            && let Some(profile) = ProfileImpl::read_data(data)
        {
            stack.set_data_component(profile);
        }
    }

    fn apply_implicit_components(&self, stack: &ItemStack) {
        *self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<CustomNameImpl>()
            .map(|name| name.name.clone());
        *self
            .note_block_sound
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<NoteBlockSoundImpl>()
            .map(|value| value.sound.clone());
        *self
            .profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<ProfileImpl>()
            .map(DataComponentImpl::write_data);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SkullBlockEntity {
    pub const ID: &'static str = "minecraft:skull";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            components: super::components::BlockEntityComponents::new(),
            note_block_sound: Mutex::new(None),
            profile: Mutex::new(None),
            custom_name: Mutex::new(None),
        }
    }
}
