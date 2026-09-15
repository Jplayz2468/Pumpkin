use std::any::Any;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct GoatHornItem;

impl ItemMetadata for GoatHornItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::GOAT_HORN.id])
    }
}

impl ItemBehaviour for GoatHornItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        // InstrumentItem.java:61-66 `play`: SoundSource.RECORDS (not PLAYERS), volume =
        // `instrument.range() / 16.0F` and pitch 1.0F. Every vanilla goat-horn instrument
        // (assets/datapacks/26_2/data/minecraft/instrument/*.json) shares range 256.0, so
        // 256/16 = 16.0 is correct for all of them today. `playSound(player, player, ...)`
        // excludes the acting player, who already hears their own use client-side.
        //
        // NOTE: `InstrumentImpl` (pumpkin-data's `minecraft:instrument` component) is a
        // stub that discards its NBT payload entirely (see
        // crates/pumpkin-data/src/data_component_impl/basic.rs), so this item cannot tell
        // which of the 8 instruments (Ponder/Sing/Seek/Feel/Admire/Call/Yearn/Dream) a
        // given stack carries and always plays `sound.0` (Ponder). Fixing that requires
        // teaching `InstrumentImpl` to keep the instrument id and adding a lookup table of
        // {sound_event, range, use_duration} per id (data already exists as datapack JSON
        // under `assets/datapacks/*/data/minecraft/instrument/`) -- a data-component change
        // that reaches beyond this file, so it is left as unverified follow-up work rather
        // than guessed at here.
        player.world().play_sound_raw_expect(
            player,
            Sound::ItemGoatHornSound0 as u16,
            SoundCategory::Records,
            &player.position(),
            16.0,
            1.0,
        );
        let stack = player.inventory().held_item();
        player
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack, Self::USE_DURATION);

        // InstrumentItem.java:37: `player.getCooldowns().addCooldown(itemStack,
        // floor(instrument.useDuration() * 20.0F))`. GOAT_HORN has no static `UseCooldown`
        // component (Items.java:1877-1884), so this must be applied programmatically here;
        // Pumpkin's generic cooldown-on-use handling (entity/living.rs) only fires for
        // items whose *static* item data declares `UseCooldown`, so it never covers this.
        // The default cooldown group is the item's own registry key, shared by every goat
        // horn variant since they're all `Item::GOAT_HORN`.
        player.set_item_cooldown(Item::GOAT_HORN.registry_key, Self::USE_DURATION);
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl GoatHornItem {
    // All 8 vanilla instrument definitions currently share `use_duration: 7.0` seconds
    // (see assets/datapacks/26_2/data/minecraft/instrument/*.json) => 7.0 * 20 = 140.
    pub const USE_DURATION: i32 = 140;
}
