#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_player_ground(&self, player: &Player, ground: &SSetPlayerGround) {
        // A movement packet was received this tick — tracked for SClientTickEnd zeroing.
        self.received_movement_this_tick
            .store(true, Ordering::Relaxed);
        if !player.accepts_movement() {
            return;
        }
        player.handle_flight_movement(
            Vector3::default(),
            player.last_client_movement.swap(Vector3::default()),
            ground.on_ground,
            ground.horizontal_collision,
        );
    }
}
