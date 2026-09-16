//! Player-owned vehicle trees are stored in RootVehicle, not entity-region roots.
use super::{Entity, EntityBase, RemovalReason};
use crate::world::World;
use pumpkin_data::entity::EntityType;
use pumpkin_nbt::compound::NbtCompound;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) fn tree(root: &Arc<dyn EntityBase>) -> Vec<Arc<dyn EntityBase>> {
    let mut stack = vec![root.clone()];
    let mut result = Vec::new();
    let mut seen = FxHashSet::default();
    while let Some(entity) = stack.pop() {
        if !seen.insert(entity.get_entity().entity_uuid) {
            continue;
        }
        let passengers = entity
            .get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        stack.extend(passengers.into_iter().rev());
        result.push(entity);
    }
    result
}

pub(crate) fn root_vehicle(entity: &Entity) -> Option<Arc<dyn EntityBase>> {
    let mut root = entity.get_vehicle()?;
    let mut seen = FxHashSet::from_iter([entity.entity_uuid]);
    loop {
        if !seen.insert(root.get_entity().entity_uuid) {
            return None;
        }
        let Some(parent) = root.get_entity().get_vehicle() else {
            return Some(root);
        };
        root = parent;
    }
}

pub(crate) fn player_count(root: &Arc<dyn EntityBase>) -> usize {
    tree(root)
        .iter()
        .skip(1)
        .filter(|entity| entity.get_entity().entity_type == &EntityType::PLAYER)
        .count()
}

pub(crate) fn chunk_root(entity: &Arc<dyn EntityBase>) -> bool {
    let base = entity.get_entity();
    !base.is_removed()
        && !base.has_vehicle()
        && base.entity_type != &EntityType::PLAYER
        && player_count(entity) != 1
}

pub(crate) fn saved_root(entity: &Entity) -> Option<NbtCompound> {
    let immediate = entity.get_vehicle()?;
    let root = root_vehicle(entity)?;
    if player_count(&root) != 1 {
        return None;
    }
    let mut wrapper = NbtCompound::new();
    wrapper.put_uuid("Attach", immediate.get_entity().entity_uuid);
    wrapper.put_compound("Entity", crate::world::entity_chunks::saved_tree(&root));
    Some(wrapper)
}

