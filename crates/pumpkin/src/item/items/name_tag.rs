use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::CustomNameImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;

pub struct NameTagItem;

impl ItemMetadata for NameTagItem {
    fn ids() -> Box<[u16]> {
        [Item::NAME_TAG.id].into()
    }
}

impl ItemBehaviour for NameTagItem {
    fn use_on_entity(&self, item: &mut ItemStack, player: &Player, entity: Arc<dyn EntityBase>) {
        let entity = entity.get_entity();
        // NameTagItem.java:18-19: `customName != null && target.getType().canSerialize()`.
        // `canSerialize()` is false for entity types built with `.noSave()` (e.g. Player,
        // EntityTypes.java:1166), which matches Pumpkin's `entity_type.saveable` flag.
        if entity.entity_type.saveable
            && let Some(name) = item.get_data_component::<CustomNameImpl>()
            // NameTagItem.java:20: the rename (and the item shrink) only happens if the target
            // is still alive; a dead target consumes the interaction without renaming anything.
            && entity.is_alive()
        {
            entity.set_custom_name(name.name.clone());
            item.decrement_unless_creative(player.gamemode.load(), 1);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
