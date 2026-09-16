use crate::chunk::format::linear::LinearV2File;
use crate::chunk::format::pump::PumpFile;
use crate::chunk_system::{ChunkListener, ChunkLoading, GenerationSchedule, LevelChannel};
use crate::generation::generator::WorldGenerator;
use crate::lighting::DynamicLightEngine;
use crate::{
    chunk::{
        ChunkData, ChunkEntityData, ChunkReadingError,
        format::anvil::AnvilChunkFile,
        io::{
            Dirtiable, FileIO, LoadedData,
            file_manager::{ChunkFileManager, LevelFileIO},
        },
        palette::has_random_ticking_fluid,
    },
    generation::get_world_gen_with_all_settings,
    tick::{OrderedTick, ScheduledTick, TickPriority},
    world::WorldPortalExt,
};
use arc_swap::ArcSwap;
use crossbeam::queue::SegQueue;
use dashmap::{DashMap, Entry};
use pumpkin_config::{chunk::ChunkConfig, lighting::LightingEngineConfig, world::LevelConfig};
use pumpkin_data::biome::Biome;
use pumpkin_data::dimension::Dimension;
use pumpkin_data::{Block, BlockStateId, block_properties::has_random_ticks, fluid::Fluid};
use pumpkin_util::math::{position::BlockPos, vector2::Vector2};
use pumpkin_util::world_seed::Seed;
use rustc_hash::FxHashSet;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering},
    thread,
};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};
// use tokio::runtime::Handle;
use tokio::{
    select,
    sync::{
        mpsc::{self, Receiver},
        oneshot,
    },
    task::JoinHandle,
};
use tokio_util::task::TaskTracker;

pub type SyncChunk = Arc<ChunkData>;
pub type SyncEntityChunk = Arc<ChunkEntityData>;
type EntityLoadWaiters = Vec<oneshot::Sender<Result<SyncEntityChunk, String>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadedChunkChange {
    Loaded(Vector2<i32>),
    Unloaded(Vector2<i32>),
}

pub type ChunkSaver =
    LevelFileIO<LinearV2File<ChunkData>, AnvilChunkFile<ChunkData>, PumpFile<ChunkData>>;

pub type EntitySaver = LevelFileIO<
    LinearV2File<ChunkEntityData>,
    AnvilChunkFile<ChunkEntityData>,
    PumpFile<ChunkEntityData>,
>;

/// The `Level` module provides functionality for working with chunks within or outside a Minecraft world.
///
/// Key features include:
///
/// - **Chunk Loading:** Efficiently loads chunks from disk.
/// - **Chunk Caching:** Stores accessed chunks in memory for faster access.
/// - **Chunk Generation:** Generates new chunks on-demand using a specified `WorldGenerator`.
///
/// For more details on world generation, refer to the `WorldGenerator` module.
pub struct Level {
    pub seed: Seed,
    pub world_portal: ArcSwap<Option<Arc<dyn WorldPortalExt>>>,
    pub level_folder: Arc<LevelFolder>,
    pub lighting_config: LightingEngineConfig,

    /// Counts the number of ticks that have been scheduled for this world
    schedule_tick_counts: AtomicI64,

    /// Vanilla `Level.randValue`: the dedicated LCG that picks random-tick positions.
    /// Seeded once at construction from a thread-local source, exactly as vanilla does
    /// (`Level.java:116`), so positions are not reproducible across restarts in vanilla
    /// either -- what must match is the algorithm and the number of advances.
    rand_value: AtomicI32,

    // Chunks that are paired with chunk watchers. When a chunk is no longer watched, it is removed
    // from the loaded chunks map and sent to the underlying ChunkIO
    pub loaded_chunks: Arc<DashMap<Vector2<i32>, SyncChunk>>,
    pub(crate) loaded_chunk_changes: Arc<SegQueue<LoadedChunkChange>>,
    loaded_entity_chunks: Arc<DashMap<Vector2<i32>, SyncEntityChunk>>,
    pub chunks_with_scheduled_ticks: Arc<dashmap::DashSet<Vector2<i32>>>,
    pub chunk_loading: Mutex<ChunkLoading>,

    chunk_watchers: Arc<DashMap<Vector2<i32>, usize>>,

    pub chunk_saver: Arc<ChunkSaver>,
    entity_saver: Arc<EntitySaver>,

    pub world_gen: ArcSwap<WorldGenerator>,

    /// Handles runtime lighting updates
    pub light_engine: DynamicLightEngine,

    /// Tracks tasks associated with this world instance
    tasks: TaskTracker,
    pub chunk_system_tasks: TaskTracker,
    /// Notification that interrupts tasks for shutdown
    pub cancel_token: CancellationToken,

    pub shut_down_chunk_system: AtomicBool,
    pub should_save: AtomicBool,
    pub should_unload: AtomicBool,
    /// Whether periodic autosaving is enabled. Toggled by `/save-off` and `/save-on`;
    /// a manual `/save-all` still saves while this is `false`.
    pub save_enabled: AtomicBool,
    /// Number of ticks between autosave checks. If 0, autosave is disabled.
    pub autosave_ticks: u64,

    pending_entity_loads: Arc<DashMap<Vector2<i32>, EntityLoadWaiters>>,

    pub level_channel: Arc<LevelChannel>,
    pub thread_tracker: Mutex<Vec<thread::JoinHandle<()>>>,
    pub chunk_listener: Arc<ChunkListener>,
}

