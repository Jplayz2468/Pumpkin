use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use pumpkin_data::block_properties::JukeboxLikeProperties;
use pumpkin_data::data_component_impl::{BlockEntityDataImpl, JukeboxPlayableImpl};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::jukebox_song::JukeboxSong;
use pumpkin_data::particle::Particle;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockStateId};
use pumpkin_inventory::{Clearable, Inventory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::BlockEntity;
use crate::entity::Entity;
use crate::entity::item::ItemEntity;
use crate::world::World;

/// Single-item container and JukeboxSongPlayer, including the twenty-tick ending grace period.
pub struct JukeboxBlockEntity {
    position: BlockPos,
    world: Mutex<Weak<World>>,
    state: Mutex<BlockStateId>,
    record_stack: Mutex<ItemStack>,
    ticks_since_song_started: AtomicI64,
    /// Zero means stopped; otherwise this includes the twenty-tick grace period.
    playback_end_tick: AtomicU64,
    dirty: AtomicBool,
    comparator_dirty: AtomicBool,
}

const RECORD_ITEM_NBT_KEY: &str = "RecordItem";
const TICKS_SINCE_SONG_STARTED_NBT_KEY: &str = "ticks_since_song_started";

impl BlockEntity for JukeboxBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn set_world(&self, world: Weak<World>) {
        if let Some(level) = world.upgrade() {
            *self.state.lock().unwrap() = level.get_block_state_id(&self.position);
        }
        *self.world.lock().unwrap() = world;
    }

    fn set_removed(&self) {
        if let Some(world) = self.world() {
            self.emit_stop(&world);
        }
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        let entity = Self::new(position);
        let record = nbt
            .get_compound(RECORD_ITEM_NBT_KEY)
            .and_then(ItemStack::read_item_stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone());
        if let Some(ticks) = nbt.get_long(TICKS_SINCE_SONG_STARTED_NBT_KEY)
            && let Some(song) = Self::song_from_stack(&record)
            && ticks < (song.length_in_ticks() + 20) as i64
        {
            entity
                .ticks_since_song_started
                .store(ticks, Ordering::Relaxed);
            entity
                .playback_end_tick
                .store(song.length_in_ticks() + 20, Ordering::Relaxed);
        }
        *entity.record_stack.lock().unwrap() = record;
        entity
    }

    fn apply_implicit_components(&self, stack: &ItemStack) {
        let Some(data) = stack.get_data_component::<BlockEntityDataImpl>() else {
            return;
        };
        if data.nbt.get_string("id").is_some_and(|id| id != Self::ID) {
            return;
        }
        let loaded = Self::from_nbt(&data.nbt, self.position);
        self.stop_playing();
        *self.record_stack.lock().unwrap() = loaded.get_record();
        self.ticks_since_song_started.store(
            loaded.ticks_since_song_started.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.playback_end_tick.store(
            loaded.playback_end_tick.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.mark_dirty();
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let record = self.get_record();
        if !record.is_empty() {
            let mut record_nbt = NbtCompound::new();
            record.write_item_stack(&mut record_nbt);
            nbt.put(RECORD_ITEM_NBT_KEY, record_nbt);
        }
        if self.is_playing() {
            nbt.put_long(
                TICKS_SINCE_SONG_STARTED_NBT_KEY,
                self.ticks_since_song_started.load(Ordering::Relaxed),
            );
        }
    }

    fn tick(&self, world: &Arc<World>) {
        let state = world.get_block_state_id(&self.position);
        // JukeboxBlock only supplies a ticker while HAS_RECORD is true.
        if state.to_block() != &Block::JUKEBOX
            || !JukeboxLikeProperties::from_state_id(state).has_record
            || !self.is_playing()
        {
            return;
        }
        *self.state.lock().unwrap() = state;
        let ticks = self.ticks_since_song_started.load(Ordering::Relaxed);
        if ticks >= self.playback_end_tick.load(Ordering::Relaxed) as i64 {
            self.stop_playing();
            return;
        }
        if ticks % 20 == 0 {
            world.emit_game_event_from_entity(
                "jukebox_play",
                self.position.to_centered_f64(),
                None,
                Some(state),
            );
            let color = world.rand_bounded_i32(4) as f32 / 24.0;
            world.spawn_particles(
                Particle::Note,
                self.position
                    .to_f64()
                    .add(&Vector3::new(0.5, f64::from(1.2_f32), 0.5)),
                0,
                Vector3::new(color, 0.0, 0.0),
                1.0,
            );
        }
        self.ticks_since_song_started
            .fetch_add(1, Ordering::Relaxed);
    }

    fn on_block_replaced_with_state(
        self: Arc<Self>,
        _world: &Arc<World>,
        _position: &BlockPos,
        old_state: BlockStateId,
    ) {
        *self.state.lock().unwrap() = old_state;
        self.pop_out_item();
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }
    fn is_comparator_dirty(&self) -> bool {
        self.comparator_dirty.load(Ordering::Relaxed)
    }
    fn clear_comparator_dirty(&self) {
        self.comparator_dirty.store(false, Ordering::Relaxed);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }
}

impl JukeboxBlockEntity {
    pub const ID: &'static str = "minecraft:jukebox";

    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            world: Mutex::new(Weak::new()),
            state: Mutex::new(Block::JUKEBOX.default_state.id),
            record_stack: Mutex::new(ItemStack::EMPTY.clone()),
            ticks_since_song_started: AtomicI64::new(0),
            playback_end_tick: AtomicU64::new(0),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
        }
    }

    fn world(&self) -> Option<Arc<World>> {
        self.world.lock().unwrap().upgrade()
    }

    pub fn song_from_stack(stack: &ItemStack) -> Option<JukeboxSong> {
        let playable = stack.get_data_component::<JukeboxPlayableImpl>()?;
        // The generated registry contains vanilla keys only. Do not turn custom namespaces into vanilla songs.
        let name = playable
            .song
            .strip_prefix("minecraft:")
            .unwrap_or(playable.song);
        JukeboxSong::from_name(name)
    }

    pub fn get_record(&self) -> ItemStack {
        self.record_stack.lock().unwrap().clone()
    }

    fn notify_item_changed(&self, world: &Arc<World>, inserted: bool) {
        let state = world.get_block_state_id(&self.position);
        if state.to_block() == &Block::JUKEBOX {
            let next = JukeboxLikeProperties {
                has_record: inserted,
            }
            .to_state_id(&Block::JUKEBOX);
            world.set_block_state(&self.position, next, BlockFlags::NOTIFY_LISTENERS);
            *self.state.lock().unwrap() = world.get_block_state_id(&self.position);
            let current_state = *self.state.lock().unwrap();
            world.emit_game_event_from_entity(
                "block_change",
                self.position.to_centered_f64(),
                None,
                Some(current_state),
            );
        }
    }

    pub fn set_record(&self, stack: ItemStack) {
        let inserted = !stack.is_empty();
        let song = Self::song_from_stack(&stack);
        *self.record_stack.lock().unwrap() = stack;
        if let Some(world) = self.world() {
            self.notify_item_changed(&world, inserted);
        }
        if inserted && let Some(song) = song {
            self.start_playing(song.length_in_ticks());
        } else {
            self.stop_playing();
        }
        self.mark_dirty();
    }

    pub fn clear_record(&self) -> ItemStack {
        let record = self.get_record();
        self.set_record(ItemStack::EMPTY.clone());
        record
    }

    fn on_song_changed(&self, world: &Arc<World>) {
        world.update_neighbors_at(&self.position, &Block::JUKEBOX, None);
        self.mark_dirty();
    }

    /// Retained for the plugin API; ordinary insertion resolves its duration from the record.
    pub fn start_playing(&self, length_in_ticks: u64) {
        self.ticks_since_song_started.store(0, Ordering::Relaxed);
        self.playback_end_tick
            .store(length_in_ticks.saturating_add(20), Ordering::Relaxed);
        if let Some(world) = self.world() {
            if let Some(song) = Self::song_from_stack(&self.get_record()) {
                world.sync_world_event(
                    WorldEvent::SoundPlayJukeboxSong,
                    self.position,
                    song.get_id() as i32,
                );
            }
            self.on_song_changed(&world);
        }
        self.mark_dirty();
    }

    fn emit_stop(&self, world: &Arc<World>) {
        let state = *self.state.lock().unwrap();
        world.emit_game_event_from_entity(
            "jukebox_stop_play",
            self.position.to_centered_f64(),
            None,
            Some(state),
        );
        world.sync_world_event(WorldEvent::SoundStopJukeboxSong, self.position, 0);
    }

    pub fn stop_playing(&self) {
        if self.playback_end_tick.swap(0, Ordering::Relaxed) == 0 {
            return;
        }
        self.ticks_since_song_started.store(0, Ordering::Relaxed);
        if let Some(world) = self.world() {
            self.emit_stop(&world);
            self.on_song_changed(&world);
        }
        self.mark_dirty();
    }

    pub fn is_playing(&self) -> bool {
        self.playback_end_tick.load(Ordering::Relaxed) != 0
    }

    pub fn pop_out_item(&self) {
        let Some(world) = self.world() else {
            return;
        };
        if self.get_record().is_empty() {
            return;
        }
        let record = self.clear_record();
        let position = Vector3::new(
            f64::from(self.position.0.x) + 0.5 + f64::from((world.rand_f32() - 0.5) * 0.7),
            f64::from(self.position.0.y) + 1.01,
            f64::from(self.position.0.z) + 0.5 + f64::from((world.rand_f32() - 0.5) * 0.7),
        );
        let entity = Entity::new(world.clone(), position, &EntityType::ITEM);
        world.spawn_entity(Arc::new(ItemEntity::new(entity, record)));
        self.on_song_changed(&world);
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }
}

