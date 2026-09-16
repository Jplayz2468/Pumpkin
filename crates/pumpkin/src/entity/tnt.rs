use super::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use pumpkin_data::Block;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::vector3::Vector3;
use std::sync::atomic::Ordering;

pub struct TNTEntity {
    entity: Entity,
    state: std::sync::Mutex<super::tnt_state::TntState>,
    owner: std::sync::Mutex<Option<super::entity_reference::EntityOwner>>,
    used_portal: std::sync::atomic::AtomicBool,
}

impl TNTEntity {
    /// Unprimed construction is also used by loading and /summon: no launch impulse.
    pub fn new(entity: Entity, power: f32, fuse: i32) -> Self {
        Self {
            entity,
            state: std::sync::Mutex::new(super::tnt_state::TntState {
                fuse,
                power,
                block: Block::TNT.default_state.id,
            }),
            owner: std::sync::Mutex::default(),
            used_portal: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn primed(entity: Entity, power: f32, fuse: i32, owner: Option<&dyn EntityBase>) -> Self {
        let rotation = entity.world.load().rand_f64() * f64::from(std::f64::consts::TAU as f32);
        entity.velocity.store(Vector3::new(
            -rotation.sin() * 0.02,
            f64::from(0.2_f32),
            -rotation.cos() * 0.02,
        ));
        let tnt = Self::new(entity, power, fuse);
        *tnt.owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = owner
            .filter(|owner| owner.get_living_entity().is_some())
            .map(|owner| super::entity_reference::EntityOwner::new(owner.get_entity().entity_uuid));
        tnt
    }
    pub fn shorten_fuse_after_explosion(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.fuse = self
            .entity
            .world
            .load()
            .rand_bounded_i32((state.fuse / 4).max(1))
            + state.fuse / 8;
    }
}

impl EntityBase for TNTEntity {
    fn teleport(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: std::sync::Arc<crate::world::World>,
    ) {
        self.entity.teleport(position, yaw, pitch, &world);
        self.used_portal.store(true, Ordering::Relaxed);
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        entity.tick_portal(caller);

        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.velocity.store(velo);
        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);

        // Read back what actually happened instead of reusing the pre-move
        // value: `move_entity` clamps on collision, and an explosion may have
        // pushed us while we were moving above
        let velo = entity.velocity.load() * f64::from(self.get_air_drag());
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
        } else {
            entity.velocity.store(velo);
        }

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
        }

        let (fuse, power) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.fuse = state.fuse.wrapping_sub(1);
            (state.fuse, state.power)
        };
        entity.set_synced_data(pumpkin_data::tracked_data::tnt::FUSE_ID, VarInt(fuse));
        if fuse <= 0 {
            entity.remove();
            let world = entity.world.load_full();
            let mut pos = entity.pos.load();
            pos.y += f64::from(entity.entity_dimension.load().height) * 0.0625;
            if world.level_info.load().game_rules.tnt_explodes {
                world.explode_with_source(
                    pos,
                    power,
                    crate::world::ExplosionInteraction::Tnt,
                    self.used_portal.load(Ordering::Relaxed).then(|| {
                        std::sync::Arc::new(
                            crate::world::explosion::PortalTntExplosionDamageCalculator,
                        )
                            as std::sync::Arc<
                                dyn crate::world::explosion::ExplosionDamageCalculator,
                            >
                    }),
                    false,
                    Some(self),
                );
            }
        } else {
            entity.update_fluid_state(caller);
        }
    }

    fn get_explosion_owner(&self) -> Option<std::sync::Arc<dyn EntityBase>> {
        self.owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()?
            .resolve(
                |cached| {
                    cached.upgrade().filter(|owner| {
                        !owner.get_entity().is_removed() && owner.get_living_entity().is_some()
                    })
                },
                |uuid| {
                    let owner = self.entity.world.load().get_entity_in_any_dimension(uuid)?;
                    if owner.get_entity().is_removed() || owner.get_living_entity().is_none() {
                        return None;
                    }
                    Some((std::sync::Arc::downgrade(&owner), owner))
                },
            )
    }

    fn write_custom_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write(nbt);
        if let Some(owner) = self
            .owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            owner.write(nbt, "owner");
        }
    }

    fn read_custom_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            super::tnt_state::TntState::read(nbt);
        *self
            .owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            super::entity_reference::EntityOwner::read(nbt, "owner");
    }

    fn init_data_tracker(&self) {
        let state = *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.entity
            .set_synced_data(pumpkin_data::tracked_data::tnt::FUSE_ID, VarInt(state.fuse));
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::tnt::BLOCK_STATE_ID,
            VarInt(i32::from(state.block.as_u16())),
        );
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_default_gravity(&self) -> f64 {
        0.04
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
