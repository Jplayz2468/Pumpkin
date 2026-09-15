use std::sync::{Arc, atomic::Ordering};

use pumpkin_util::math::position::BlockPos;

use crate::{block::entities::BlockEntity, world::World};

pub use pumpkin_inventory::ViewerCountTracker;

pub trait ViewerCountTrackerExt {
    fn update_viewer_count<T>(&self, entity: &T, world: &Arc<World>, position: &BlockPos)
    where
        T: BlockEntity + ViewerCountListener + 'static,
    {
        self.update_viewer_count_with_source(entity, world, position, None);
    }

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
    fn update_viewer_count_with_source<T>(
        &self,
        entity: &T,
        world: &Arc<World>,
        position: &BlockPos,
        source: Option<i32>,
    ) where
        T: BlockEntity + ViewerCountListener + 'static,
    {
        let current = self.current.load(Ordering::Relaxed);
        let old = self.old.swap(current, Ordering::Relaxed);
        if old != current {
            match (old, current) {
                (n, 0) if n > 0 => {
                    entity.on_container_close(world, position);
                    world.emit_game_event_with_source(
                        "container_close",
                        position.to_centered_f64(),
                        source,
                    );
                }
                (0, n) if n > 0 => {
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

pub trait ViewerCountListener: Send + Sync {
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