impl Inventory for JukeboxBlockEntity {
    fn size(&self) -> usize {
        1
    }
    fn get_max_count_per_stack(&self) -> u8 {
        1
    }
    fn is_empty(&self) -> bool {
        self.record_stack.lock().unwrap().is_empty()
    }
    fn get_stack(&self, _slot: usize) -> ItemStack {
        self.get_record()
    }
    fn remove_stack(&self, _slot: usize) -> ItemStack {
        self.clear_record()
    }
    fn remove_stack_specific(&self, _slot: usize, _amount: u8) -> ItemStack {
        self.clear_record()
    }
    fn set_stack(&self, _slot: usize, stack: ItemStack) {
        self.set_record(stack);
    }
    fn is_valid_slot_for(&self, _slot: usize, stack: &ItemStack) -> bool {
        stack.get_data_component::<JukeboxPlayableImpl>().is_some() && self.is_empty()
    }
    fn can_transfer_to(&self, into: &dyn Inventory, _slot: usize, _stack: &ItemStack) -> bool {
        (0..into.size()).any(|slot| into.get_stack(slot).is_empty())
    }
    fn mark_dirty(&self) {
        JukeboxBlockEntity::mark_dirty(self);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for JukeboxBlockEntity {
    fn clear(&self) {
        self.clear_record();
    }
}
