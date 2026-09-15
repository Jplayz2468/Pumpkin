use std::sync::Arc;

use crate::entity::ai::goal::{
    active_target::ActiveTargetGoal, wander_around::WanderAroundGoal,
    zombie_attack::ZombieAttackGoal,
};
use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::{
    Entity,
    mob::{Mob, MobEntity},
};
use pumpkin_data::entity::EntityType;
use pumpkin_nbt::compound::NbtCompound;

pub struct DrownedEntity {
    entity: Arc<ZombieEntityBase>,
}

impl DrownedEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = ZombieEntityBase::new(entity);
        let zombie = Self { entity };
        let mob_arc = Arc::new(zombie);

        // Vanilla `Drowned` does not override `registerGoals`, so it still gets
        // the turtle-egg and look-around goals from `Zombie.registerGoals`
        // (Zombie.java:112-115), which `ZombieEntityBase::new` above already
        // wires up identically. `Drowned.addBehaviourGoals` (Drowned.java:91-103)
        // replaces everything `Zombie.addBehaviourGoals` (Zombie.java:119-128)
        // would otherwise have added, so we undo and redo the two goals we can
        // still express and add the one extra target.
        {
            let mut goal_selector = mob_arc
                .entity
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Base zombie: ZombieAttackGoal at priority 3 (Zombie.java:121).
            // Drowned: `DrownedAttackGoal extends ZombieAttackGoal` with no
            // overrides, at priority 2 (Drowned.java:94).
            let _ = goal_selector.remove_goals::<ZombieAttackGoal>();
            goal_selector.add_goal(2, ZombieAttackGoal::new(1.0, false));
            // Base zombie: water-avoiding stroll (Zombie.java:123). Drowned
            // uses a plain random stroll instead, since it prefers water
            // (Drowned.java:97, `new RandomStrollGoal(this, 1.0)`).
            let _ = goal_selector.remove_goals::<WanderAroundGoal>();
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(1.0)));
            // Not ported: `DrownedGoToWaterGoal` (:92), `DrownedTridentAttackGoal`
            // (:93), `DrownedGoToBeachGoal` (:95) and `DrownedSwimUpGoal` (:96)
            // have no equivalent goal type in `entity::ai::goal`.
        }
        {
            let mut target_selector = mob_arc
                .entity
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Drowned additionally targets Axolotls (Drowned.java:102); the
            // Villager (mustSee=false), IronGolem and Turtle targets it also
            // declares (Drowned.java:100,101,103) already match what
            // `ZombieEntityBase::new` wired up from the base zombie
            // (Zombie.java:126-128), so they are left alone.
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &mob_arc.entity.mob_entity,
                    &EntityType::AXOLOTL,
                    true,
                ),
            );
            // Not ported: the Player target's `okTarget` predicate
            // (Drowned.java:99) and the "ignore damage from other Drowned,
            // alert nearby ZombifiedPiglin" behaviour of `HurtByTargetGoal`
            // (Drowned.java:98) — `RevengeGoal` here has no per-class ignore
            // or alert support (same pre-existing limitation as the base
            // zombie's revenge goal).
        }

        mob_arc
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let entity = ZombieEntityBase::with_can_break_doors(entity, can_break_doors);
        let zombie = Self { entity };
        Arc::new(zombie)
    }
}

impl Mob for DrownedEntity {
    fn as_zombie_base(&self) -> Option<&ZombieEntityBase> {
        Some(&self.entity)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.entity.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.entity.mob_read_nbt(nbt);
    }
}
