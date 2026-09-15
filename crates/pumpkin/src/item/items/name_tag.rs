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
    fn use_on_entity(
        &self,
        item: &mut ItemStack,
        player: &Player,
        target: Arc<dyn EntityBase>,
    ) -> crate::block::registry::BlockActionResult {
        use crate::block::registry::BlockActionResult;
        let entity = target.get_entity();
        let Some(name) = item.get_data_component::<CustomNameImpl>() else {
            return BlockActionResult::Pass;
        };
        if !entity.entity_type.saveable || target.get_living_entity().is_none() {
            return BlockActionResult::Pass;
        }
        // NameTagItem.java:18-28: even a dead target consumes the action, but only
        // living targets are renamed and consume the stack.
        if entity.is_alive()
            && target
                .get_living_entity()
                .is_some_and(|living| living.health.load() > 0.0)
        {
            entity.set_custom_name(name.name.clone());
            if let Some(mob) = target.get_mob() {
                mob.get_mob_entity()
                    .persistence_required
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            item.decrement_unless_creative(player.gamemode.load(), 1);
        }
        BlockActionResult::Success
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
