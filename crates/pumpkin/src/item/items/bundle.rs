use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::data_component_impl::BundleContentsImpl;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag;

pub struct BundleItem;

impl ItemMetadata for BundleItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_BUNDLES.1.into()
    }
}

impl ItemBehaviour for BundleItem {
    /// BundleItem.java:139-143 `use` does not extract anything itself; it only starts
    /// the use-item animation. The actual extraction happens tick-by-tick while the
    /// player holds the button, in `onUseTick` (see `on_use_tick` below).
    fn normal_use(&self, _item: &Item, player: &Player) {
        // Pumpkin's generic dispatch does not forward which hand triggered the use
        // (see ItemBehaviour::normal_use), so mirror the pre-existing workaround of
        // checking the main hand first, then the off hand.
        let held_item = player.inventory.held_item();
        if !held_item.is_empty() && Self::ids().contains(&held_item.item.id) {
            player
                .living_entity
                .set_active_hand(pumpkin_util::Hand::Right, held_item, Self::USE_DURATION);
            return;
        }

        let off_hand_item = player.inventory.off_hand_item();
        if !off_hand_item.is_empty() && Self::ids().contains(&off_hand_item.item.id) {
            player
                .living_entity
                .set_active_hand(pumpkin_util::Hand::Left, off_hand_item, Self::USE_DURATION);
        }
    }

    /// BundleItem.java:219-228 `onUseTick`: drops one item on the first tick, then
    /// every other tick once TICKS_AFTER_FIRST_THROW (10, BundleItem.java:37) ticks
    /// have elapsed, until TICKS_MAX_THROW_DURATION (200, BundleItem.java:39) is hit.
    fn on_use_tick(&self, _stack: &ItemStack, player: &Player, remaining_use_ticks: i32) {
        let active_hand = *player
            .living_entity
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(hand) = active_hand else {
            return;
        };

        let mut held = player.inventory().get_stack_in_hand(hand);
        if held.is_empty() || !Self::ids().contains(&held.item.id) {
            player.living_entity.clear_active_hand();
            return;
        }

        let is_first_tick = remaining_use_ticks == Self::USE_DURATION;
        let is_repeat_tick =
            remaining_use_ticks < Self::USE_DURATION - 10 && remaining_use_ticks % 2 == 0;
        if !is_first_tick && !is_repeat_tick {
            return;
        }

        // BundleItem.java:145-150 `dropContent` -> BundleItem.java:192-205
        // `dropContent(bundle, player)` -> BundleItem.java:207-217
        // `removeOneItemFromBundle`.
        let mut extracted = None;
        if let Some(bundle_contents) = held.get_data_component_mut::<BundleContentsImpl>() {
            extracted = bundle_contents.try_extract();
        }
        let Some(extracted_stack) = extracted else {
            return;
        };

        let position = player.position();
        let world = player.world();
        // removeOneItemFromBundle plays BUNDLE_REMOVE_ONE (BundleItem.java:210-211);
        // the outer dropContent then plays BUNDLE_DROP_CONTENTS (BundleItem.java:147,
        // 269-273) once the removal actually happened.
        world.play_sound(Sound::ItemBundleRemoveOne, SoundCategory::Players, &position);
        world.play_sound(
            Sound::ItemBundleDropContents,
            SoundCategory::Players,
            &position,
        );

        player.inventory().set_stack_in_hand(hand, held);
        // BundleItem.java:197 `player.drop(itemStack.get(), true)`.
        player.drop_item(extracted_stack);
    }

    /// BundleItem.java:231-233 `getUseDuration` (TICKS_MAX_THROW_DURATION).
    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BundleItem {
    /// BundleItem.java:39 TICKS_MAX_THROW_DURATION / BundleItem.java:231-233
    /// getUseDuration.
    pub const USE_DURATION: i32 = 200;
}
