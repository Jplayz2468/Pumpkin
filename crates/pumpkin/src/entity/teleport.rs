//! Shared entity replacement and passenger-tree transfer.
use super::{Entity, EntityBase, RemovalReason, teleport_state::TeleportState};
use crate::world::portal::TeleportTransition;
use futures::FutureExt;
use pumpkin_nbt::compound::NbtCompound;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, atomic::Ordering},
};

struct TransferGuard(Arc<dyn EntityBase>);
impl Drop for TransferGuard {
    fn drop(&mut self) {
        self.0
            .get_entity()
            .teleporting
            .store(false, Ordering::Release);
    }
}

pub fn request(entity: &Entity, transition: TeleportTransition) {
    let world = entity.world.load_full();
    let Some(source) = world.get_entity_by_id(entity.entity_id) else {
        return;
    };
    // Loaded destinations complete synchronously, preserving command/block
    // ordering. Only actual chunk/client I/O continues on the runtime.
    let mut pending = transfer(source, transition);
    if pending.as_mut().now_or_never().is_none()
        && let Some(server) = world.server.upgrade()
    {
        server.runtime.spawn(pending);
    }
}

/// The position-only teleport used by block shape changes is not a dimension
/// transition: it keeps riding relationships and does not set TNT's portal flag.
pub fn move_relative(entity: &dyn EntityBase, movement: pumpkin_util::math::vector3::Vector3<f64>) {
    let base = entity.get_entity();
    let position = base.pos.load() + movement;
    if let Some(player) = entity.get_player() {
        player.request_teleport(position, base.yaw.load(), base.pitch.load());
    } else {
        base.teleport(position, None, None, &base.world.load());
    }
    let passengers = base
        .passengers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    for passenger in passengers {
        move_relative(passenger.as_ref(), movement);
    }
}