pub struct TickData {
    pub block_ticks: Vec<OrderedTick<&'static Block>>,
    pub fluid_ticks: Vec<OrderedTick<&'static Fluid>>,
    pub random_ticks: Vec<RandomTickSample>,
}

#[derive(Clone, Copy)]
pub struct RandomTickSample {
    pub position: BlockPos,
    pub tick_block: bool,
    pub tick_fluid: bool,
}

pub struct LevelFolder {
    pub root_folder: PathBuf,
    pub dim_folder: PathBuf,
    pub region_folder: PathBuf,
    pub entities_folder: PathBuf,
    pub poi_folder: PathBuf,
}

impl Level {
    #[must_use]
    #[expect(clippy::too_many_lines)]
    pub fn from_root_folder(
        level_config: &LevelConfig,
        root_folder: PathBuf,
        seed: i64,
        dimension: Dimension,
    ) -> Arc<Self> {
        let (namespace, name) = match dimension.minecraft_name.split_once(':') {
            Some((ns, n)) => (ns, n),
            None => ("minecraft", dimension.minecraft_name),
        };

        // 26.2 canonical layout: root_folder/dimensions/<namespace>/<name>
        let canonical_dim_folder = root_folder.join("dimensions").join(namespace).join(name);

        // Check if canonical 26.2 folder exists, or fall back to pre-26.2 legacy folders
        let dim_folder = if canonical_dim_folder.exists() {
            canonical_dim_folder
        } else if dimension.minecraft_name == Dimension::OVERWORLD.minecraft_name
            && root_folder.join("region").exists()
        {
            root_folder.clone()
        } else if dimension.minecraft_name == Dimension::THE_NETHER.minecraft_name
            && root_folder.join("DIM-1").join("region").exists()
        {
            root_folder.join("DIM-1")
        } else if dimension.minecraft_name == Dimension::THE_END.minecraft_name
            && root_folder.join("DIM1").join("region").exists()
        {
            root_folder.join("DIM1")
        } else {
            canonical_dim_folder
        };

        let region_folder = dim_folder.join("region");
        let entities_folder = dim_folder.join("entities");
        let poi_folder = dim_folder.join("poi");

        let _ = std::fs::create_dir_all(&region_folder);
        let _ = std::fs::create_dir_all(&entities_folder);
        let _ = std::fs::create_dir_all(&poi_folder);

        let level_folder = Arc::new(LevelFolder {
            root_folder,
            dim_folder,
            region_folder,
            entities_folder,
            poi_folder,
        });

        let main_folder = &level_folder.root_folder;

        let mut is_flat = false;
        let mut flat_layers = Vec::new();
        let mut flat_biome = "minecraft:plains".to_string();
        let mut generator_settings_name: Option<String> = None;
        let mut biome_source: Option<crate::world_info::BiomeSource> = None;
        let mut structure_overrides: Option<Vec<String>> = None;

        if let Some(wgs) = crate::world_info::data_files::read_world_gen_settings(main_folder)
            && let Some(dim_settings) = wgs.dimensions.get(dimension.minecraft_name)
        {
            biome_source.clone_from(&dim_settings.generator.biome_source);

            if dim_settings.generator.generator_type == "minecraft:flat" {
                is_flat = true;
                let flat_settings = dim_settings
                    .generator
                    .settings
                    .as_ref()
                    .and_then(crate::world_info::GeneratorSettings::as_flat_settings)
                    .or_else(|| {
                        crate::world_info::FlatLevelGeneratorPreset::from_name("classic_flat")
                            .map(|p| p.settings)
                    });
                if let Some(flat_settings) = flat_settings {
                    flat_layers = flat_settings.to_flat_layers();
                    structure_overrides = flat_settings.structure_overrides_vec();
                    flat_biome = flat_settings.biome;
                }
            } else if let Some(crate::world_info::GeneratorSettings::Reference(s)) =
                &dim_settings.generator.settings
            {
                generator_settings_name = Some(s.clone());
            }
        }

        let seed = Seed(seed as u64);
        let world_gen: Arc<WorldGenerator> = Arc::from(get_world_gen_with_all_settings(
            seed,
            dimension,
            is_flat,
            flat_layers,
            flat_biome,
            generator_settings_name.as_deref(),
            biome_source.as_ref(),
            structure_overrides.as_deref(),
        ));

        let chunk_saver = match &level_config.chunk {
            ChunkConfig::Linear => Arc::new(ChunkSaver::Linear(ChunkFileManager::new(()))),
            ChunkConfig::Anvil(config) => {
                Arc::new(ChunkSaver::Anvil(ChunkFileManager::new(config.clone())))
            }
            ChunkConfig::Pump => Arc::new(ChunkSaver::Pump(ChunkFileManager::new(()))),
        };
        let entity_saver = match &level_config.chunk {
            ChunkConfig::Linear => Arc::new(EntitySaver::Linear(ChunkFileManager::new(()))),
            ChunkConfig::Anvil(config) => {
                Arc::new(EntitySaver::Anvil(ChunkFileManager::new(config.clone())))
            }
            ChunkConfig::Pump => Arc::new(EntitySaver::Pump(ChunkFileManager::new(()))),
        };

        let pending_entity_loads = Arc::new(DashMap::new());
        let level_channel = Arc::new(LevelChannel::new());
        let thread_tracker = Mutex::new(Vec::new());
        let listener = Arc::new(ChunkListener::new());

        let level_ref = Arc::new(Self {
            seed,
            world_portal: ArcSwap::new(Arc::new(None)),
            world_gen: ArcSwap::new(world_gen),
            level_folder,
            lighting_config: level_config.lighting,
            light_engine: DynamicLightEngine::new(),
            chunk_saver,
            entity_saver,
            schedule_tick_counts: AtomicI64::new(0),
            rand_value: AtomicI32::new(rand::random::<i32>()),
            loaded_chunks: Arc::new(DashMap::new()),
            loaded_chunk_changes: Arc::new(SegQueue::new()),
            loaded_entity_chunks: Arc::new(DashMap::new()),
            chunks_with_scheduled_ticks: Arc::new(dashmap::DashSet::new()),
            chunk_loading: Mutex::new(ChunkLoading::new(level_channel.clone())),
            chunk_watchers: Arc::new(DashMap::new()),
            tasks: TaskTracker::new(),
            chunk_system_tasks: TaskTracker::new(),
            cancel_token: CancellationToken::new(),
            shut_down_chunk_system: AtomicBool::new(false),
            should_save: AtomicBool::new(false),
            should_unload: AtomicBool::new(false),
            save_enabled: AtomicBool::new(true),
            autosave_ticks: level_config.autosave_ticks,
            pending_entity_loads,
            level_channel: level_channel.clone(),
            thread_tracker,
            chunk_listener: listener.clone(),
        });

        GenerationSchedule::create(
            4,
            level_ref.clone(),
            level_channel,
            listener,
            level_ref
                .thread_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_mut(),
        );

        level_ref
    }

