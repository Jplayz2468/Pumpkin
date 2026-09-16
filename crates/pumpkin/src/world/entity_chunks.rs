//! Saved residents become live through world ticking, independently of viewers.
use super::World;
use crate::entity::{EntityBase, r#type::from_type};
use pumpkin_data::entity::EntityType;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{vector2::Vector2, vector3::Vector3};
use pumpkin_world::{chunk::ChunkEntityData, level::SyncEntityChunk};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::{Arc, Mutex, RwLock, Weak};
use uuid::Uuid;

#[derive(Default)]
pub(super) struct EntityChunkLifecycle {
    pending: Mutex<FxHashSet<Vector2<i32>>>,
    snapshot_roots: Mutex<FxHashMap<Vector2<i32>, FxHashSet<Uuid>>>,
    pending_snapshot: std::sync::atomic::AtomicBool,
    ready: RwLock<FxHashMap<Vector2<i32>, Weak<ChunkEntityData>>>,
    completed: crossbeam::queue::SegQueue<(Vector2<i32>, Result<SyncEntityChunk, String>)>,
}

impl World {
    pub(crate) fn are_entities_loaded(&self, pos: &Vector2<i32>) -> bool {
        let ready = self
            .entity_chunks
            .ready
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(pos)
            .and_then(Weak::upgrade);
        ready.is_some_and(|ready| {
            self.level
                .get_entity_chunk_sync(pos)
                .is_some_and(|current| Arc::ptr_eq(&ready, &current))
        })
    }

    pub(crate) fn request_entity_chunk(&self, pos: Vector2<i32>) {
        if self.are_entities_loaded(&pos) {
            return;
        }
        let handle = self
            .server
            .upgrade()
            .map(|s| s.runtime.clone())
            .or_else(|| tokio::runtime::Handle::try_current().ok());
        let Some(handle) = handle else {
            return;
        };
        if !self
            .entity_chunks
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(pos)
        {
            return;
        }
        let weak = self.self_reference.clone();
        let level = self.level.clone();
        let _guard = handle.enter();
        self.level.spawn_task(async move {
            let result = level.try_load_entity_chunk(pos).await;
            if let Some(world) = weak.upgrade() {
                world.entity_chunks.completed.push((pos, result));
            }
        });
    }

    pub(crate) fn forget_entity_chunk(&self, pos: &Vector2<i32>) {
        self.entity_chunks
            .ready
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(pos);
        self.entity_chunks
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(pos);
    }

    pub(crate) fn activate_completed_entity_chunks(&self) {
        let Some(world) = self.self_reference.upgrade() else {
            return;
        };
        while let Some((pos, result)) = self.entity_chunks.completed.pop() {
            let chunk = match result {
                Ok(chunk) => chunk,
                Err(error) => {
                    // Like Java's failed loadingInbox future, this chunk remains
                    // unready. Do not replace corrupt storage with an empty chunk.
                    tracing::error!("Entity chunk {pos:?} remains unready: {error}");
                    continue;
                }
            };
            let current = self.level.get_entity_chunk_sync(&pos);
            if self.level.should_retain_entity_chunk(&pos)
                && self.level.is_chunk_loaded(&pos)
                && current.is_some_and(|current| Arc::ptr_eq(&current, &chunk))
                && !self.are_entities_loaded(&pos)
            {
                let saved = std::mem::take(
                    &mut *chunk
                        .data
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
                for nbt in saved {
                    load_tree(&world, &nbt, None);
                }
                self.entity_chunks
                    .ready
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(pos, Arc::downgrade(&chunk));
            }
            self.entity_chunks
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&pos);
        }
        self.entity_chunks
            .ready
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, chunk| chunk.strong_count() != 0);
    }
}

