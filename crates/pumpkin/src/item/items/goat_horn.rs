use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::{
    data_component_impl::{InstrumentImpl, UseCooldownImpl},
    game_event::GameEvent,
    item::Item,
    item_stack::ItemStack,
    sound::SoundCategory,
};
use pumpkin_util::Hand;
use std::any::Any;

pub struct GoatHornItem;
impl ItemMetadata for GoatHornItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::GOAT_HORN.id])
    }
}
impl ItemBehaviour for GoatHornItem {
    fn use_stack(&self, stack: &ItemStack, player: &Player, hand: Hand, _yaw: f32, _pitch: f32) {
        let Some(instrument) = stack
            .get_data_component::<InstrumentImpl>()
            .and_then(InstrumentImpl::playback)
        else {
            return;
        };
        let world = player.world();
        player
            .living_entity
            .set_active_hand(hand, stack.clone(), instrument.duration_ticks);
        world.play_sound_event_fine_expect(
            player,
            &instrument.sound,
            SoundCategory::Records,
            &player.position(),
            instrument.range / 16.0,
            1.0,
        );
        world.emit_game_event_from_entity(
            GameEvent::InstrumentPlay.name(),
            player.position(),
            Some(player),
            None,
        );
        let group = stack
            .get_data_component::<UseCooldownImpl>()
            .and_then(|c| c.cooldown_group.as_deref())
            .unwrap_or(stack.item.registry_key);
        player.set_item_cooldown(group, instrument.duration_ticks);
    }
    fn normal_use(&self, _item: &Item, player: &Player) {
        self.use_stack(
            &player.inventory().held_item(),
            player,
            Hand::Right,
            0.0,
            0.0,
        );
    }
    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl GoatHornItem {
    /// The prototype duration; use_stack reads the effective component for each use.
    pub const USE_DURATION: i32 = 140;
}
