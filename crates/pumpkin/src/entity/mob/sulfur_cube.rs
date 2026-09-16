use std::sync::Arc;

use pumpkin_data::sound::Sound;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, slime::SlimeEntity},
};

/// A sulfur cube, the cube mob of the sulfur caves.
///
/// Built on the shared cube behaviour like the magma cube, so it jumps, chases,
/// attacks on contact and splits when killed.
///
/// Not yet implemented: the archetype system. In vanilla a sulfur cube swallows
/// an item, matches it against the `sulfur_cube_archetype` registry and takes on
/// that archetype's attribute modifiers, explosion, contact damage, knockback and
/// sounds -- which is also where its fuse priming, bucketing and shearing come
/// from. Until that lands this is a plain hostile cube. Tracked in
/// SURVIVAL_PARITY_BACKLOG.md.
pub struct SulfurCubeEntity {
    pub slime: Arc<SlimeEntity>,
}

impl SulfurCubeEntity {
    /// `SulfurCube.MAX_SIZE`; the shared cube spawn roll would otherwise reach 4.
    const MAX_SIZE: i32 = 2;

    pub fn new(entity: Entity) -> Arc<Self> {
        let slime = SlimeEntity::new(entity);
        // SlimeEntity::new randomizes across 1/2/4, which a sulfur cube never uses.
        let size = slime.get_size().clamp(1, Self::MAX_SIZE);
        slime.set_size(size, true);
        Arc::new(Self { slime })
    }
}

impl CustomSound for SulfurCubeEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySulfurCubeDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySulfurCubeHurt)
    }
}

impl Mob for SulfurCubeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        self.slime.get_mob_entity()
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.slime.mob_tick(caller);
    }

    fn post_tick(&self) {
        self.slime.post_tick();
    }

    fn mob_player_collision(&self, player: &Arc<crate::entity::player::Player>) {
        self.slime
            .get_mob_entity()
            .try_attack(&*self.slime, &**player);
    }
}