pub fn transfer(
    source: Arc<dyn EntityBase>,
    transition: TeleportTransition,
) -> Pin<Box<dyn Future<Output = Option<Arc<dyn EntityBase>>> + Send>> {
    Box::pin(async move {
        let old = source.get_entity();
        if old.is_removed() || old.teleporting.swap(true, Ordering::AcqRel) {
            return None;
        }
        let _guard = TransferGuard(source.clone());
        let old_world = old.world.load_full();
        let new_world = &transition.new_world;
        let crosses_worlds = !Arc::ptr_eq(&old_world, new_world);
        if crosses_worlds
            && source.get_player().is_none()
            && new_world.get_entity_by_uuid(old.entity_uuid).is_some()
        {
            return None;
        }
        let previous = TeleportState::of(old);
        let absolute = transition.state.absolute(previous, transition.relatives);

        if !transition.as_passenger
            && let Some(vehicle) = old.get_vehicle()
        {
            vehicle
                .get_entity()
                .remove_passenger_before_teleport(old.entity_id);
            if old.has_vehicle() {
                return None;
            }
        }

        // Load before detaching anyone. A failed/cancelled player transition may
        // leave that passenger behind; it must never remain mounted across worlds.
        let chunk = pumpkin_util::math::vector2::Vector2::new(
            (absolute.position.x.floor() as i32) >> 4,
            (absolute.position.z.floor() as i32) >> 4,
        );
        if transition.portal {
            new_world.place_portal_ticket(chunk);
        }
        new_world.level.get_or_fetch_chunk(chunk, |_| ()).await;
        if old.is_removed() || !Arc::ptr_eq(&old.world.load_full(), &old_world) {
            return None;
        }
        let passengers = old
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if crosses_worlds {
            for passenger in &passengers {
                old.remove_passenger_on_disconnect(passenger.get_entity().entity_id);
            }
        }
        let mut arrived = Vec::new();
        for passenger in passengers {
            let mut child = transition.clone();
            child.state = transition.state.passenger(
                previous,
                TeleportState::of(passenger.get_entity()),
                transition.relatives,
            );
            child.as_passenger = true;
            if let Some(passenger) = transfer(passenger, child).await {
                arrived.push(passenger);
            }
        }

        let result: Arc<dyn EntityBase> = if source.get_player().is_some() {
            let player = old_world.get_player_by_uuid(old.entity_uuid)?;
            if crosses_worlds {
                player
                    .teleport_world_with_movement(
                        new_world.clone(),
                        absolute.position,
                        Some(absolute.yaw),
                        Some(absolute.pitch),
                        Some(absolute.velocity),
                    )
                    .await;
                if !Arc::ptr_eq(&player.world(), new_world) {
                    return None;
                }
            } else {
                player.request_teleport_with_movement(
                    absolute.position,
                    absolute.yaw,
                    absolute.pitch,
                    Some(absolute.velocity),
                );
            }
            if matches!(
                player.client.as_ref(),
                crate::net::ClientPlatform::Bedrock(_)
            ) {
                player.get_entity().send_velocity();
            }
            player
        } else if crosses_worlds {
            let replacement = super::r#type::from_type(
                old.entity_type,
                absolute.position,
                new_world,
                old.entity_uuid,
            );
            let mut nbt = NbtCompound::new();
            source.write_nbt(&mut nbt);
            replacement.read_nbt_non_mut(&nbt);
            replacement.restore_transient_state(source.as_ref());
            // ChangedDimension invalidates owner caches and stale tick snapshots.
            if !old_world
                .remove_entity_with_reason(source.as_ref(), RemovalReason::ChangedDimension)
            {
                return None;
            }
            replacement.get_entity().apply_teleport_state(absolute);
            replacement.init_data_tracker();
            new_world.add_entity_silent(replacement.clone());
            replacement
        } else {
            old.apply_teleport_state(absolute);
            old.teleport(
                absolute.position,
                Some(absolute.yaw),
                Some(absolute.pitch),
                new_world,
            );
            old_world
                .entity_tracker
                .update_entity_position(source.as_ref(), &old_world);
            source.clone()
        };
        if crosses_worlds {
            for passenger in arrived {
                if Arc::ptr_eq(&passenger.get_entity().world.load_full(), new_world) {
                    result.get_entity().add_passenger(result.clone(), passenger);
                }
            }
        }
        result.after_teleport();
        if transition.portal
            && let Some(player) = result.get_player()
        {
            player.try_send_client_packet(&pumpkin_protocol::java::client::play::CWorldEvent::new(
                pumpkin_data::world::WorldEvent::SoundPortalTravel as i32,
                pumpkin_util::math::position::BlockPos::new(0, 0, 0),
                0,
                false,
            ));
        }
        Some(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::registry::BlockRegistry, world::World};
    use arc_swap::ArcSwap;
    use pumpkin_data::{dimension::Dimension, entity::EntityType};
    use pumpkin_util::{
        math::{vector2::Vector2, vector3::Vector3},
        world_seed::Seed,
    };
    use pumpkin_world::{chunk::ChunkData, level::Level, world_info::LevelData};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transfers_real_world_membership_nested_riders_and_owner_references() {
        let directory = tempfile::tempdir().unwrap();
        let registry = Arc::new(BlockRegistry::default());
        let info = Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262))));
        let make_world = |dimension: Dimension| {
            let level = Level::from_root_folder(
                &pumpkin_config::world::LevelConfig::default(),
                directory.path().to_path_buf(),
                262,
                dimension.clone(),
            );
            for x in -1..=3 {
                for z in -1..=1 {
                    level
                        .loaded_chunks
                        .insert(Vector2::new(x, z), ChunkData::empty_sync(x, z));
                }
            }
            World::load(
                level,
                info.clone(),
                dimension,
                registry.clone(),
                std::sync::Weak::new(),
            )
        };
        let old = make_world(Dimension::OVERWORLD);
        let destination = make_world(Dimension::THE_NETHER);
        let root = super::super::r#type::from_type(
            &EntityType::TNT,
            Vector3::new(0.0, 64.0, 0.0),
            &old,
            uuid::Uuid::new_v4(),
        );
        let rider = super::super::r#type::from_type(
            &EntityType::FALLING_BLOCK,
            Vector3::new(0.5, 65.0, 0.25),
            &old,
            uuid::Uuid::new_v4(),
        );
        let nested = super::super::r#type::from_type(
            &EntityType::ARROW,
            Vector3::new(0.75, 66.0, 0.5),
            &old,
            uuid::Uuid::new_v4(),
        );
        let mut tnt = NbtCompound::new();
        tnt.put_short("fuse", 37);
        tnt.put_float("explosion_power", 7.0);
        root.read_custom_nbt(&tnt);
        root.get_entity()
            .velocity
            .store(Vector3::new(0.4, -0.1, 0.2));
        for entity in [&root, &rider, &nested] {
            old.add_entity_silent(entity.clone());
        }
        nested
            .get_entity()
            .set_projectile_owner(Some(root.get_entity()));
        root.get_entity().add_passenger(root.clone(), rider.clone());
        rider
            .get_entity()
            .add_passenger(rider.clone(), nested.clone());
        let root_uuid = root.get_entity().entity_uuid;
        let new_root = transfer(
            root.clone(),
            TeleportTransition::from_target(
                destination.clone(),
                Vector3::new(32.0, 70.0, 0.0),
                Some(90.0),
                Some(0.0),
            ),
        )
        .await
        .unwrap();
        assert!(old.entities.load().is_empty());
        assert_eq!(destination.entities.load().len(), 3);
        assert!(root.get_entity().removal_reason.load() == Some(RemovalReason::ChangedDimension));
        assert_ne!(root.get_entity().entity_id, new_root.get_entity().entity_id);
        assert_eq!(new_root.get_entity().entity_uuid, root_uuid);
        assert!(!new_root.get_entity().is_removed());
        let new_rider = destination
            .get_entity_by_uuid(rider.get_entity().entity_uuid)
            .unwrap();
        let new_nested = destination
            .get_entity_by_uuid(nested.get_entity().entity_uuid)
            .unwrap();
        assert_eq!(
            new_rider.get_entity().pos.load(),
            Vector3::new(32.5, 71.0, 0.25)
        );
        assert_eq!(
            new_nested.get_entity().pos.load(),
            Vector3::new(32.75, 72.0, 0.5)
        );
        assert_eq!(
            new_rider
                .get_entity()
                .get_vehicle()
                .unwrap()
                .get_entity()
                .entity_id,
            new_root.get_entity().entity_id
        );
        assert_eq!(
            new_nested
                .get_entity()
                .get_vehicle()
                .unwrap()
                .get_entity()
                .entity_id,
            new_rider.get_entity().entity_id
        );
        assert_eq!(
            new_nested
                .get_projectile_owner()
                .unwrap()
                .get_entity()
                .entity_id,
            new_root.get_entity().entity_id
        );
        let mut saved = NbtCompound::new();
        new_root.write_custom_nbt(&mut saved);
        assert_eq!(saved.get_short("fuse"), Some(37));
        assert_eq!(saved.get_float("explosion_power"), Some(7.0));
        let velocity = new_root.get_entity().velocity.load();
        assert!((velocity.x + 0.2).abs() < 0.001 && (velocity.z - 0.4).abs() < 0.001);
        assert_eq!(velocity.y, -0.1);
        // The public request path must complete immediately with loaded chunks,
        // even without a server scheduler (as with these isolated real worlds).
        new_root.teleport(
            Vector3::new(33.0, 70.0, 0.0),
            None,
            None,
            destination.clone(),
        );
        assert_eq!(
            new_root.get_entity().pos.load(),
            Vector3::new(33.0, 70.0, 0.0)
        );
        assert_eq!(destination.entities.load().len(), 3);
        assert_eq!(
            new_nested.get_entity().pos.load(),
            Vector3::new(33.75, 72.0, 0.5)
        );
        move_relative(new_root.as_ref(), Vector3::new(0.0, 0.5, 0.0));
        assert_eq!(
            new_nested.get_entity().pos.load(),
            Vector3::new(33.75, 72.5, 0.5)
        );
        assert!(new_rider.get_entity().has_vehicle() && new_nested.get_entity().has_vehicle());
        old.level.shutdown().await;
        destination.level.shutdown().await;
    }
}