    pub fn set_world_gen(&self, generator: Arc<WorldGenerator>) {
        self.world_gen.store(generator);
    }

    #[must_use]
    pub fn world_gen(&self) -> Arc<WorldGenerator> {
        self.world_gen.load_full()
    }

    /// Spawns a task associated with this world. All tasks spawned with this method are awaited
    /// when the client. This means tasks should complete in a reasonable (no looping) amount of time.
    pub fn spawn_task<F>(&self, task: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.tasks.spawn(task)
    }

    pub async fn shutdown(&self) {
        let world_id = self.level_folder.root_folder.display();
        info!("Saving level ({})...", world_id);
        self.cancel_token.cancel();
        self.shut_down_chunk_system.store(true, Ordering::Relaxed);
        self.level_channel.notify();

        self.tasks.close();
        self.chunk_system_tasks.close();

        let handles = {
            let mut lock = self
                .thread_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.drain(..).collect::<Vec<_>>()
        };

        let handle_count = handles.len();
        info!("Joining {} threads for {}...", handle_count, world_id);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = std::thread::Builder::new()
            .name("Thread-Joiner".into())
            .spawn(move || {
                let mut failed_count = 0;
                for handle in handles {
                    if handle.join().is_err() {
                        failed_count += 1;
                    }
                }
                let _ = tx.send(failed_count);
            });

        match timeout(Duration::from_secs(3), rx).await {
            Ok(Ok(failed_count)) => {
                if failed_count > 0 {
                    warn!(
                        "{} threads failed to join properly for {}.",
                        failed_count, world_id
                    );
                }
            }
            Ok(Err(_)) => {
                warn!("Thread join task panicked for {}.", world_id);
            }
            Err(_) => {
                warn!("Timed out waiting for threads to join for {}.", world_id);
            }
        }

        self.tasks.wait().await;
        self.chunk_system_tasks.wait().await;

        info!("Flushing chunk data to disk for {}...", world_id);
        self.chunk_saver.block_and_await_ongoing_tasks().await;
        info!("Flushing entity data to disk for {}...", world_id);
        self.entity_saver.block_and_await_ongoing_tasks().await;

        // save all chunks currently in memory
        let chunks_to_write = self
            .loaded_entity_chunks
            .iter()
            .map(|chunk| (*chunk.key(), chunk.value().clone()))
            .collect::<Vec<_>>();
        self.loaded_entity_chunks.clear();

        // TODO: I think the chunk_saver should be at the server level
        self.entity_saver.clear_watched_chunks().await;
        self.write_entity_chunks(chunks_to_write).await;
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.loaded_chunks.len()
    }

    pub fn list_cached(&self) {
        for entry in self.loaded_chunks.iter() {
            debug!("In map: {:?}", entry.key());
        }
    }

    /// Marks chunks as "watched" by a unique player. When no players are watching a chunk,
    /// it is removed from memory. Should only be called on chunks the player was not watching
    /// before
    pub async fn mark_chunks_as_newly_watched(&self, chunks: &[Vector2<i32>]) {
        for chunk in chunks {
            self.chunk_watchers
                .entry(*chunk)
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }

        self.entity_saver
            .watch_chunks(&self.level_folder, chunks)
            .await;
    }

    /// Marks chunks no longer "watched" by a unique player. When no players are watching a chunk,
    /// it is removed from memory. Should only be called on chunks the player was watching before
    pub async fn mark_chunks_as_not_watched(
        &self,
        chunks: impl IntoIterator<Item = impl std::borrow::Borrow<Vector2<i32>>>,
    ) -> Vec<Vector2<i32>> {
        let mut chunks_to_clean = Vec::new();
        let chunks_vec: Vec<Vector2<i32>> = chunks.into_iter().map(|c| *c.borrow()).collect();

        for chunk in &chunks_vec {
            if let Entry::Occupied(mut entry) = self.chunk_watchers.entry(*chunk) {
                *entry.get_mut() = entry.get().saturating_sub(1);
                if *entry.get() == 0 {
                    entry.remove();
                    chunks_to_clean.push(*chunk);
                }
            }
        }

        self.entity_saver
            .unwatch_chunks(&self.level_folder, &chunks_vec)
            .await;
        chunks_to_clean
    }

