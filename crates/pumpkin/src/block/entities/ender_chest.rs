use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::block::viewer::{ViewerCountListener, ViewerCountTracker, ViewerCountTrackerExt};
use crate::world::World;

use super::BlockEntity;

pub struct EnderChestBlockEntity {
    pub position: BlockPos,

    removed: AtomicBool,

    // Viewer
    viewers: Arc<ViewerCountTracker>,
}

impl BlockEntity for EnderChestBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        Self {
            position,
            viewers: Arc::new(ViewerCountTracker::at(position)),
            removed: AtomicBool::new(false),
        }
    }

    fn set_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        Some(NbtCompound::new())
    }

    fn refresh_viewers(&self, world: &Arc<World>, source: Option<i32>) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers
            .update_viewer_count_with_source(self, world, &self.position, source);
    }

    fn tick(&self, world: &Arc<World>) {
        if self.removed.load(Ordering::Relaxed) {
            return;
        }
        self.viewers
            .update_viewer_count::<Self>(self, world, &self.position);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ViewerCountListener for EnderChestBlockEntity {
    fn rechecks_viewers(&self) -> bool {
        true
    }

    fn on_container_open(&self, world: &Arc<World>, _position: &BlockPos) {
        self.play_sound(world, Sound::BlockEnderChestOpen);
    }

    fn on_container_close(&self, world: &Arc<World>, _position: &BlockPos) {
        self.play_sound(world, Sound::BlockEnderChestClose);
    }

    fn on_viewer_count_update(&self, world: &Arc<World>, position: &BlockPos, _old: u16, new: u16) {
        world.add_synced_block_event(*position, Self::LID_ANIMATION_EVENT_TYPE, new as u8);
    }
}

impl EnderChestBlockEntity {
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
    pub const ID: &'static str = "minecraft:ender_chest";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            viewers: Arc::new(ViewerCountTracker::at(position)),
            removed: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn get_tracker(&self) -> Arc<ViewerCountTracker> {
        self.viewers.clone()
    }

    fn play_sound(&self, world: &Arc<World>, sound: Sound) {
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &self.position.to_centered_f64(),
            0.5,
            world.rand_f32() * 0.1 + 0.9,
        );
    }
}
