use std::sync::Arc;

use pumpkin_data::potion::Effect;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::{
    Entity, EntityBase,
    mob::{Mob, MobEntity},
};
use pumpkin_data::effect::StatusEffect;
use pumpkin_nbt::compound::NbtCompound;

pub struct HuskEntity {
    entity: Arc<ZombieEntityBase>,
}

impl HuskEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = ZombieEntityBase::new(entity);
        let zombie = Self { entity };
        Arc::new(zombie)
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let entity = ZombieEntityBase::with_can_break_doors(entity, can_break_doors);
        let zombie = Self { entity };
        Arc::new(zombie)
    }
}

impl Mob for HuskEntity {
    fn as_zombie_base(&self) -> Option<&ZombieEntityBase> {
        Some(&self.entity)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    /// `Husk.doHurtTarget` (Husk.java): a bare-handed husk gives its victim hunger for
    /// 140 ticks per point of effective (regional) difficulty.
    fn on_attack(&self, target: &dyn EntityBase) {
        if !self
            .entity
            .mob_entity
            .living_entity
            .held_item(self)
            .is_empty()
        {
            return;
        }
        let Some(living) = target.get_living_entity() else {
            return;
        };
        let entity = self.get_entity();
        let world = entity.world.load_full();
        let difficulty = RegionalDifficulty::at(&world, entity.pos.load());
        // Vanilla casts the float to an int, truncating, before multiplying.
        let duration = 140 * difficulty.effective_difficulty as i32;
        if duration > 0 {
            living.add_effect(Effect {
                effect_type: &StatusEffect::HUNGER,
                duration,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }

    /// `Husk` inherits `Zombie.tick`, which is where the water conversion runs -- a husk
    /// under water turns into a plain zombie (Husk.java:83).
    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.entity.mob_tick(caller);
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.entity.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.entity.mob_read_nbt(nbt);
    }
}
