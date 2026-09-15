#[allow(clippy::wildcard_imports)]
use super::*;
use crate::block::entities::beacon::BeaconBlockEntity;
use pumpkin_inventory::beacon_screen_handler::BeaconScreenHandler;
use pumpkin_inventory::screen_handler::ScreenHandler;
use pumpkin_protocol::java::server::play::SSetBeacon;

impl JavaClient {
    pub fn handle_set_beacon(&self, player: &Arc<Player>, packet: &SSetBeacon) {
        let is_valid = {
            let screen_handler_lock = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let mut screen_handler = screen_handler_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let Some(beacon_handler) = screen_handler
                .as_any_mut()
                .downcast_mut::<BeaconScreenHandler>()
            else {
                debug!(
                    "Player {} interacted with invalid menu (expected beacon)",
                    player.gameprofile.name
                );
                return;
            };

            if !beacon_handler.can_use(player.as_ref()) {
                return;
            }
            let levels = beacon_handler.properties.get_property(0);
            let primary_id = packet.primary_effect.map(|v| v.0);
            let secondary_id = packet.secondary_effect.map(|v| v.0);

            if !beacon_handler.inventory.is_empty()
                && BeaconBlockEntity::validate_effects(primary_id, secondary_id, levels)
            {
                beacon_handler
                    .properties
                    .set_property(1, primary_id.map_or(0, |id| id + 1));
                beacon_handler
                    .properties
                    .set_property(2, secondary_id.map_or(0, |id| id + 1));
                beacon_handler.inventory.remove_stack_specific(0, 1);
                let world = player.world();
                if let Some(be) = world.get_block_entity(&beacon_handler.position)
                    && let Some(beacon) = be.as_any().downcast_ref::<BeaconBlockEntity>()
                {
                    beacon.mark_dirty();
                    if !beacon
                        .beam_sections
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .is_empty()
                    {
                        world.play_sound(
                            pumpkin_data::sound::Sound::BlockBeaconPowerSelect,
                            pumpkin_data::sound::SoundCategory::Blocks,
                            &beacon_handler.position.to_centered_f64(),
                        );
                    }
                }

                screen_handler.sync_state();

                info!(
                    "Player {} updated beacon effects: primary {:?}, secondary {:?}",
                    player.gameprofile.name, primary_id, secondary_id
                );
                true
            } else {
                false
            }
        };

        if !is_valid {
            let primary_id = packet.primary_effect.map(|v| v.0);
            let secondary_id = packet.secondary_effect.map(|v| v.0);
            warn!(
                "Player {} tried to set invalid beacon effects: primary {:?}, secondary {:?}",
                player.gameprofile.name, primary_id, secondary_id
            );
            self.try_kick(&TextComponent::translate(
                "multiplayer.disconnect.generic",
                &[],
            ));
        }
    }
}