fn load_tree(world: &Arc<World>, nbt: &NbtCompound, parent: Option<&Arc<dyn EntityBase>>) {
    let Some(id) = nbt.get_string("id") else {
        return;
    };
    let Some(kind) = EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id)) else {
        tracing::warn!("Skipping unknown saved entity type {id}");
        return;
    };
    let uuid = nbt.get_uuid("UUID").unwrap_or_else(Uuid::new_v4);
    if world.get_entity_by_uuid(uuid).is_some() {
        return;
    }
    let entity = from_type(kind, Vector3::new(0.0, 0.0, 0.0), world, uuid);
    entity.read_nbt_non_mut(nbt);
    entity.init_data_tracker();
    world.add_entity_silent(entity.clone());
    if let Some(parent) = parent {
        parent
            .get_entity()
            .add_passenger(parent.clone(), entity.clone());
    }
    for player in world.players.load().iter() {
        player.try_restore_vehicle(&entity);
    }
    if let Some(passengers) = nbt.get_list("Passengers") {
        for passenger in passengers {
            if let pumpkin_nbt::tag::NbtTag::Compound(passenger) = passenger {
                load_tree(world, passenger, Some(&entity));
            }
        }
    }
}

/// Chunk storage writes vehicle roots with their saved passenger trees.
pub(super) fn saved_tree(entity: &Arc<dyn EntityBase>) -> NbtCompound {
    fn write(entity: &Arc<dyn EntityBase>, seen: &mut FxHashSet<Uuid>) -> Option<NbtCompound> {
        let base = entity.get_entity();
        if base.is_removed() || entity.get_player().is_some() || !seen.insert(base.entity_uuid) {
            return None;
        }
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt);
        let passengers = base
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let passengers: Vec<_> = passengers
            .iter()
            .filter_map(|passenger| write(passenger, seen))
            .map(pumpkin_nbt::tag::NbtTag::Compound)
            .collect();
        if !passengers.is_empty() {
            nbt.put("Passengers", pumpkin_nbt::tag::NbtTag::List(passengers));
        }
        Some(nbt)
    }
    write(entity, &mut FxHashSet::default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::registry::BlockRegistry;
    use arc_swap::ArcSwap;
    use pumpkin_data::{Block, dimension::Dimension};
    use pumpkin_nbt::tag::NbtTag;
    use pumpkin_util::{math::position::BlockPos, world_seed::Seed};
    use pumpkin_world::{
        chunk::{ChunkData, io::Dirtiable},
        level::Level,
        tick::TickPriority,
        world_info::LevelData,
    };

    fn saved_entity(kind: &str, id: u128) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", format!("minecraft:{kind}"));
        nbt.put_uuid("UUID", Uuid::from_u128(id));
        nbt.put(
            "Pos",
            NbtTag::List(vec![0.0.into(), 64.0.into(), 0.0.into()]),
        );
        nbt.put(
            "Motion",
            NbtTag::List(vec![0.25.into(), (-0.1).into(), 0.5.into()]),
        );
        nbt
    }

    fn empty_world(path: &std::path::Path) -> Arc<World> {
        let level = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            path.to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        for x in 0..=3 {
            level
                .loaded_chunks
                .insert(Vector2::new(x, 0), ChunkData::empty_sync(x, 0));
        }
        World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
            Dimension::OVERWORLD,
            Arc::new(BlockRegistry::default()),
            Weak::new(),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn periodic_autosave_writes_live_residents_before_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let world = empty_world(dir.path());
        let pos = Vector2::new(0, 0);
        world.forced_chunks.lock().unwrap().insert(pos);
        world.update_active_chunks();
        world.level.try_load_entity_chunk(pos).await.unwrap();
        let entity = from_type(
            &EntityType::TNT,
            Vector3::new(0.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(7),
        );
        let mut state = NbtCompound::new();
        state.put_short("fuse", 42);
        entity.read_custom_nbt(&state);
        world.add_entity_silent(entity);
        world.level_time.lock().unwrap().world_age = world.level.autosave_ticks as i64 - 1;
        world.tick_environment();
        assert_eq!(world.level.queue_entity_chunks(Vec::new()).await, Ok(true));
        // Open a fresh storage reader before World::shutdown can write anything.
        let reader = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        let stored = reader.try_load_entity_chunk(pos).await.unwrap();
        assert_eq!(stored.data.lock().unwrap().len(), 1);
        assert_eq!(
            stored.data.lock().unwrap()[0].get_uuid("UUID"),
            Some(Uuid::from_u128(7))
        );
        assert_eq!(stored.data.lock().unwrap()[0].get_short("fuse"), Some(42));
        reader.shutdown().await;
        world.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn snapshots_remove_moved_and_deleted_roots_without_erasing_unactivated_residents() {
        let dir = tempfile::tempdir().unwrap();
        let world = empty_world(dir.path());
        let a = world
            .level
            .try_load_entity_chunk(Vector2::new(0, 0))
            .await
            .unwrap();
        let b = world
            .level
            .try_load_entity_chunk(Vector2::new(1, 0))
            .await
            .unwrap();
        a.data.lock().unwrap().push(saved_entity("tnt", 99));
        let entity = from_type(
            &EntityType::TNT,
            Vector3::new(0.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(1),
        );
        world.add_entity_silent(entity.clone());
        world.snapshot_entity_chunks();
        assert_eq!(a.data.lock().unwrap().len(), 2);
        entity.get_entity().set_pos(Vector3::new(16.0, 64.0, 0.0));
        world.snapshot_entity_chunks();
        assert_eq!(a.data.lock().unwrap().len(), 1);
        assert_eq!(
            a.data.lock().unwrap()[0].get_uuid("UUID"),
            Some(Uuid::from_u128(99))
        );
        assert_eq!(b.data.lock().unwrap().len(), 1);
        world.remove_entity(entity.as_ref());
        world.snapshot_entity_chunks();
        assert!(b.data.lock().unwrap().is_empty());
        world.shutdown().await;
        let reloaded = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        assert_eq!(
            reloaded
                .try_load_entity_chunk(Vector2::new(0, 0))
                .await
                .unwrap()
                .data
                .lock()
                .unwrap()
                .len(),
            1
        );
        assert!(
            reloaded
                .try_load_entity_chunk(Vector2::new(1, 0))
                .await
                .unwrap()
                .data
                .lock()
                .unwrap()
                .is_empty()
        );
        reloaded.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unload_uses_vehicle_roots_across_chunk_boundaries_and_preserves_saved_tree() {
        let dir = tempfile::tempdir().unwrap();
        let world = empty_world(dir.path());
        let a = Vector2::new(0, 0);
        let b = Vector2::new(2, 0);
        for pos in [a, b] {
            world.level.try_load_entity_chunk(pos).await.unwrap();
        }
        let root = from_type(
            &EntityType::TNT,
            Vector3::new(0.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(1),
        );
        let rider = from_type(
            &EntityType::FALLING_BLOCK,
            Vector3::new(32.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(2),
        );
        let nested = from_type(
            &EntityType::ARROW,
            Vector3::new(33.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(3),
        );
        for entity in [&root, &rider, &nested] {
            world.add_entity_silent(entity.clone());
        }
        root.get_entity().add_passenger(root.clone(), rider.clone());
        rider
            .get_entity()
            .add_passenger(rider.clone(), nested.clone());
        world.unload_entity_chunks([b]);
        assert_eq!(world.entities.load().len(), 3);
        root.get_entity()
            .teleporting
            .store(true, std::sync::atomic::Ordering::Release);
        world.unload_entity_chunks([a]);
        assert_eq!(world.entities.load().len(), 3);
        root.get_entity()
            .teleporting
            .store(false, std::sync::atomic::Ordering::Release);
        world.unload_entity_chunks([a]);
        assert!(world.entities.load().is_empty());
        for entity in [&root, &rider, &nested] {
            let base = entity.get_entity();
            assert!(
                base.removal_reason.load() == Some(crate::entity::RemovalReason::UnloadedToChunk)
            );
            assert!(!base.has_vehicle());
            assert!(!base.has_passengers());
        }
        world.shutdown().await;
        let reloaded = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        let storage = reloaded.try_load_entity_chunk(a).await.unwrap();
        let saved = storage.data.lock().unwrap().clone();
        assert_eq!(saved.len(), 1);
        let NbtTag::Compound(rider) = &saved[0].get_list("Passengers").unwrap()[0] else {
            panic!("rider")
        };
        assert_eq!(rider.get_uuid("UUID"), Some(Uuid::from_u128(2)));
        let NbtTag::Compound(nested) = &rider.get_list("Passengers").unwrap()[0] else {
            panic!("nested")
        };
        assert_eq!(nested.get_uuid("UUID"), Some(Uuid::from_u128(3)));
        assert!(
            reloaded
                .try_load_entity_chunk(b)
                .await
                .unwrap()
                .data
                .lock()
                .unwrap()
                .is_empty()
        );
        reloaded.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ticket_only_restart_activates_saved_trees_once_before_scheduled_work() {
        let dir = tempfile::tempdir().unwrap();
        let config = pumpkin_config::world::LevelConfig::default();
        let center = Vector2::new(0, 0);
        let writer =
            Level::from_root_folder(&config, dir.path().to_path_buf(), 262, Dimension::OVERWORLD);
        let storage = writer.try_load_entity_chunk(center).await.unwrap();
        let mut root = saved_entity("tnt", 1);
        root.put_short("fuse", 37);
        let mut rider = saved_entity("falling_block", 2);
        let mut nested = saved_entity("arrow", 3);
        nested.put_uuid("Owner", Uuid::from_u128(1));
        rider.put("Passengers", NbtTag::List(vec![NbtTag::Compound(nested)]));
        root.put("Passengers", NbtTag::List(vec![NbtTag::Compound(rider)]));
        storage.data.lock().unwrap().push(root);
        storage.mark_dirty(true);
        let (tickets, _) =
            super::super::portal::tickets::PortalTickets::read(&NbtCompound::new(), 0);
        tickets
            .save(
                &writer.level_folder.dim_folder,
                0,
                &FxHashSet::from_iter([center]),
            )
            .unwrap();
        writer.shutdown().await;
        drop(storage);
        drop(writer);

        let level =
            Level::from_root_folder(&config, dir.path().to_path_buf(), 262, Dimension::OVERWORLD);
        level
            .loaded_chunks
            .insert(center, ChunkData::empty_sync(0, 0));
        let world = World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
            Dimension::OVERWORLD,
            Arc::new(BlockRegistry::default()),
            Weak::new(),
        );
        let pos = BlockPos::new(0, 64, 0);
        world
            .level
            .schedule_block_tick(&Block::STONE, pos, 0, TickPriority::Normal);
        assert!(!world.are_entities_loaded(&center));
        assert!(
            world
                .level
                .get_scheduled_block_ticks_if(|pos| world.are_entities_loaded(pos))
                .is_empty()
        );
        world.update_active_chunks();
        assert!(world.players.load().is_empty());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !world.are_entities_loaded(&center) {
                tokio::task::yield_now().await;
                world.update_active_chunks();
            }
        })
        .await
        .unwrap();
        assert_eq!(world.entities.load().len(), 3);
        assert_eq!(
            world
                .level
                .get_scheduled_block_ticks_if(|pos| world.are_entities_loaded(pos))
                .len(),
            1
        );
        let root = world.get_entity_by_uuid(Uuid::from_u128(1)).unwrap();
        let rider = world.get_entity_by_uuid(Uuid::from_u128(2)).unwrap();
        let arrow = world.get_entity_by_uuid(Uuid::from_u128(3)).unwrap();
        assert_eq!(
            rider
                .get_entity()
                .get_vehicle()
                .unwrap()
                .get_entity()
                .entity_uuid,
            Uuid::from_u128(1)
        );
        assert_eq!(
            arrow
                .get_entity()
                .get_vehicle()
                .unwrap()
                .get_entity()
                .entity_uuid,
            Uuid::from_u128(2)
        );
        assert_eq!(
            arrow
                .get_entity()
                .resolve_projectile_owner()
                .unwrap()
                .get_entity()
                .entity_uuid,
            Uuid::from_u128(1)
        );
        assert_eq!(
            root.get_entity().velocity.load(),
            Vector3::new(0.25, -0.1, 0.5)
        );
        assert_eq!(saved_tree(&root).get_short("fuse"), Some(37));
        world.request_entity_chunk(center);
        world.update_active_chunks();
        assert_eq!(world.entities.load().len(), 3);
        // Two snapshots must not append duplicate roots or save riders as roots.
        for _ in 0..2 {
            world.save_entity_snapshots().await;
        }
        let storage = world.level.get_entity_chunk_sync(&center).unwrap();
        let saved = storage.data.lock().unwrap().clone();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].get_list("Passengers").unwrap().len(), 1);
        world.level.shutdown().await;
    }
}

impl World {
    /// Rebuild activated storage from current live roots, including empty chunks.
    /// Preserve records in storage whose residents have not been activated yet.
    pub(crate) fn snapshot_entity_chunks(&self) -> Vec<(Vector2<i32>, SyncEntityChunk)> {
        use pumpkin_world::chunk::io::Dirtiable;
        let entities = self.entities.load();
        let live_ids: FxHashSet<_> = entities
            .iter()
            .map(|e| e.get_entity().entity_uuid)
            .collect();
        let mut roots: FxHashMap<Vector2<i32>, Vec<NbtCompound>> = FxHashMap::default();
        for entity in entities.iter() {
            let base = entity.get_entity();
            if !base.is_removed() && !base.has_vehicle() {
                roots
                    .entry(base.chunk_pos.load())
                    .or_default()
                    .push(saved_tree(entity));
            }
        }
        let cached = self.level.cached_entity_chunks();
        let mut owned = self
            .entity_chunks
            .snapshot_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pos, chunk) in &cached {
            let previous = owned.remove(pos).unwrap_or_default();
            let mut data = chunk
                .data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut next = if self.are_entities_loaded(pos) {
                Vec::new()
            } else {
                data.iter()
                    .filter(|nbt| {
                        nbt.get_uuid("UUID").is_none_or(|uuid| {
                            !live_ids.contains(&uuid) && !previous.contains(&uuid)
                        })
                    })
                    .cloned()
                    .collect()
            };
            let current = roots.remove(pos).unwrap_or_default();
            let ids: FxHashSet<_> = current
                .iter()
                .filter_map(|nbt| nbt.get_uuid("UUID"))
                .collect();
            if !ids.is_empty() {
                owned.insert(*pos, ids);
            }
            next.extend(current);
            if *data != next {
                *data = next;
                chunk.mark_dirty(true);
            }
        }
        // A spawned root can reach a chunk before its storage read finishes.
        // Request it now; a later snapshot includes it without erasing saved residents.
        self.entity_chunks
            .pending_snapshot
            .store(!roots.is_empty(), std::sync::atomic::Ordering::Release);
        for pos in roots.keys() {
            self.request_entity_chunk(*pos);
        }
        cached
    }

    pub(crate) fn retry_pending_entity_snapshot(&self) {
        use std::sync::atomic::Ordering;
        if !self.entity_chunks.pending_snapshot.load(Ordering::Acquire) {
            return;
        }
        let missing: Vec<_> = self
            .entities
            .load()
            .iter()
            .filter_map(|entity| {
                let base = entity.get_entity();
                let pos = base.chunk_pos.load();
                (!base.is_removed()
                    && !base.has_vehicle()
                    && self.level.get_entity_chunk_sync(&pos).is_none())
                .then_some(pos)
            })
            .collect();
        if !missing.is_empty() {
            for pos in missing {
                self.request_entity_chunk(pos);
            }
            return;
        }
        let handle = self
            .server
            .upgrade()
            .map(|s| s.runtime.clone())
            .or_else(|| tokio::runtime::Handle::try_current().ok());
        if let Some(handle) = handle {
            let _guard = handle.enter();
            let snapshots = self.snapshot_entity_chunks();
            let _written = self.level.queue_entity_chunks(snapshots);
        }
    }

    pub(crate) async fn save_entity_snapshots(&self) {
        let positions: FxHashSet<_> = self
            .entities
            .load()
            .iter()
            .filter(|entity| {
                !entity.get_entity().is_removed() && !entity.get_entity().has_vehicle()
            })
            .map(|entity| entity.get_entity().chunk_pos.load())
            .collect();
        for pos in positions {
            if let Err(error) = self.level.try_load_entity_chunk(pos).await {
                tracing::error!("Cannot snapshot unreadable entity storage {pos:?}: {error}");
            }
        }
        let chunks = self.snapshot_entity_chunks();
        self.level.write_entity_chunks(chunks).await;
    }

    pub(crate) fn unload_entity_chunks(&self, positions: impl IntoIterator<Item = Vector2<i32>>) {
        use crate::entity::RemovalReason;
        let chunks: FxHashSet<_> = positions
            .into_iter()
            .filter(|pos| !self.level.should_retain_entity_chunk(pos))
            .collect();
        if chunks.is_empty() {
            return;
        }
        // Capture and remove in the same synchronous phase; no I/O await may
        // separate selection, serialization and removal of a vehicle tree.
        let roots: Vec<_> = self
            .entities
            .load()
            .iter()
            .filter(|entity| {
                let base = entity.get_entity();
                !base.is_removed() && !base.has_vehicle() && chunks.contains(&base.chunk_pos.load())
            })
            .cloned()
            .collect();
        let mut deferred = FxHashSet::default();
        let mut groups = Vec::new();
        let mut seen = FxHashSet::default();
        for root in roots {
            let pos = root.get_entity().chunk_pos.load();
            if self.level.get_entity_chunk_sync(&pos).is_none() {
                self.request_entity_chunk(pos);
                deferred.insert(pos);
                continue;
            }
            let mut group = Vec::new();
            let mut stack = vec![root];
            while let Some(entity) = stack.pop() {
                if !seen.insert(entity.get_entity().entity_uuid) {
                    continue;
                }
                // Player-owned vehicle persistence has its own lifecycle.
                if entity.get_player().is_some()
                    || entity
                        .get_entity()
                        .teleporting
                        .load(std::sync::atomic::Ordering::Acquire)
                {
                    deferred.insert(pos);
                    continue;
                }
                stack.extend(
                    entity
                        .get_entity()
                        .passengers
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .iter()
                        .cloned(),
                );
                group.push(entity);
            }
            groups.push((pos, group));
        }
        let removing: Vec<_> = groups
            .into_iter()
            .filter(|(pos, _)| !deferred.contains(pos))
            .flat_map(|(_, group)| group)
            .collect();
        self.snapshot_entity_chunks();
        for entity in &removing {
            self.remove_entity_with_reason(entity.as_ref(), RemovalReason::UnloadedToChunk);
        }
        // Strong parent/child references must not keep unloaded trees alive.
        for entity in &removing {
            let base = entity.get_entity();
            base.vehicle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            base.passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        }
        let completed: Vec<_> = chunks
            .into_iter()
            .filter(|pos| !deferred.contains(pos))
            .collect();
        for pos in &completed {
            self.entity_chunks
                .snapshot_roots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(pos);
            self.forget_entity_chunk(pos);
            self.save_block_entities(*pos);
            if let Some((_, entities)) = self.block_entities.remove(pos) {
                for entity in entities.into_values() {
                    entity.set_removed();
                }
            }
        }
        self.level.clean_entity_chunks(completed);
    }
}
