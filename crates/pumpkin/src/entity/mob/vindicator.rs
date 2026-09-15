use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal,
        break_door::BreakDoorGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, open_door::OpenDoorGoal,
        revenge::RevengeGoal, swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    ai::target_predicate::TargetPredicate,
    mob::{
        Mob, MobEntity,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            HoldGroundAttackGoal, ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, Raider,
            RaiderCelebrationGoal, RaiderData, RaiderMoveThroughVillageGoal,
        },
    },
};

pub struct VindicatorEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    /// Vindicator.java:54 `isJohnny`. Sticky: once the name matches, stays `true` even if
    /// the vindicator is renamed away afterwards (vanilla never resets it either).
    pub is_johnny: AtomicBool,
}

impl VindicatorEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let vindicator = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            is_johnny: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(vindicator);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let vindicator_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vindicator.java:63 `FloatGoal` -> SwimGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Vindicator.java:64 `AvoidEntityGoal<>(this, Creaking.class, 8.0F, 1.0, 1.2)`
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::CREAKING, 8.0, 1.0, 1.2)),
            );
            // Raider.java:64 `ObtainRaidLeaderBannerGoal` priority 1
            goal_selector.add_goal(1, Box::new(ObtainRaidLeaderBannerGoal));
            // Vindicator.java:65 `Vindicator.VindicatorBreakDoorGoal(this)`
            goal_selector.add_goal(
                2,
                Box::new(VindicatorBreakDoorGoal::new(vindicator_weak.clone())),
            );
            // Vindicator.java:66 `AbstractIllager.RaiderOpenDoorGoal(this)`
            goal_selector.add_goal(
                3,
                Box::new(VindicatorOpenDoorGoal::new(vindicator_weak.clone())),
            );
            // Raider.java:65 `PathfindToRaidGoal<>(this)` priority 3
            goal_selector.add_goal(3, Box::new(PathfindToRaidGoal::default()));
            // Vindicator.java:67 `Raider.HoldGroundAttackGoal(this, 10.0F)` priority 4
            goal_selector.add_goal(4, Box::new(HoldGroundAttackGoal::new(10.0)));
            // PatrollingMonster.java:40 `LongDistancePatrolGoal<>(this, 0.7, 0.595)` priority 4
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            // Raider.java:66 `RaiderMoveThroughVillageGoal(this, 1.05F, 1)` priority 4
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            // Vindicator.java:68 `MeleeAttackGoal(this, 1.0, false)` priority 5 — vanilla passes
            // `false` (don't keep following once the target is lost from sight), pumpkin
            // previously had `true` and the goal at the wrong priority (3).
            goal_selector.add_goal(5, Box::new(MeleeAttackGoal::new(1.0, false)));
            // Raider.java:67 `RaiderCelebration(this)` priority 5
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            // Vindicator.java:74 `RandomStrollGoal(this, 0.6)` priority 8
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.6)));
            // Vindicator.java:75 `LookAtPlayerGoal(this, Player.class, 3.0F, 1.0F)` priority 9
            goal_selector.add_goal(
                9,
                Box::new(LookAtEntityGoal::new(
                    mob_weak,
                    &EntityType::PLAYER,
                    3.0,
                    1.0,
                    false,
                )),
            );
            // Vindicator.java:76 `LookAtPlayerGoal(this, Mob.class, 8.0F)` priority 10 wants
            // "look at any nearby Mob"; LookAtEntityGoal (ai/goal/look_at_entity.rs, not ours to
            // modify) only supports one concrete EntityType. Kept as closest idle-look fallback.
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Vindicator.java:69 `HurtByTargetGoal(this, Raider.class).setAlertOthers()`
            // priority 1. RevengeGoal (ai/goal/revenge.rs) is the closest existing type, but its
            // "alert nearby raiders" behaviour is an explicit TODO in that file (not ours to
            // extend), so a struck vindicator retaliates itself but won't call in allies.
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // Vindicator.java:70 `NearestAttackableTargetGoal<>(this, Player.class, true)`
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            // Vindicator.java:71 `NearestAttackableTargetGoal<>(this, AbstractVillager.class, true)`
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, true),
            );
            // Vindicator.java:72 `NearestAttackableTargetGoal<>(this, IronGolem.class, true)`
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
            // Vindicator.java:73 `VindicatorJohnnyAttackGoal(this)`
            target_selector.add_goal(
                4,
                Box::new(VindicatorJohnnyAttackGoal::new(vindicator_weak)),
            );
        };

        mob_arc
    }
}

impl Mob for VindicatorEntity {
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
        // Vindicator.java:100-103: only written when `true`.
        if self.is_johnny.load(Ordering::Relaxed) {
            nbt.put_bool("Johnny", true);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
        if let Some(johnny) = nbt.get_bool("Johnny") {
            self.is_johnny.store(johnny, Ordering::Relaxed);
        }
    }
}

impl PatrollingMonster for VindicatorEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }
}

impl Raider for VindicatorEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityVindicatorCelebrate
    }
}

/// Vindicator.java:182-205 `VindicatorBreakDoorGoal`: only breaks doors during an active
/// raid, and only rolls a 1-in-10 chance to even attempt it each time it could start.
struct VindicatorBreakDoorGoal {
    inner: BreakDoorGoal,
    vindicator: Weak<VindicatorEntity>,
}

