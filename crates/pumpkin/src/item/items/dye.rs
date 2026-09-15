use std::sync::Arc;

use crate::block::entities::{
    BlockEntity,
    sign::{DyeColor, Text},
};
use pumpkin_data::tag;

use crate::{
    block::{UseWithItemArgs, registry::BlockActionResult},
    entity::player::Player,
    item::{ItemBehaviour, ItemMetadata},
};

use crate::entity::EntityBase;
use pumpkin_data::item_stack::ItemStack;

pub struct DyeItem;

impl ItemMetadata for DyeItem {
    fn ids() -> Box<[u16]> {
        tag::Item::C_DYES.1.into()
    }
}

impl ItemBehaviour for DyeItem {
    fn use_on_entity(
        &self,
        item: &mut ItemStack,
        player: &Player,
        entity: Arc<dyn EntityBase>,
    ) -> crate::block::registry::BlockActionResult {
        // DyeItem.java:21: `target instanceof Sheep sheep && sheep.isAlive() && !sheep.isSheared()`
        if let Some(sheep) = entity
            .cast_any()
            .downcast_ref::<crate::entity::passive::sheep::SheepEntity>()
            && entity.get_entity().is_alive()
            && entity
                .get_living_entity()
                .is_some_and(|living| living.health.load() > 0.0)
            && !sheep.is_sheared()
            && let Some(color) =
                crate::entity::passive::animal::get_dye_color_from_item(item.get_item())
            && color != sheep.get_color()
        {
            let ent = entity.get_entity();
            let world = ent.world.load();
            // DyeItem.java:24: `sheep.level().playSound(player, sheep, SoundEvents.DYE_USE, ...)`
            // — the `player` argument excludes that player from hearing the broadcast sound,
            // since their own client already plays it locally.
            world.play_sound_expect(
                player,
                pumpkin_data::sound::Sound::ItemDyeUse,
                pumpkin_data::sound::SoundCategory::Players,
                &ent.pos.load(),
            );
            sheep.set_color(color);
            item.decrement_unless_creative(player.gamemode.load(), 1);
            return crate::block::registry::BlockActionResult::Success;
        }
        crate::block::registry::BlockActionResult::Pass
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl DyeItem {
    pub fn apply_to_sign(
        &self,
        args: &UseWithItemArgs<'_>,
        block_entity: &Arc<dyn BlockEntity>,
        text: &Text,
        color_name: &str,
    ) -> BlockActionResult {
        let dye_color = DyeColor::by_name(color_name).unwrap_or_default();
        if text.get_color() == dye_color {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        text.set_color(dye_color);

        args.world.update_block_entity(block_entity);
        args.world.play_block_sound(
            pumpkin_data::sound::Sound::ItemDyeUse,
            pumpkin_data::sound::SoundCategory::Blocks,
            *args.position,
        );
        BlockActionResult::Success
    }
}
