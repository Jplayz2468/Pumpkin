use crate::entity::{Entity, EntityBase};

pub enum EntityPredicate<'a> {
    ValidEntity,
    ValidLivingEntity,
    NotMounted,
    ValidInventories,
    ExceptCreativeOrSpectator,
    ExceptSpectator,
    CanCollide,
    CanHit,
    Rides(&'a Entity),
}

impl EntityPredicate<'_> {
    #[must_use]
    pub fn test(&self, entity: &Entity) -> bool {
        match self {
            EntityPredicate::ValidEntity => entity.is_alive(),
            EntityPredicate::ValidLivingEntity => {
                entity.is_alive() && entity.get_living_entity().is_some()
            }
            EntityPredicate::NotMounted => {
                entity.is_alive() && !entity.has_passengers() && !entity.has_vehicle()
            }
            EntityPredicate::ValidInventories => {
                // TODO: implement
                false
            }
            // Java's `EntitySelector.NO_CREATIVE_OR_SPECTATOR`:
            //   entity -> !(entity instanceof Player p) || !p.isSpectator() && !p.isCreative()
            // It passes *everything that is not* a creative/spectator player, so every
            // non-player and every survival player must return `true` here. Callers read it
            // as "this target is still a legitimate target".
            EntityPredicate::ExceptCreativeOrSpectator => entity
                .get_player()
                .is_none_or(|player| !player.is_spectator() && !player.is_creative()),
            EntityPredicate::ExceptSpectator => !entity.is_spectator(),
            EntityPredicate::CanCollide => {
                EntityPredicate::ExceptSpectator.test(entity) && entity.is_collidable(None)
            }
            EntityPredicate::CanHit => {
                EntityPredicate::ExceptSpectator.test(entity) && entity.can_hit()
            }
            EntityPredicate::Rides(target_entity) => {
                let target: &Entity = target_entity;

                let mut opt_vehicle_arc = entity.get_vehicle();

                while let Some(vehicle_arc) = opt_vehicle_arc {
                    let vehicle_entity_base: &dyn EntityBase = &*vehicle_arc;
                    let target_base: &dyn EntityBase = target;

                    if std::ptr::eq(vehicle_entity_base, target_base) {
                        return false;
                    }

                    opt_vehicle_arc = vehicle_entity_base.get_entity().get_vehicle();
                }
                true
            }
        }
    }
}
