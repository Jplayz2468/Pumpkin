use super::chunk_state::{Chunk, StagedChunkEnum};
use super::generation_cache::Cache;
use super::{ChunkPos, IOLock};
use crate::ProtoChunk;
use crate::chunk::format::LightContainer;
use crate::chunk::io::LoadedData::Loaded;
use crate::chunk::io::{FileIO, LoadedData, run_blocking};
use crate::level::Level;
use pumpkin_config::lighting::LightingEngineConfig;
use pumpkin_data::chunk::ChunkStatus;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use tracing::{debug, error, warn};

pub enum RecvChunk {
    IO(Chunk),
    Generation(Cache),
    GenerationFailure {
        pos: ChunkPos,
        stage: StagedChunkEnum,
        error: String,
    },
}

/// Checks if a chunk needs relighting based on the current lighting configuration
/// Returns true if the chunk has uniform lighting (from full/dark mode) but the server
/// is now running in default mode (which needs proper lighting calculation)
fn needs_relighting(chunk: &crate::chunk::ChunkData, config: LightingEngineConfig) -> bool {
    if config != LightingEngineConfig::Default {
        return false;
    }

    // If the chunk says it's already lit, believe it.
    if chunk.light_populated.load(Relaxed) {
        return false;
    }

    let engine = chunk
        .light_engine
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Scan for any complex lighting data
    let has_complex_light = engine.sky_light.iter().any(|lc| match lc {
        LightContainer::Full(data) => data.iter().any(|&b| b != 0x00 && b != 0xFF),
        LightContainer::Empty(val) => *val != 0 && *val != 15,
    }) || engine.block_light.iter().any(|lc| match lc {
        LightContainer::Full(data) => data.iter().any(|&b| b != 0x00 && b != 0xFF),
        LightContainer::Empty(val) => *val != 0 && *val != 15,
    });

    // If it has complex light, we don't need to relight.
    !has_complex_light
}

fn load_proto_chunk(chunk: &crate::chunk::ChunkData, level: &Level) -> ProtoChunk {
    ProtoChunk::from_chunk_data(chunk, &level.world_gen.load())
}

fn process_loaded_chunk(chunk: Arc<crate::chunk::ChunkData>, level: &Level) -> Chunk {
    let pos = ChunkPos::new(chunk.x, chunk.z);
    if chunk.status == ChunkStatus::Full {
        let needs_relight = needs_relighting(&chunk, level.lighting_config);
        if needs_relight {
            debug!(
                "Chunk {pos:?} has uniform lighting, downgrading to Features stage for relighting"
            );

            let mut proto = load_proto_chunk(&chunk, level);

            // Clear all lighting data
            let section_count = proto.light.sky_light.len();
            proto.light.sky_light = (0..section_count)
                .map(|_| LightContainer::new_empty(15))
                .collect();
            proto.light.block_light = (0..section_count)
                .map(|_| LightContainer::new_empty(0))
                .collect();
            proto.stage = StagedChunkEnum::Features;
            Chunk::Proto(Box::new(proto))
        } else {
            Chunk::Level(chunk)
        }
    } else {
        let proto = load_proto_chunk(&chunk, level);
        Chunk::Proto(Box::new(proto))
    }
}

