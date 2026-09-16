use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;

use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;

use pumpkin_data::damage::DamageType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_protocol::java::client::play::Metadata;

use pumpkin_util::math::vector3::Vector3;

use crate::entity::vehicle::vehicle::VehicleEntity;

pub struct BoatEntity {
    pub vehicle: VehicleEntity,
    ticks_underwater: AtomicCell<f32>,
    underwater: AtomicBool,
    bubble_time: AtomicI32,
    above_bubble_column: AtomicBool,
    bubble_drag_down: AtomicBool,
    left_paddle_moving: AtomicBool,
    right_paddle_moving: AtomicBool,
}

impl BoatEntity {
    pub const fn new(entity: Entity) -> Self {
        Self {
            vehicle: VehicleEntity::new(entity),
            ticks_underwater: AtomicCell::new(0.0),
            underwater: AtomicBool::new(false),
            bubble_time: AtomicI32::new(0),
            above_bubble_column: AtomicBool::new(false),
            bubble_drag_down: AtomicBool::new(false),
            left_paddle_moving: AtomicBool::new(false),
            right_paddle_moving: AtomicBool::new(false),
        }
    }

    fn update_underwater_status(&self) -> bool {
        let bounds = self.vehicle.entity.bounding_box.load();
        let max_y = bounds.max.y + 0.001;
        let world = self.vehicle.entity.world.load();
        for x in bounds.min.x.floor() as i32..bounds.max.x.ceil() as i32 {
            for y in bounds.max.y.floor() as i32..max_y.ceil() as i32 {
                for z in bounds.min.z.floor() as i32..bounds.max.z.ceil() as i32 {
                    let pos = pumpkin_util::math::position::BlockPos::new(x, y, z);
                    let (fluid, state) = world.get_fluid_and_fluid_state(&pos);
                    if fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER)
                        && max_y
                            < f64::from(y) + f64::from(world.get_fluid_height(&pos, fluid, &state))
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn set_bubble_time(&self, time: i32) {
        if self.bubble_time.swap(time, Ordering::Relaxed) != time {
            self.vehicle.entity.set_synced_data(
                pumpkin_data::tracked_data::boat::ID_BUBBLE_TIME,
                pumpkin_protocol::codec::var_int::VarInt(time),
            );
        }
    }

    pub fn on_above_bubble_column(&self, drag_down: bool) {
        self.above_bubble_column.store(true, Ordering::Relaxed);
        self.bubble_drag_down.store(drag_down, Ordering::Relaxed);
        if self.bubble_time.load(Ordering::Relaxed) == 0 {
            self.set_bubble_time(60);
        }
    }

    fn tick_bubble_column(&self) {
        if !self.above_bubble_column.swap(false, Ordering::Relaxed) {
            self.set_bubble_time(0);
        }
        let time = self.bubble_time.load(Ordering::Relaxed);
        if time <= 0 {
            return;
        }
        self.set_bubble_time(time - 1);
        if time == 1 {
            let entity = &self.vehicle.entity;
            let passengers = entity
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let velocity = entity.velocity.load();
            if self.bubble_drag_down.load(Ordering::Relaxed) {
                entity.velocity.store(velocity.add_raw(0.0, -0.7, 0.0));
                for passenger in passengers {
                    entity.remove_passenger(passenger.get_entity().entity_id);
                }
            } else {
                entity.velocity.store(Vector3::new(
                    velocity.x,
                    if passengers.iter().any(|p| p.get_player().is_some()) {
                        2.7
                    } else {
                        0.6
                    },
                    velocity.z,
                ));
            }
            entity.velocity_dirty.store(true, Ordering::Relaxed);
        }
    }

    pub fn set_paddles(&self, left: bool, right: bool) {
        self.left_paddle_moving.store(left, Ordering::Relaxed);
        self.right_paddle_moving.store(right, Ordering::Relaxed);

        self.vehicle.entity.send_meta_data(
            &[
                Metadata::new(pumpkin_data::tracked_data::boat::ID_PADDLE_LEFT, left),
                Metadata::new(pumpkin_data::tracked_data::boat::ID_PADDLE_RIGHT, right),
            ],
            None,
        );
    }

    fn send_wobble_metadata(&self) {
        self.vehicle.send_wobble_metadata();
    }
}

impl EntityBase for BoatEntity {
    fn get_entity(&self) -> &Entity {
        &self.vehicle.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        let underwater = self.update_underwater_status();
        self.underwater.store(underwater, Ordering::Relaxed);
        let time = if underwater {
            self.ticks_underwater.load() + 1.0
        } else {
            0.0
        };
        self.ticks_underwater.store(time);
        if time >= 60.0 {
            let passengers = self
                .vehicle
                .entity
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            for passenger in passengers.into_iter().rev() {
                self.vehicle
                    .entity
                    .remove_passenger(passenger.get_entity().entity_id);
            }
        }
        self.vehicle.tick();
        self.vehicle.entity.tick(caller, server);
        self.vehicle.entity.tick_block_collisions(caller);
        self.tick_bubble_column();
    }

    fn modify_passenger_fluid_box(
        &self,
        bounds: pumpkin_util::math::boundingbox::BoundingBox,
    ) -> Option<pumpkin_util::math::boundingbox::BoundingBox> {
        super::super::fluid_interaction::boat_passenger_box(
            self.vehicle.entity.bounding_box.load(),
            bounds,
            self.underwater.load(Ordering::Relaxed),
        )
    }

    fn init_data_tracker(&self) {
        self.send_wobble_metadata();
    }

    fn can_hit(&self) -> bool {
        self.vehicle.entity.is_alive()
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        true
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.vehicle.damage_with_context(amount, source)
    }

    fn interact(&self, player: &Arc<Player>, _item_stack: &mut ItemStack) -> bool {
        if player.get_entity().is_sneaking() {
            return false;
        }

        if self.ticks_underwater.load() >= 60.0 {
            return false;
        }

        if self
            .vehicle
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            >= 2
        {
            return false;
        }

        if player.get_entity().has_vehicle() {
            return false;
        }

        let world = self.vehicle.entity.world.load();
        let Some(vehicle) = world.get_entity_by_id(self.vehicle.entity.entity_id) else {
            return false;
        };

        let Some(passenger) = world.get_player_by_id(player.entity_id()) else {
            return false;
        };

        self.vehicle
            .entity
            .add_passenger(vehicle, passenger as Arc<dyn EntityBase>);

        true
    }

    fn set_paddle_state(&self, left: bool, right: bool) {
        self.set_paddles(left, right);
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn is_pushable(&self) -> bool {
        true
    }
}