    /// Returns whether the chunk should be removed from memory
    #[inline]
    pub async fn mark_chunk_as_not_watched(&self, chunk: Vector2<i32>) -> bool {
        !self.mark_chunks_as_not_watched([chunk]).await.is_empty()
    }

    // In Level::clean_entity_chunks()
    pub fn clean_entity_chunks(
        self: &Arc<Self>,
        chunks: impl IntoIterator<Item = impl std::borrow::Borrow<Vector2<i32>>>,
    ) {
        let chunks_to_process: Vec<_> = chunks
            .into_iter()
            .filter_map(|pos_borrow| {
                let pos = pos_borrow.borrow();
                if self.should_retain_entity_chunk(pos) {
                    return None;
                }

                // Remove immediately to prevent race conditions
                self.loaded_entity_chunks.remove(pos)
            })
            .collect();

        if chunks_to_process.is_empty() {
            return;
        }

        let level = self.clone();
        self.spawn_task(async move {
            debug!("Writing {} entity chunks to disk", chunks_to_process.len());
            level.write_entity_chunks(chunks_to_process).await;
        });
    }

    /// Vanilla `Level.getBlockRandomPos` (`Level.java:1066`):
    ///
    /// ```java
    /// this.randValue = this.randValue * 3 + 1013904223;
    /// int val = this.randValue >> 2;
    /// return new BlockPos(xo + (val & 15), yo + (val >> 16 & yMask), zo + (val >> 8 & 15));
    /// ```
    ///
    /// Note the bit positions: x from bits 0-3, z from bits 8-11 and y from bits 16-19.
    /// Each call advances the LCG exactly once.
    pub fn get_block_random_pos(&self, xo: i32, yo: i32, zo: i32, y_mask: i32) -> BlockPos {
        // fetch_update returns the previous value; recompute the new one to use here.
        let previous = self
            .rand_value
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some(v.wrapping_mul(3).wrapping_add(1_013_904_223))
            })
            .unwrap_or(0);
        let val = previous.wrapping_mul(3).wrapping_add(1_013_904_223) >> 2;

        BlockPos::new(
            xo + (val & 15),
            yo + ((val >> 16) & y_mask),
            zo + ((val >> 8) & 15),
        )
    }

    pub fn get_scheduled_ticks(
        &self,
    ) -> (
        Vec<OrderedTick<&'static Block>>,
        Vec<OrderedTick<&'static Fluid>>,
    ) {
        self.get_scheduled_ticks_if(|_| true)
    }

    pub fn get_scheduled_ticks_if(
        &self,
        eligible: impl Fn(&Vector2<i32>) -> bool,
    ) -> (
        Vec<OrderedTick<&'static Block>>,
        Vec<OrderedTick<&'static Fluid>>,
    ) {
        (
            self.get_scheduled_block_ticks_if(&eligible),
            self.get_scheduled_fluid_ticks_if(eligible),
        )
    }

    pub fn get_scheduled_block_ticks_if(
        &self,
        eligible: impl Fn(&Vector2<i32>) -> bool,
    ) -> Vec<OrderedTick<&'static Block>> {
        self.collect_scheduled_ticks_if(eligible, |chunk| &chunk.block_ticks)
    }

    pub fn get_scheduled_fluid_ticks_if(
        &self,
        eligible: impl Fn(&Vector2<i32>) -> bool,
    ) -> Vec<OrderedTick<&'static Fluid>> {
        self.collect_scheduled_ticks_if(eligible, |chunk| &chunk.fluid_ticks)
    }

    fn collect_scheduled_ticks_if<T: std::hash::Hash + Eq + 'static>(
        &self,
        eligible: impl Fn(&Vector2<i32>) -> bool,
        queue: impl Fn(&ChunkData) -> &crate::tick::scheduler::ChunkTickScheduler<&'static T>,
    ) -> Vec<OrderedTick<&'static T>> {
        // Release the index shard before reading chunk storage or removing entries.
        let positions: Vec<_> = self
            .chunks_with_scheduled_ticks
            .iter()
            .map(|p| *p)
            .collect();
        let mut chunks = Vec::new();
        for pos in positions {
            if let Some(chunk) = self.loaded_chunks.get(&pos) {
                chunks.push((pos, chunk.value().clone()));
            } else {
                self.chunks_with_scheduled_ticks.remove(&pos);
            }
        }
        let queues: Vec<_> = chunks
            .iter()
            .map(|(pos, chunk)| (queue(chunk), eligible(pos)))
            .collect();
        let ticks = crate::tick::scheduler::collect_ticks(&queues, 65536);
        for (pos, chunk) in chunks {
            if !chunk.block_ticks.has_ticks() && !chunk.fluid_ticks.has_ticks() {
                self.chunks_with_scheduled_ticks.remove(&pos);
            }
        }
        ticks
    }

    pub fn get_random_ticks(
        &self,
        active_chunks: &FxHashSet<Vector2<i32>>,
        random_tick_speed: i64,
    ) -> Vec<RandomTickSample> {
        let samples_per_section = random_tick_speed.max(0);
        let mut random_ticks = Vec::with_capacity(active_chunks.len() * 3);

        // Process active chunks (random ticks)
        //
        // Sorted: active_chunks is an FxHashSet whose iteration order is arbitrary, and
        // random_ticks is executed in collection order.
        let mut active_chunks_sorted: Vec<_> = active_chunks.iter().copied().collect();
        active_chunks_sorted.sort_unstable_by_key(|pos| (pos.x, pos.y));
        for pos in &active_chunks_sorted {
            if let Some(chunk) = self.loaded_chunks.get(pos) {
                let chunk = chunk.value();
                let chunk_x_base = chunk.x * 16;
                let chunk_z_base = chunk.z * 16;
                let section_count = chunk.section.count;

                // Use the bitmask to skip sections
                let mask = chunk.section.randomly_ticking_mask.load(Ordering::Relaxed);
                if mask != 0 {
                    let sections = chunk
                        .section
                        .block_sections
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let min_y = chunk.section.min_y;

                    for i in 0..section_count {
                        if (mask & (1 << i)) == 0 {
                            continue;
                        }
                        let y_base = min_y + (i as i32 * 16);
                        for _ in 0..samples_per_section {
                            let pos = self.get_block_random_pos(chunk_x_base, y_base, chunk_z_base, 15);
                            let x_offset = (pos.0.x - chunk_x_base) as usize;
                            let y_in_section = (pos.0.y - y_base) as usize;
                            let z_offset = (pos.0.z - chunk_z_base) as usize;

                            let block_state_id = sections[i].get(x_offset, y_in_section, z_offset);
                            let tick_block = has_random_ticks(block_state_id);
                            let tick_fluid = has_random_ticking_fluid(block_state_id);
                            if tick_block || tick_fluid {
                                random_ticks.push(RandomTickSample {
                                    position: pos,
                                    tick_block,
                                    tick_fluid,
                                });
                            }
                        }
                    }
                }
            }
        }

        random_ticks
    }

    pub fn get_tick_data(
        &self,
        active_chunks: &FxHashSet<Vector2<i32>>,
        random_tick_speed: i64,
    ) -> TickData {
        let (block_ticks, fluid_ticks) = self.get_scheduled_ticks();
        let random_ticks = self.get_random_ticks(active_chunks, random_tick_speed);
        TickData {
            block_ticks,
            fluid_ticks,
            random_ticks,
        }
    }

    pub fn clean_entity_chunk(self: &Arc<Self>, chunk: &Vector2<i32>) {
        self.clean_entity_chunks([*chunk]);
    }

    pub fn is_chunk_watched(&self, chunk: &Vector2<i32>) -> bool {
        self.chunk_watchers.get(chunk).is_some()
    }

    pub fn should_retain_entity_chunk(&self, pos: &Vector2<i32>) -> bool {
        if self.chunk_watchers.get(pos).is_some_and(|count| *count > 0) {
            return true;
        }
        self.chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pos_level
            .get(pos)
            .is_some_and(|level| *level <= ChunkLoading::FULL_CHUNK_LEVEL)
    }

    pub fn clean_memory(self: &Arc<Self>) -> Vec<Vector2<i32>> {
        self.chunk_watchers.retain(|_, watcher| *watcher != 0);

        let candidates: Vec<_> = self
            .loaded_entity_chunks
            .iter()
            .map(|entry| *entry.key())
            .collect();
        let entity_chunks_to_remove: Vec<_> = candidates
            .into_iter()
            .filter(|pos| !self.should_retain_entity_chunk(pos))
            .collect();

        // We do not clean them here because we want the caller to save any active entities in them first.

        // if the difference is too big, we can shrink the loaded chunks
        // (1024 chunks is the equivalent to a 32x32 chunks area)
        if self.chunk_watchers.capacity() - self.chunk_watchers.len() >= 4096 {
            self.chunk_watchers.shrink_to_fit();
        }

        if self.loaded_chunks.capacity() - self.loaded_chunks.len() >= 4096 {
            self.loaded_chunks.shrink_to_fit();
        }

        if self.loaded_entity_chunks.capacity() - self.loaded_entity_chunks.len() >= 4096 {
            self.loaded_entity_chunks.shrink_to_fit();
        }
        entity_chunks_to_remove
    }

    pub async fn get_or_fetch_chunk<R, F: Fn(&SyncChunk) -> R>(
        self: &Arc<Self>,
        pos: Vector2<i32>,
        f: F,
    ) -> R {
        // Check if already in memory
        if let Some(res) = self.read_chunk_sync(&pos, &f) {
            return res;
        }
        let chunk = self.fetch_chunk(pos).await;
        if self.loaded_chunks.insert(pos, chunk.clone()).is_none() {
            self.loaded_chunk_changes
                .push(LoadedChunkChange::Loaded(pos));
        }
        f(&chunk)
    }

    pub fn loaded_chunk_changes(&self) -> impl Iterator<Item = LoadedChunkChange> + '_ {
        std::iter::from_fn(|| self.loaded_chunk_changes.pop())
    }

    async fn fetch_chunk(self: &Arc<Self>, pos: Vector2<i32>) -> SyncChunk {
        let recv = self.chunk_listener.add_single_chunk_listener(pos);

        {
            let mut lock = self
                .chunk_loading
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.add_ticket(pos, ChunkLoading::FULL_CHUNK_LEVEL);
            lock.send_change();
        };

        let chunk = recv
            .await
            .unwrap_or_else(|_| ChunkData::empty_sync(pos.x, pos.y));

        {
            let mut lock = self
                .chunk_loading
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.remove_ticket(pos, ChunkLoading::FULL_CHUNK_LEVEL);
            lock.send_change();
        };

        chunk
    }

    async fn load_single_entity_chunk(
        &self,
        pos: Vector2<i32>,
    ) -> Result<(SyncEntityChunk, bool), ChunkReadingError> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        self.entity_saver
            .fetch_chunks(&self.level_folder, &[pos], tx)
            .await;

        match rx.recv().await {
            Some(LoadedData::Loaded(chunk)) => Ok((chunk, false)),
            Some(LoadedData::Error((_, err))) => Err(err),
            _ => Err(ChunkReadingError::ChunkNotExist),
        }
    }

    pub fn receive_entity_chunks(
        self: &Arc<Self>,
        chunks: Vec<Vector2<i32>>,
    ) -> Receiver<(Weak<ChunkEntityData>, bool)> {
        let (sender, receiver) = mpsc::channel(64);
        let level = self.clone();
        self.spawn_task(async move {
            use futures::StreamExt;
            let loads = futures::stream::iter(chunks.into_iter().map(|pos| {
                let level = level.clone();
                async move {
                    let was_loaded = level.get_entity_chunk_sync(&pos).is_some();
                    (level.try_load_entity_chunk(pos).await, !was_loaded)
                }
            }))
            .buffered(32);
            tokio::pin!(loads);
            let receive = async {
                while let Some((result, first)) = loads.next().await {
                    match result {
                        Ok(chunk) => {
                            if sender.send((Arc::downgrade(&chunk), first)).await.is_err() {
                                break;
                            }
                        }
                        Err(error) => warn!("Unable to load entity chunk: {error}"),
                    }
                }
            };
            tokio::select! {
                () = level.cancel_token.cancelled() => {},
                () = receive => {},
            }
        });
        receiver
    }

    /// Share both disk reads and empty-chunk creation across all simultaneous
    /// requests. Cancelling a requester does not cancel or replace the shared load.
    pub async fn try_load_entity_chunk(
        self: &Arc<Self>,
        pos: Vector2<i32>,
    ) -> Result<SyncEntityChunk, String> {
        if let Some(chunk) = self.get_entity_chunk_sync(&pos) {
            return Ok(chunk);
        }
        let (tx, rx) = oneshot::channel();
        let start = match self.pending_entity_loads.entry(pos) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                entry.get_mut().push(tx);
                false
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(vec![tx]);
                true
            }
        };
        if start {
            let level = self.clone();
            self.spawn_task(async move {
                let result = if let Some(chunk) = level.get_entity_chunk_sync(&pos) {
                    Ok(chunk)
                } else {
                    match level.load_single_entity_chunk(pos).await {
                        Ok((chunk, _)) => Ok(chunk),
                        Err(ChunkReadingError::ChunkNotExist) => Ok(Arc::new(ChunkEntityData {
                            x: pos.x,
                            z: pos.y,
                            data: Mutex::new(Vec::new()),
                            dirty: AtomicBool::new(false),
                        })),
                        Err(error) => Err(error.to_string()),
                    }
                };
                if let Ok(chunk) = &result {
                    level.loaded_entity_chunks.insert(pos, chunk.clone());
                }
                if let Some((_, waiters)) = level.pending_entity_loads.remove(&pos) {
                    for waiter in waiters {
                        let _ = waiter.send(result.clone());
                    }
                }
            });
        }
        rx.await
            .map_err(|_| "Entity storage load task stopped".to_owned())?
    }

    pub async fn get_entity_chunk(self: &Arc<Self>, pos: Vector2<i32>) -> SyncEntityChunk {
        self.try_load_entity_chunk(pos)
            .await
            .unwrap_or_else(|error| {
                // Legacy callers do not expose errors. Keep the fallback detached:
                // failed storage must never be replaced in the cache with an empty chunk.
                warn!("Unable to load entity chunk {pos:?}: {error}");
                Arc::new(ChunkEntityData {
                    x: pos.x,
                    z: pos.y,
                    data: Mutex::new(Vec::new()),
                    dirty: AtomicBool::new(false),
                })
            })
    }

    pub fn get_block_state(&self, position: &BlockPos) -> BlockStateId {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        let id = self
            .read_chunk_sync(&chunk_coordinate, |chunk| {
                chunk.section.get_block_absolute_y(
                    relative.x as usize,
                    relative.y,
                    relative.z as usize,
                )
            })
            .flatten();

        id.unwrap_or(Block::VOID_AIR.default_state.id)
    }

    pub fn set_block_state(
        &self,
        position: &BlockPos,
        block_state_id: BlockStateId,
    ) -> BlockStateId {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        self.read_chunk_sync(&chunk_coordinate, |chunk| {
            let replaced_block_state_id = chunk.set_block_absolute_y(
                relative.x as usize,
                relative.y,
                relative.z as usize,
                block_state_id,
            );
            if replaced_block_state_id != block_state_id {
                chunk.mark_dirty(true);
            }
            replaced_block_state_id
        })
        .unwrap_or(Block::VOID_AIR.default_state.id)
    }

    pub async fn write_chunks(&self, chunks_to_write: Vec<(Vector2<i32>, SyncChunk)>) {
        if chunks_to_write.is_empty() {
            return;
        }

        let chunk_saver = self.chunk_saver.clone();
        let level_folder = self.level_folder.clone();

        trace!("Sending chunks to ChunkIO {:}", chunks_to_write.len());
        if let Err(error) = chunk_saver
            .save_chunks(&level_folder, chunks_to_write)
            .await
        {
            error!("Failed writing Chunk to disk {error}");
        }
    }

    pub async fn write_entity_chunks(&self, chunks_to_write: Vec<(Vector2<i32>, SyncEntityChunk)>) {
        if chunks_to_write.is_empty() {
            return;
        }

        let chunk_saver = self.entity_saver.clone();
        let level_folder = self.level_folder.clone();

        trace!("Sending chunks to ChunkIO {:}", chunks_to_write.len());
        if let Err(error) = chunk_saver
            .save_chunks(&level_folder, chunks_to_write)
            .await
        {
            error!("Failed writing Chunk to disk {error}");
        }
    }

    pub fn is_chunk_loaded(&self, coordinates: &Vector2<i32>) -> bool {
        self.loaded_chunks.contains_key(coordinates)
    }

    pub fn read_chunk_sync<R, F: Fn(&SyncChunk) -> R>(
        &self,
        coordinates: &Vector2<i32>,
        f: F,
    ) -> Option<R> {
        self.loaded_chunks.get(coordinates).map(|x| f(x.value()))
    }

    pub fn read_entity_chunk_sync<R, F: Fn(&SyncEntityChunk) -> R>(
        &self,
        coordinates: &Vector2<i32>,
        f: F,
    ) -> Option<R> {
        self.loaded_entity_chunks
            .get(coordinates)
            .map(|x| f(x.value()))
    }

    pub fn get_rough_biome(&self, position: &BlockPos) -> &'static Biome {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        let id = self.read_chunk_sync(&chunk_coordinate, |chunk| {
            chunk.section.get_rough_biome_absolute_y(
                relative.x as usize,
                relative.y,
                relative.z as usize,
            )
        });
        Biome::from_id(id.flatten().unwrap_or(0)).unwrap_or(&Biome::THE_VOID)
    }

    pub fn get_entity_chunk_sync(&self, pos: &Vector2<i32>) -> Option<SyncEntityChunk> {
        self.loaded_entity_chunks
            .get(pos)
            .map(|x| x.value().clone())
    }

    pub async fn get_or_fetch_entity_chunk<R, F: Fn(&SyncEntityChunk) -> R>(
        self: &Arc<Self>,
        pos: Vector2<i32>,
        f: F,
    ) -> R {
        if let Some(res) = self.read_entity_chunk_sync(&pos, &f) {
            return res;
        }
        let chunk = self.get_entity_chunk(pos).await;
        f(&chunk)
    }

    pub fn try_get_entity_chunk(
        &self,
        coordinates: Vector2<i32>,
    ) -> Option<dashmap::mapref::one::Ref<'_, Vector2<i32>, Arc<ChunkEntityData>>> {
        self.loaded_entity_chunks.try_get(&coordinates).try_unwrap()
    }

    pub fn schedule_block_tick(
        &self,
        block: &Block,
        block_pos: BlockPos,
        delay: u32,
        priority: TickPriority,
    ) {
        let tick_order = self.schedule_tick_counts.fetch_add(1, Ordering::Relaxed);
        let scheduled_tick = ScheduledTick {
            delay,
            position: block_pos,
            priority,
            // SAFETY: `block` is a valid reference that outlives this function call for scheduling.
            value: unsafe { &*std::ptr::from_ref::<Block>(block) },
        };

        let chunk_pos = block_pos.chunk_position();
        if self
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk.block_ticks.schedule_tick(&scheduled_tick, tick_order);
            })
            .is_some()
        {
            self.chunks_with_scheduled_ticks.insert(chunk_pos);
        }
    }

    pub fn schedule_fluid_tick(
        &self,
        fluid: &Fluid,
        block_pos: BlockPos,
        delay: u32,
        priority: TickPriority,
    ) {
        let tick_order = self.schedule_tick_counts.fetch_add(1, Ordering::Relaxed);
        let scheduled_tick = ScheduledTick {
            delay,
            position: block_pos,
            priority,
            // SAFETY: `fluid` is a valid reference that outlives this function call for scheduling.
            value: unsafe { &*std::ptr::from_ref::<Fluid>(fluid) },
        };

        let chunk_pos = block_pos.chunk_position();
        if self
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk.fluid_ticks.schedule_tick(&scheduled_tick, tick_order);
            })
            .is_some()
        {
            self.chunks_with_scheduled_ticks.insert(chunk_pos);
        }
    }

    pub fn is_block_tick_scheduled(&self, block_pos: &BlockPos, block: &Block) -> bool {
        self.read_chunk_sync(&block_pos.chunk_position(), |chunk| {
            chunk.block_ticks.is_scheduled(*block_pos, block)
        })
        .unwrap_or(false)
    }

    pub fn is_fluid_tick_scheduled(&self, block_pos: &BlockPos, fluid: &Fluid) -> bool {
        self.read_chunk_sync(&block_pos.chunk_position(), |chunk| {
            chunk.fluid_ticks.is_scheduled(*block_pos, fluid)
        })
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_config::world::LevelConfig;
    use tempfile::TempDir;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_entity_loads_share_storage_and_cached_receivers_deliver_every_chunk() {
        use futures::future::join_all;
        let dir = TempDir::new().unwrap();
        let level = Level::from_root_folder(
            &LevelConfig::default(),
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        let pos = Vector2::new(0, 0);
        let results = join_all((0..64).map(|_| level.try_load_entity_chunk(pos))).await;
        let first = results[0].as_ref().unwrap();
        assert!(
            results
                .iter()
                .all(|result| Arc::ptr_eq(first, result.as_ref().unwrap()))
        );
        // The old try_send path silently dropped cached chunks beyond capacity 64.
        let mut receiver = level.receive_entity_chunks(vec![pos; 130]);
        let mut received = 0;
        while let Some((chunk, _)) = receiver.recv().await {
            assert!(Arc::ptr_eq(first, &chunk.upgrade().unwrap()));
            received += 1;
        }
        assert_eq!(received, 130);
        level.shutdown().await;
    }

    #[tokio::test]
    async fn block_callbacks_can_enqueue_same_tick_fluids() {
        let dir = TempDir::new().unwrap();
        let level = Level::from_root_folder(
            &LevelConfig::default(),
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        let pos = BlockPos::new(0, 64, 0);
        level
            .loaded_chunks
            .insert(Vector2::new(0, 0), ChunkData::empty_sync(0, 0));
        level.schedule_block_tick(&Block::STONE, pos, 0, TickPriority::Normal);
        assert_eq!(level.get_scheduled_block_ticks_if(|_| true).len(), 1);
        // This is the ordering in World::tick_scheduled_ticks: dispatch blocks,
        // then collect fluids, including work scheduled by those block callbacks.
        level.schedule_fluid_tick(&Fluid::WATER, pos, 0, TickPriority::Normal);
        assert!(level.get_scheduled_fluid_ticks_if(|_| false).is_empty());
        assert!(level.is_fluid_tick_scheduled(&pos, &Fluid::WATER));
        let due = level.get_scheduled_fluid_ticks_if(|_| true);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].position, pos);
        assert!(!level.is_fluid_tick_scheduled(&pos, &Fluid::WATER));
        level.shutdown().await;
    }

    #[tokio::test]
    async fn dimension_paths_26_2() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let config = LevelConfig::default();

        let overworld_level =
            Level::from_root_folder(&config, root.clone(), 0, Dimension::OVERWORLD);
        assert_eq!(
            overworld_level.level_folder.dim_folder,
            root.join("dimensions").join("minecraft").join("overworld")
        );
        assert_eq!(
            overworld_level.level_folder.region_folder,
            root.join("dimensions")
                .join("minecraft")
                .join("overworld")
                .join("region")
        );

        let nether_level = Level::from_root_folder(&config, root.clone(), 0, Dimension::THE_NETHER);
        assert_eq!(
            nether_level.level_folder.dim_folder,
            root.join("dimensions").join("minecraft").join("the_nether")
        );

        let end_level = Level::from_root_folder(&config, root.clone(), 0, Dimension::THE_END);
        assert_eq!(
            end_level.level_folder.dim_folder,
            root.join("dimensions").join("minecraft").join("the_end")
        );
    }

    #[tokio::test]
    async fn legacy_dimension_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let config = LevelConfig::default();

        // Create legacy directories
        std::fs::create_dir_all(root.join("region")).unwrap();
        std::fs::create_dir_all(root.join("DIM-1").join("region")).unwrap();
        std::fs::create_dir_all(root.join("DIM1").join("region")).unwrap();

        let overworld_level =
            Level::from_root_folder(&config, root.clone(), 0, Dimension::OVERWORLD);
        assert_eq!(overworld_level.level_folder.dim_folder, root);

        let nether_level = Level::from_root_folder(&config, root.clone(), 0, Dimension::THE_NETHER);
        assert_eq!(nether_level.level_folder.dim_folder, root.join("DIM-1"));

        let end_level = Level::from_root_folder(&config, root.clone(), 0, Dimension::THE_END);
        assert_eq!(end_level.level_folder.dim_folder, root.join("DIM1"));
    }
}