pub async fn io_read_work(
    recv: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Vec<ChunkPos>>>>,
    send: crossbeam::channel::Sender<(ChunkPos, RecvChunk)>,
    level: Arc<Level>,
    lock: IOLock,
) {
    debug!("io read thread start");

    // Cleaner loop and async recv
    loop {
        let batch = {
            let mut lock_rx = recv.lock().await;
            lock_rx.recv().await
        };
        let Some(batch) = batch else {
            break;
        };
        for pos in &batch {
            // Lock handling
            loop {
                let notified = lock.1.notified();
                if !lock
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(pos)
                {
                    break;
                }
                notified.await;
            }
        }

        let (t_send, mut t_recv) = tokio::sync::mpsc::channel(1000);

        let batch_len = batch.len();
        let level_clone = level.clone();

        let fetch_task = tokio::spawn(async move {
            let mut disk = Vec::new();
            for pos in batch {
                let pending = level_clone
                    .pending_chunk_writes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&pos)
                    .cloned();
                if let Some(pending) = pending {
                    let result = run_blocking(move || pending.read()).await;
                    let data = match result {
                        Ok(Ok(chunk)) => Loaded(chunk),
                        result => LoadedData::Error((
                            pos,
                            crate::chunk::ChunkReadingError::IoError(std::io::Error::other(
                                match result {
                                    Ok(Err(error)) => error,
                                    Err(error) => error.to_string(),
                                    _ => unreachable!(),
                                },
                            )),
                        )),
                    };
                    if t_send.send(data).await.is_err() {
                        return;
                    }
                } else {
                    disk.push(pos);
                }
            }
            level_clone
                .chunk_saver
                .fetch_chunks(&level_clone.level_folder, &disk, t_send)
                .await;
        });

        for _ in 0..batch_len {
            let Some(data) = t_recv.recv().await else {
                break;
            };

            match data {
                Loaded(chunk) => {
                    let pos = ChunkPos::new(chunk.x, chunk.z);
                    let level = level.clone();
                    let result = run_blocking(move || process_loaded_chunk(chunk, &level)).await;
                    let received = match result {
                        Ok(processed) => RecvChunk::IO(processed),
                        Err(err) => RecvChunk::GenerationFailure {
                            pos,
                            stage: StagedChunkEnum::Empty,
                            error: err.to_string(),
                        },
                    };
                    if send.send((pos, received)).is_err() {
                        break;
                    }
                }
                LoadedData::Error((pos, error)) => {
                    // Preserve the holder as an unsuccessful load. Only absence
                    // permits new terrain; corruption/I/O failures retry loading.
                    tokio::select! {
                        _ = level.cancel_token.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
                    }
                    if send
                        .send((
                            pos,
                            RecvChunk::GenerationFailure {
                                pos,
                                stage: StagedChunkEnum::Empty,
                                error: format!("Chunk read failed: {error}"),
                            },
                        ))
                        .is_err()
                    {
                        return;
                    }
                }
                LoadedData::Missing(pos) => {
                    if send
                        .send((
                            pos,
                            RecvChunk::IO(Chunk::Proto(Box::new(ProtoChunk::new(
                                pos.x,
                                pos.y,
                                &level.world_gen.load(),
                            )))),
                        ))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        let _ = fetch_task.await;
    }
    debug!("io read thread stop");
}

pub(crate) struct ChunkWrite {
    pub chunks: Vec<(ChunkPos, Chunk)>,
    pub completion: Vec<crate::level::SaveCompletion>,
}

impl From<Vec<(ChunkPos, Chunk)>> for ChunkWrite {
    fn from(chunks: Vec<(ChunkPos, Chunk)>) -> Self {
        Self {
            chunks,
            completion: Vec::new(),
        }
    }
}

pub(crate) async fn io_write_work(
    mut recv: tokio::sync::mpsc::Receiver<ChunkWrite>,
    level: Arc<Level>,
    lock: IOLock,
) {
    loop {
        // Don't check cancel_token here (keep saving chunks)
        let Some(job) = recv.recv().await else { break };
        let data = job.chunks;
        // debug!("io write thread receive chunks size {}", data.len());
        let positions = data.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();
        let level_for_upgrade = level.clone();
        let upgrade_result = run_blocking(move || {
            let mut vec = Vec::with_capacity(data.len());
            for (pos, chunk) in data {
                match chunk {
                    Chunk::Level(chunk) => vec.push((pos, chunk)),
                    Chunk::Proto(chunk) => {
                        let mut temp = Chunk::Proto(chunk);
                        temp.upgrade_to_level_chunk(
                            level_for_upgrade.world_gen.load().dimension(),
                            &level_for_upgrade.lighting_config,
                        );
                        let Chunk::Level(chunk) = temp else { panic!() };
                        vec.push((pos, chunk));
                    }
                }
            }
            vec
        })
        .await;
        let result = match upgrade_result {
            Ok(vec) => {
                let pending: Vec<_> = {
                    let mut cache = level
                        .pending_chunk_writes
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for (pos, source) in vec {
                        cache.insert(
                            pos,
                            Arc::new(super::pending_write::PendingWrite::new(source)),
                        );
                    }
                    cache
                        .iter()
                        .map(|(pos, pending)| (*pos, pending.clone()))
                        .collect()
                };
                let captured = run_blocking(move || {
                    let mut snapshots = Vec::new();
                    let mut entries = Vec::new();
                    let mut errors = Vec::new();
                    for (pos, entry) in pending {
                        match entry.snapshot() {
                            Ok(image) => {
                                snapshots.push((pos, image));
                                entries.push((pos, entry));
                            }
                            Err(error) => errors.push(format!("Snapshot {pos:?}: {error}")),
                        }
                    }
                    (snapshots, entries, errors)
                })
                .await;
                match captured {
                    Err(error) => Err(format!("Chunk snapshot task failed: {error}")),
                    Ok((snapshots, entries, mut errors)) => {
                        match level
                            .chunk_saver
                            .save_chunks(&level.level_folder, snapshots)
                            .await
                        {
                            Ok(()) => {
                                let mut cache = level
                                    .pending_chunk_writes
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                for (pos, entry) in entries {
                                    if cache
                                        .get(&pos)
                                        .is_some_and(|current| Arc::ptr_eq(current, &entry))
                                    {
                                        cache.remove(&pos);
                                    }
                                }
                            }
                            Err(error) => {
                                for (_, entry) in entries {
                                    entry.failed();
                                }
                                errors.push(error.to_string());
                            }
                        }
                        if errors.is_empty() {
                            Ok(())
                        } else {
                            Err(errors.join("; "))
                        }
                    }
                }
            }
            Err(error) => Err(format!("Failed to upgrade chunks for saving: {error}")),
        };
        if let Err(error) = &result {
            error!("Failed to save chunks: {error}");
        }

        {
            let mut data = lock
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for i in positions {
                match data.entry(i) {
                    Entry::Occupied(mut entry) => {
                        let rc = entry.get_mut();
                        if *rc <= 1 {
                            entry.remove();
                        } else {
                            *rc -= 1;
                        }
                    }
                    Entry::Vacant(_) => {
                        warn!(
                            "io_write: attempted to release missing lock entry for {:?}",
                            i
                        );
                    }
                }
            }
        }
        lock.1.notify_waiters();
        for completion in job.completion {
            let _ = completion.send(result.clone());
        }
    }
}

pub fn run_generation(
    pos: ChunkPos,
    mut cache: Cache,
    stage: StagedChunkEnum,
    level: &Level,
) -> RecvChunk {
    let portal = level.world_portal.load_full();
    let Some(portal_ref) = portal.as_deref() else {
        error!("Chunk generation FAILED at {pos:?} ({stage:?}): World portal is not initialized");
        return RecvChunk::GenerationFailure {
            pos,
            stage,
            error: "World portal is not initialized".to_string(),
        };
    };
    // Run generation with panic catching
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cache.advance(
            stage,
            &level.world_gen.load(),
            portal_ref,
            &level.lighting_config,
        );
        cache // Return cache on success
    }));

    match result {
        Ok(cache) => RecvChunk::Generation(cache),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| {
                    payload
                        .downcast_ref::<String>()
                        .map(std::string::String::as_str)
                })
                .unwrap_or("Unknown panic payload");

            error!("Chunk generation FAILED at {pos:?} ({stage:?}): {msg}");

            RecvChunk::GenerationFailure {
                pos,
                stage,
                error: msg.to_string(),
            }
        }
    }
}
