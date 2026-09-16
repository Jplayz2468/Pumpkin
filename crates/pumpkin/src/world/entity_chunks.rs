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
            for entity in world.entities.load().iter() {
                world.save_entity(entity).await;
            }
        }
        let storage = world.level.get_entity_chunk_sync(&center).unwrap();
        let saved = storage.data.lock().unwrap().clone();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].get_list("Passengers").unwrap().len(), 1);
        world.level.shutdown().await;
    }
}