impl VindicatorBreakDoorGoal {
    fn new(vindicator: Weak<VindicatorEntity>) -> Self {
        Self {
            // Vindicator.java:184: `super(mob, 6, DOOR_BREAKING_PREDICATE)`. `getDoorBreakTime()`
            // clamps to `Math.max(240, doorBreakTime)` (BreakDoorGoal.java:27), so the literal
            // `6` here is functionally identical to the default 240 — kept for fidelity anyway.
            inner: BreakDoorGoal::with_door_break_time(
                6,
                Arc::new(|d: Difficulty| d == Difficulty::Normal || d == Difficulty::Hard),
            ),
            vindicator,
        }
    }
}

impl Goal for VindicatorBreakDoorGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(vindicator) = self.vindicator.upgrade() else {
            return false;
        };
        if !vindicator.has_active_raid() {
            return false;
        }
        // Vindicator.java:197: `random.nextInt(reducedTickDelay(10)) == 0`.
        if mob.get_random().random_range(0..10) != 0 {
            return false;
        }
        self.inner.can_start(mob)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let Some(vindicator) = self.vindicator.upgrade() else {
            return false;
        };
        vindicator.has_active_raid() && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
        // Vindicator.java:203: `this.mob.setNoActionTime(0);`
        mob.get_mob_entity().no_action_time.store(0, Ordering::Relaxed);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// `AbstractIllager.RaiderOpenDoorGoal` (`AbstractIllager.java:51-60`): plain door-opening,
/// gated on an active raid so vindicators don't let themselves into houses outside a raid.
struct VindicatorOpenDoorGoal {
    inner: OpenDoorGoal,
    vindicator: Weak<VindicatorEntity>,
}

impl VindicatorOpenDoorGoal {
    fn new(vindicator: Weak<VindicatorEntity>) -> Self {
        Self {
            // AbstractIllager.java:53: `super(raider, false)`.
            inner: OpenDoorGoal::new(false),
            vindicator,
        }
    }
}

impl Goal for VindicatorOpenDoorGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(vindicator) = self.vindicator.upgrade() else {
            return false;
        };
        vindicator.has_active_raid() && self.inner.can_start(mob)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::empty()
    }
}

/// Vindicator.java:207-222 `VindicatorJohnnyAttackGoal`: a vindicator custom-named "Johnny"
/// attacks any nearby attackable `LivingEntity`, not just its usual raider target types.
///
/// Vanilla latches `isJohnny` inside an override of `setCustomName` the moment the name is
/// set to "Johnny" (Vindicator.java:144-150). Pumpkin's `Entity::set_custom_name`
/// (`entity/mod.rs`) is a plain method with no per-mob override hook, and adding one there is
/// out of scope for a goal audit, so this checks the current custom name lazily on each
/// `can_start` instead and latches `is_johnny` the first time it matches — same end state
/// (permanently latched once ever named "Johnny"), just detected a tick later than vanilla.
struct VindicatorJohnnyAttackGoal {
    vindicator: Weak<VindicatorEntity>,
}

impl VindicatorJohnnyAttackGoal {
    const fn new(vindicator: Weak<VindicatorEntity>) -> Self {
        Self { vindicator }
    }
}

impl Goal for VindicatorJohnnyAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(vindicator) = self.vindicator.upgrade() else {
            return false;
        };
        let entity = &vindicator.mob_entity.living_entity.entity;

        if !vindicator.is_johnny.load(Ordering::Relaxed) {
            let named_johnny = (**entity.custom_name.load())
                .as_ref()
                .is_some_and(|name| name.clone().get_text() == "Johnny");
            if named_johnny {
                vindicator.is_johnny.store(true, Ordering::Relaxed);
            }
        }
        if !vindicator.is_johnny.load(Ordering::Relaxed) {
            return false;
        }

        // Vindicator.java:209: `NearestAttackableTargetGoal<>(this, LivingEntity.class, 0,
        // true, true, target -> target.attackable())`.
        let follow_range = vindicator
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        let world = entity.world.load();
        let mob_pos = entity.pos.load();
        let target_predicate = TargetPredicate::create_attackable();
        let bb = entity
            .bounding_box
            .load()
            .expand(follow_range, follow_range, follow_range);
        let nearby = world.get_entities_at_box(&bb);

        let mut best: Option<(Arc<dyn EntityBase>, f64)> = None;
        for cand in nearby {
            if cand.get_entity().entity_id == entity.entity_id {
                continue;
            }
            let Some(living) = cand.get_living_entity() else {
                continue;
            };
            if !target_predicate.test(&world, Some(&vindicator.mob_entity.living_entity), living)
            {
                continue;
            }
            let dist = mob_pos.squared_distance_to_vec(&cand.get_entity().pos.load());
            if dist > follow_range * follow_range {
                continue;
            }
            if best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist) {
                best = Some((cand, dist));
            }
        }

        if let Some((target, _)) = best {
            mob.set_mob_target(Some(target));
            true
        } else {
            false
        }
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        mob.get_mob_entity()
            .get_target()
            .is_some_and(|t| t.get_entity().is_alive())
    }

    fn start(&mut self, mob: &dyn Mob) {
        // Vindicator.java:220: `this.mob.setNoActionTime(0);`
        mob.get_mob_entity().no_action_time.store(0, Ordering::Relaxed);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        mob.set_mob_target(None);
    }

    fn controls(&self) -> Controls {
        Controls::TARGET
    }
}
