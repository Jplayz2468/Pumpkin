use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::entity::EntityType;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    variant,
};

pub struct TropicalFishEntity {
    pub mob_entity: MobEntity,
    /// `TropicalFish.packVariant` (TropicalFish.java:84): pattern in the low 16 bits,
    /// base colour in bits 16-23, pattern colour in bits 24-31.
    pub packed_variant: AtomicI32,
}

impl TropicalFishEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let tropical_fish = Self {
            mob_entity,
            // TropicalFish.DEFAULT_VARIANT is kob/white/white, which packs to zero.
            packed_variant: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(tropical_fish);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }
}

impl TropicalFishEntity {
    fn sync_variant(&self) {
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::tropical_fish::DATA_ID_TYPE_VARIANT,
            VarInt(self.packed_variant.load(Ordering::Relaxed)),
        );
    }
}

impl Mob for TropicalFishEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    /// `TropicalFish.finalizeSpawn` (TropicalFish.java:236): 90% of fish take one of the
    /// twenty-two named common variants, the rest roll pattern and both colours freely.
    fn mob_finalize_spawn(&self, _reason: crate::entity::spawn::SpawnReason) {
        self.packed_variant
            .store(variant::random_fish_variant(), Ordering::Relaxed);
        self.sync_variant();
    }

    fn mob_init_data_tracker(&self) {
        self.sync_variant();
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("Variant", self.packed_variant.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(packed) = nbt.get_int("Variant") {
            self.packed_variant.store(packed, Ordering::Relaxed);
        }
    }
}
