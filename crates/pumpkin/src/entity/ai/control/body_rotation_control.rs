use crate::entity::ai::control::{Control, body_rotation_math::BodyRotation};
use crate::entity::mob::Mob;

#[derive(Default)]
pub struct BodyRotationControl {
    state: BodyRotation,
}

impl BodyRotationControl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn client_tick(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        let position = entity.pos.load();
        let previous = entity.last_pos.load();
        let first_passenger_is_mob = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .is_some_and(|passenger| passenger.get_mob().is_some());
        let [body, head] = self.state.tick(
            [position.x - previous.x, position.z - previous.z],
            entity.yaw.load(),
            entity.body_yaw.load(),
            entity.head_yaw.load(),
            mob.get_max_head_rotation(),
            first_passenger_is_mob,
        );
        entity.body_yaw.store(body);
        entity.head_yaw.store(head);
    }
}

impl Control for BodyRotationControl {}