#[cfg(test)]
mod block_random_pos_tests {
    /// Same arithmetic as `Level::get_block_random_pos`, lifted out so the sequence can
    /// be checked without building a whole `Level`.
    fn next(rand_value: &mut i32, xo: i32, yo: i32, zo: i32, y_mask: i32) -> (i32, i32, i32) {
        *rand_value = rand_value.wrapping_mul(3).wrapping_add(1_013_904_223);
        let val = *rand_value >> 2;
        (
            xo + (val & 15),
            yo + ((val >> 16) & y_mask),
            zo + ((val >> 8) & 15),
        )
    }

    /// Reference values produced by running vanilla's exact expression from
    /// `Level.java:1066` under Java with `randValue = 12345`.
    #[test]
    fn matches_java_reference_sequence() {
        let mut rand_value = 12345_i32;
        let expected = [
            (2, 75, 1),
            (15, 79, 15),
            (5, 73, 12),
            (8, 73, 2),
            (0, 71, 3),
            (9, 66, 7),
        ];
        for (i, want) in expected.iter().enumerate() {
            let got = next(&mut rand_value, 0, 64, 0, 15);
            assert_eq!(got, *want, "sample {i} diverged from the Java reference");
        }
    }

    /// Guards the bug this replaced: y must come from bits 16-19, not 4-7.
    #[test]
    fn y_is_drawn_from_the_high_bits() {
        let mut rand_value = 12345_i32;
        let (_, y, _) = next(&mut rand_value, 0, 0, 0, 15);
        let val = 12345_i32.wrapping_mul(3).wrapping_add(1_013_904_223) >> 2;
        assert_eq!(y, (val >> 16) & 15);
        assert_ne!(y, (val >> 4) & 15, "regressed to the pre-parity bit offset");
    }
}