pub(crate) fn remove_tree(world: &World, root: &Arc<dyn EntityBase>, reason: RemovalReason) {
    let entities = tree(root);
    for entity in &entities {
        if entity.get_entity().entity_type != &EntityType::PLAYER {
            world.remove_entity_with_reason(entity.as_ref(), reason);
        }
    }
    for entity in entities {
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
}

/// Return the newly loaded root and its attachment target. A duplicate root UUID
/// returns None; failed attachment cleans up only this newly created tree.
pub(crate) fn load_root(
    world: &Arc<World>,
    wrapper: &NbtCompound,
    attach: Option<Uuid>,
) -> Option<(Arc<dyn EntityBase>, Option<Arc<dyn EntityBase>>)> {
    let nbt = wrapper.get_compound("Entity")?;
    let root = world.load_saved_entity_tree(nbt)?;
    let target = tree(&root)
        .into_iter()
        .find(|entity| Some(entity.get_entity().entity_uuid) == attach);
    Some((root, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::registry::BlockRegistry, entity::r#type::from_type};
    use arc_swap::ArcSwap;
    use pumpkin_data::dimension::Dimension;
    use pumpkin_nbt::tag::NbtTag;
    use pumpkin_util::{
        math::{vector2::Vector2, vector3::Vector3},
        world_seed::Seed,
    };
    use pumpkin_world::{chunk::ChunkData, level::Level, world_info::LevelData};
    use std::sync::Weak;

    fn world(path: &std::path::Path) -> Arc<World> {
        let level = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig::default(),
            path.to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        level
            .loaded_chunks
            .insert(Vector2::new(0, 0), ChunkData::empty_sync(0, 0));
        World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
            Dimension::OVERWORLD,
            Arc::new(BlockRegistry::default()),
            Weak::new(),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn player_owned_tree_has_embedded_payload_and_shared_vehicle_uses_chunk_storage() {
        let dir = tempfile::tempdir().unwrap();
        let world = world(dir.path());
        let root = from_type(
            &EntityType::TNT,
            Vector3::new(0.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(1),
        );
        let rider = from_type(
            &EntityType::FALLING_BLOCK,
            Vector3::new(0.0, 65.0, 0.0),
            &world,
            Uuid::from_u128(2),
        );
        // Player-type markers exercise tree policy without creating network clients.
        let player = from_type(
            &EntityType::PLAYER,
            Vector3::new(0.0, 66.0, 0.0),
            &world,
            Uuid::from_u128(3),
        );
        let second = from_type(
            &EntityType::PLAYER,
            Vector3::new(0.0, 66.0, 0.0),
            &world,
            Uuid::from_u128(4),
        );
        for entity in [&root, &rider] {
            world.add_entity_silent(entity.clone());
        }
        root.get_entity().add_passenger(root.clone(), rider.clone());
        rider
            .get_entity()
            .add_passenger(rider.clone(), player.clone());
        assert_eq!(player_count(&root), 1);
        assert!(!chunk_root(&root));
        let wrapper = saved_root(player.get_entity()).unwrap();
        assert_eq!(wrapper.get_uuid("Attach"), Some(Uuid::from_u128(2)));
        let root_nbt = wrapper.get_compound("Entity").unwrap();
        assert_eq!(root_nbt.get_uuid("UUID"), Some(Uuid::from_u128(1)));
        let NbtTag::Compound(rider_nbt) = &root_nbt.get_list("Passengers").unwrap()[0] else {
            panic!("rider")
        };
        assert!(rider_nbt.get_list("Passengers").is_none());
        let storage = world
            .level
            .try_load_entity_chunk(Vector2::new(0, 0))
            .await
            .unwrap();
        world.snapshot_entity_chunks();
        assert!(storage.data.lock().unwrap().is_empty());
        root.get_entity()
            .add_passenger(root.clone(), second.clone());
        assert_eq!(player_count(&root), 2);
        assert!(chunk_root(&root));
        assert!(saved_root(player.get_entity()).is_none());
        world.snapshot_entity_chunks();
        assert_eq!(storage.data.lock().unwrap().len(), 1);
        root.get_entity()
            .remove_passenger_on_disconnect(second.get_entity().entity_id);
        rider
            .get_entity()
            .remove_passenger_on_disconnect(player.get_entity().entity_id);
        remove_tree(&world, &root, RemovalReason::UnloadedWithPlayer);
        for entity in [&root, &rider] {
            assert!(
                entity.get_entity().removal_reason.load()
                    == Some(RemovalReason::UnloadedWithPlayer)
            );
            assert!(!entity.get_entity().has_passengers());
            assert!(!entity.get_entity().has_vehicle());
        }
        world.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn embedded_restore_resolves_nested_attachment_and_does_not_delete_uuid_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let world = world(dir.path());
        let make = |kind: &str, id| {
            let mut nbt = NbtCompound::new();
            nbt.put_string("id", format!("minecraft:{kind}"));
            nbt.put_uuid("UUID", Uuid::from_u128(id));
            nbt.put(
                "Pos",
                NbtTag::List(vec![0.0.into(), 64.0.into(), 0.0.into()]),
            );
            nbt
        };
        let mut root_nbt = make("tnt", 1);
        root_nbt.put_short("fuse", 31);
        root_nbt.put(
            "Passengers",
            NbtTag::List(vec![NbtTag::Compound(make("falling_block", 2))]),
        );
        let mut wrapper = NbtCompound::new();
        wrapper.put_compound("Entity", root_nbt);
        let (root, target) = load_root(&world, &wrapper, Some(Uuid::from_u128(2))).unwrap();
        assert_eq!(world.entities.load().len(), 2);
        assert_eq!(
            target
                .unwrap()
                .get_entity()
                .get_vehicle()
                .unwrap()
                .get_entity()
                .entity_uuid,
            Uuid::from_u128(1)
        );
        assert!(load_root(&world, &wrapper, Some(Uuid::from_u128(2))).is_none());
        remove_tree(&world, &root, RemovalReason::Discarded);
        let foreign = from_type(
            &EntityType::ARROW,
            Vector3::new(0.0, 64.0, 0.0),
            &world,
            Uuid::from_u128(2),
        );
        world.add_entity_silent(foreign.clone());
        let (root, target) = load_root(&world, &wrapper, Some(Uuid::from_u128(2))).unwrap();
        assert!(target.is_none());
        remove_tree(&world, &root, RemovalReason::Discarded);
        assert_eq!(world.entities.load().len(), 1);
        assert!(!foreign.get_entity().is_removed());
        assert!(Arc::ptr_eq(
            &world.get_entity_by_uuid(Uuid::from_u128(2)).unwrap(),
            &foreign
        ));
        world.shutdown().await;
    }
}
