use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{
        Mob, MobEntity,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, Raider, RaiderCelebrationGoal,
            RaiderData, RaiderMoveThroughVillageGoal,
        },
    },
};
use crate::world::World;

pub struct RavagerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
}

impl RavagerEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let ravager = Self {
            mob_entity,
            raider_data: RaiderData::default(),
        };
        let mob_arc = Arc::new(ravager);
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

            // Ravager.java:76 `FloatGoal` -> SwimGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Raider.java:64 `ObtainRaidLeaderBannerGoal` priority 1. `can_be_leader()` is
            // `false` for ravagers (Ravager.java:331-333, mirrored below), so this can never
            // actually fire — kept only so the goal list matches vanilla's registration.
            goal_selector.add_goal(1, Box::new(ObtainRaidLeaderBannerGoal));
            // Raider.java:65 `PathfindToRaidGoal<>(this)` priority 3
            goal_selector.add_goal(3, Box::new(PathfindToRaidGoal::default()));
            // Ravager.java:77 `MeleeAttackGoal(this, 1.0, true)` priority 4. Ravager does NOT
            // subclass/override MeleeAttackGoal itself in vanilla — the "override" referenced
            // in the audit brief is `doHurtTarget`/`aiStep`/`handleEntityEvent`
            // (Ravager.java:135-299: attackTick/stunnedTick/roarTick bookkeeping, the roar AoE
            // knockback, and the shield-block stun) sitting *outside* the goal system, on the
            // entity class itself — see the NOT IMPLEMENTED note below.
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            // PatrollingMonster.java:40 `LongDistancePatrolGoal<>(this, 0.7, 0.595)` priority 4
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            // Raider.java:66 `RaiderMoveThroughVillageGoal(this, 1.05F, 1)` priority 4
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            // Raider.java:67 `RaiderCelebration(this)` priority 5
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            // Ravager.java:78 `WaterAvoidingRandomStrollGoal(this, 0.4)` priority 5 — was a
            // plain `WanderAroundGoal::new(1.0)` at the wrong priority (5, shared, coincided
            // by accident) with the wrong speed and no water avoidance.
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::water_avoiding(0.4)));
            // Ravager.java:79 `LookAtPlayerGoal(this, Player.class, 6.0F)` priority 6 (default
            // probability, matches `with_default`'s 0.02F)
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            // Ravager.java:80 `LookAtPlayerGoal(this, Mob.class, 8.0F)` priority 10 wants "look
            // at any nearby Mob"; LookAtEntityGoal (ai/goal/look_at_entity.rs, not ours to
            // modify) only supports one concrete EntityType. Kept as closest idle-look fallback
            // (also covers the previous priority-7 slot vanilla doesn't otherwise use).
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Ravager.java:81 `HurtByTargetGoal(this, Raider.class).setAlertOthers()` — note
            // vanilla registers this at priority 2 for Ravager specifically (every other
            // raider mob uses 1). RevengeGoal (ai/goal/revenge.rs) is the closest existing
            // type but its "alert nearby raiders" behaviour is an explicit TODO there (not
            // ours to extend), so a struck ravager retaliates itself but won't call in allies.
            target_selector.add_goal(2, Box::new(RevengeGoal::new(true)));
            // Ravager.java:82 `NearestAttackableTargetGoal<>(this, Player.class, true)`
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            // Ravager.java:83 `NearestAttackableTargetGoal<>(this, AbstractVillager.class,
            // true, (target, level) -> !target.isBaby())` — was missing the baby-villager
            // exclusion predicate entirely.
            target_selector.add_goal(
                4,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::VILLAGER,
                    10,
                    true,
                    false,
                    Some(|target: &LivingEntity, _world: &World| {
                        target.entity.age.load(std::sync::atomic::Ordering::Relaxed) >= 0
                    }),
                )),
            );
            // Ravager.java:84 `NearestAttackableTargetGoal<>(this, IronGolem.class, true)`
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }
}

impl Mob for RavagerEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_patrolling_monster(&self) -> Option<&dyn PatrollingMonster> {
        Some(self)
    }

    fn as_raider(&self) -> Option<&dyn Raider> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_raider_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
    }
}

impl PatrollingMonster for RavagerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }

    fn can_be_leader(&self) -> bool {
        false
    }
}

impl Raider for RavagerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityRavagerCelebrate
    }
}

// NOT IMPLEMENTED: Ravager.java:63-65,135-299 `attackTick`/`stunnedTick`/`roarTick` fields
// and the `aiStep`/`doHurtTarget`/`handleEntityEvent`/`blockedByItem` overrides that drive
// them (attack-animation lock via `isImmobile()`, the post-stun roar AoE damage/knockback
// and particle burst, and the shield-block stun). This is core Ravager behaviour but lives
// entirely outside `registerGoals()`/the goal system, on hooks the `Mob` trait does not
// currently wire up for it:
//   - `on_attack` (mob/mod.rs Mob trait, the natural home for a `doHurtTarget` override) is
//     defined but never invoked by `MobEntity::try_attack` (mob/mod.rs) or anywhere else in
//     the tree, so there is no live hook to react to a successful melee hit from here.
//   - There is no `isImmobile()`/`hasLineOfSight()` override point exposed to mobs, and no
//     "defender blocked with a shield" hook equivalent to vanilla's `blockedByItem`.
// Wiring any of this up means changing `MobEntity::try_attack` and/or the combat/blocking
// pipeline in the shared `entity/mob/mod.rs` / `entity/living.rs`, which is well outside a
// goal-list audit and risks colliding with every other mob family being ported in parallel.
// Flagging precisely rather than guessing at new shared-engine API.
