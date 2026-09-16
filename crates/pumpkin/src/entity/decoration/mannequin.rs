use crate::entity::{Entity, EntityBase, living::LivingEntity};
use pumpkin_nbt::compound::NbtCompound;

/// A mannequin: a player-shaped, immobile display entity.
///
/// `Mannequin.java` extends `Avatar`, the shared player base, rather than any
/// mob class, and reports `isImmobile`, so it never moves or runs AI. It holds a
/// resolvable game profile that decides which skin it wears, and it accepts
/// equipment like an armour stand does.
///
/// Implemented here as an immobile living entity so that summoning one produces
/// the right entity rather than a bare fallback. Not yet implemented: the
/// profile component that gives it a skin, and equipping it. It has no spawn egg
/// and no natural spawn, so it is only reachable through `/summon`. Tracked in
/// SURVIVAL_PARITY_BACKLOG.md.
pub struct MannequinEntity {
    living_entity: LivingEntity,
}

impl MannequinEntity {
    pub fn new(entity: Entity) -> Self {
        Self {
            living_entity: LivingEntity::new(entity),
        }
    }
}

#[async_trait::async_trait]
impl EntityBase for MannequinEntity {
    fn get_entity(&self) -> &Entity {
        &self.living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.living_entity)
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
