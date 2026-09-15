use std::sync::{Arc, atomic::Ordering};

use pumpkin_util::math::position::BlockPos;

use crate::{block::entities::BlockEntity, world::World};

pub use pumpkin_inventory::ViewerCountTracker;

pub trait ViewerCountTrackerExt {
    fn update_viewer_count<T>(&self, entity: &T, world: &Arc<World>, position: &BlockPos)
    where
        T: BlockEntity + ViewerCountListener + 'static;

    fn update_viewer_count_with_source<T>(
        &self,
        entity: &T,
        world: &Arc<World>,
        position: &BlockPos,
        source: Option<i32>,
    ) where
        T: BlockEntity + ViewerCountListener + 'static;
}

impl ViewerCountTrackerExt for ViewerCountTracker {
    fn update_viewer_count<T>(&self, entity: &T, world: &Arc<World>, position: &BlockPos)
    where
        T: BlockEntity + ViewerCountListener + 'static,
    {
        self.update_viewer_count_with_source(entity, world, position, None);
        if !entity.rechecks_viewers() {
            return;
        }
        let now = world
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age;
        let next = self.next_recheck.load(Ordering::Relaxed);
        if next < 0 || now < next {
            return;
        }
        let range = f64::from_bits(self.max_interaction_range.load(Ordering::Relaxed)) + 4.0;
        let min =
            position.to_f64() - pumpkin_util::math::vector3::Vector3::new(range, range, range);
        let max = position.to_f64()
            + pumpkin_util::math::vector3::Vector3::new(1.0 + range, 1.0 + range, 1.0 + range);
        let mut count = 0u16;
        let mut max_range: f64 = 0.0;
        for player in
            world.get_players_at_box(&pumpkin_util::math::boundingbox::BoundingBox::new(min, max))
        {
            if player.is_spectator() {
                continue;
            }
            // A busy screen is not evidence that its viewer left. Retry next tick.
            let Some(is_viewer) = player_views_container(&player, *position) else {
                return;
            };
            if is_viewer {
                count = count.saturating_add(1);
                max_range = max_range.max(player.block_interaction_range());
            }
        }
        self.max_interaction_range
            .store(max_range.to_bits(), Ordering::Relaxed);
        let previous = self.current.swap(count, Ordering::Relaxed);
        self.update_viewer_count_with_source(entity, world, position, None);
        if previous == count {
            entity.on_viewer_count_update(world, position, previous, count);
        }
        self.next_recheck
            .store(if count == 0 { -1 } else { now + 5 }, Ordering::Relaxed);
    }

    fn update_viewer_count_with_source<T>(
        &self,
        entity: &T,
        world: &Arc<World>,
        position: &BlockPos,
        source: Option<i32>,
    ) where
        T: BlockEntity + ViewerCountListener + 'static,
    {
        if let Some(player) = source.and_then(|id| world.get_player_by_id(id)) {
            self.max_interaction_range.fetch_max(
                player.block_interaction_range().to_bits(),
                Ordering::Relaxed,
            );
        }
        let current = self.current.load(Ordering::Relaxed);
        let old = self.old.swap(current, Ordering::Relaxed);
        if old != current {
            match (old, current) {
                (n, 0) if n > 0 => {
                    self.next_recheck.store(-1, Ordering::Relaxed);
                    self.max_interaction_range.store(0, Ordering::Relaxed);
                    entity.on_container_close(world, position);
                    world.emit_game_event_with_source(
                        "container_close",
                        position.to_centered_f64(),
                        source,
                    );
                }
                (0, n) if n > 0 => {
                    let now = world
                        .level_time
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .world_age;
                    self.next_recheck.store(now + 5, Ordering::Relaxed);
                    entity.on_container_open(world, position);
                    world.emit_game_event_with_source(
                        "container_open",
                        position.to_centered_f64(),
                        source,
                    );
                }
                _ => {} // Ignore
            }

            entity.on_viewer_count_update(world, position, old, current);
        }
    }
}

fn player_views_container(
    player: &crate::entity::player::Player,
    position: BlockPos,
) -> Option<bool> {
    use pumpkin_inventory::generic_container_screen_handler::GenericContainerScreenHandler;
    let handler = player.current_screen_handler.try_lock().ok()?.clone();
    let handler = handler.try_lock().ok()?;
    Some(
        handler
            .as_any()
            .downcast_ref::<GenericContainerScreenHandler>()
            .is_some_and(|handler| handler.inventory.contains_viewer_position(position)),
    )
}

pub trait ViewerCountListener: Send + Sync {
    // Shulker boxes use their own open-count behavior, not ContainerOpenersCounter.
    fn rechecks_viewers(&self) -> bool {
        false
    }

    fn on_container_open(&self, _world: &Arc<World>, _position: &BlockPos) {}

    fn on_container_close(&self, _world: &Arc<World>, _position: &BlockPos) {}

    fn on_viewer_count_update(
        &self,
        _world: &Arc<World>,
        _position: &BlockPos,
        _old: u16,
        _new: u16,
    ) {
    }
}
