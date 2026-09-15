use super::{Controls, Goal};

use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::predicate::EntityPredicate;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

const MAX_ATTACK_TIME: i64 = 20;

pub struct MeleeAttackGoal {
    goal_control: Controls,
    speed: f64,
    pause_when_mob_idle: bool,
    //path: Path, TODO: add path when Navigation is implemented
    #[expect(dead_code)]
    target_location: Vector3<f64>,
    update_countdown_ticks: i32,
    pub cooldown: i32,
    #[expect(dead_code)]
    attack_interval_ticks: i32,
    last_update_time: i64,
    last_target_position: Option<Vector3<f64>>,
}

impl MeleeAttackGoal {
    #[must_use]
    pub fn new(speed: f64, pause_when_mob_idle: bool) -> Self {
        Self {
            goal_control: Controls::MOVE | Controls::LOOK,
            // MeleeAttackGoal.java:27: `this.speedModifier = speedModifier;` — stored untouched,
            // no floor. Checked every MeleeAttackGoal/ZombieAttackGoal construction site in the
            // tree: all built-in mobs pass >=1.0 (e.g. zombie 1.0, skeleton 1.2, bee 1.4), so the
            // old 0.23 clamp never actually fired for any of them; it was a Pumpkin-only
            // invention with no vanilla basis, so it is removed rather than kept as a floor.
            speed,
            pause_when_mob_idle,
            target_location: Vector3::new(0.0, 0.0, 0.0),
            update_countdown_ticks: 0,
            cooldown: 0,
            attack_interval_ticks: 20,
            last_update_time: 0,
            last_target_position: None,
        }
    }

    #[must_use]
    pub fn get_max_cooldown(&self) -> i32 {
        self.get_tick_count(20)
    }
}

impl Goal for MeleeAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let time = mob.get_entity().world.load().get_world_age();

        if time - self.last_update_time < MAX_ATTACK_TIME {
            return false;
        }
        self.last_update_time = time;

        let target = mob.get_mob_entity().get_target();

        let Some(target) = target else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }

        // MeleeAttackGoal.java:48-50: `this.path = createPath(target, 0);` and then
        // `path != null ? true : isWithinMeleeAttackRange(target)`. A mob with no route to
        // its target must not claim MOVE/LOOK, but one already standing next to the target
        // still starts (and `start` zeroes the attack cooldown, which is what lets it swing).
        let mob_entity = mob.get_mob_entity();
        let has_path = {
            let mut navigator = mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator
                .create_path(&mob_entity.living_entity, target.get_entity().pos.load(), 0)
                .is_some()
        };

        has_path || mob_entity.is_in_attack_range(&*target)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let target = mob.get_mob_entity().get_target().clone();

        let Some(target) = target else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }

        if !self.pause_when_mob_idle {
            let is_idle = mob
                .get_mob_entity()
                .navigator
                .try_lock()
                .is_ok_and(|navigator| navigator.is_idle());
            return !is_idle;
        }

        let is_valid_target = !target
            .get_player()
            .is_some_and(|p| p.is_spectator() || p.is_creative());

        let in_range = mob
            .get_mob_entity()
            .is_in_position_target_range_pos(&target.get_entity().block_pos.load());

        in_range && is_valid_target
    }

    fn start(&mut self, mob: &dyn Mob) {
        // MeleeAttackGoal.java:70 `this.mob.setAggressive(true)`.
        mob.get_mob_entity().set_attacking(true);

        let target = mob.get_mob_entity().get_target().clone();
        if let Some(target) = target {
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let target_pos = target.get_entity().pos.load();
            navigator.set_progress(NavigatorGoal {
                current_progress: mob.get_entity().pos.load(),
                destination: target_pos,
                speed: self.speed,
            });
            self.last_target_position = Some(target_pos);
        }
        self.update_countdown_ticks = 0;
        self.cooldown = 0;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        // MeleeAttackGoal.java:77-78: the target is dropped *only* when it stopped being a
        // legitimate target, i.e. a player that went creative or spectator. Every other
        // target — survival players, villagers, golems — must survive the goal stopping,
        // because this goal stops (and restarts 20 ticks later) constantly: `should_continue`
        // ends it the moment the navigation path completes, which is exactly when the mob
        // has arrived next to its victim.
        let should_clear = mob
            .get_mob_entity()
            .get_target()
            .as_deref()
            .is_some_and(|entity| {
                !EntityPredicate::ExceptCreativeOrSpectator.test(entity.get_entity())
            });
        if should_clear {
            mob.set_mob_target(None);
        }

        // MeleeAttackGoal.java:80 `this.mob.setAggressive(false)`.
        mob.get_mob_entity().set_attacking(false);

        // Vanilla: this.mob.getNavigation().stop()
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.last_target_position = None;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let target = mob.get_mob_entity().get_target().clone();
        let Some(target) = target else {
            return;
        };

        if mob.use_goal_look_control() {
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);
        }

        self.update_countdown_ticks = (self.update_countdown_ticks - 1).max(0);

        let current_target_pos = target.get_entity().pos.load();
        // MeleeAttackGoal.java:93-101. Vanilla evaluates the line-of-sight term first, but
        // every term here is side-effect free, so the cheap counter checks come first and the
        // raycast only runs on the ticks that would actually repath.
        let should_update_nav = self.update_countdown_ticks <= 0
            && (self.last_target_position.is_none_or(|last_pos| {
                current_target_pos.squared_distance_to_vec(&last_pos) >= 1.0
            }) || mob.get_random().random_range(0..20) == 0)
            && (self.pause_when_mob_idle
                || mob.get_entity().world.load_full().has_line_of_sight(
                    mob.get_entity().get_eye_pos(),
                    target.get_entity().get_eye_pos(),
                ));

        if should_update_nav {
            let mob_pos = mob.get_entity().pos.load();
            let dist_sq = mob_pos.squared_distance_to_vec(&current_target_pos);
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_progress(NavigatorGoal {
                current_progress: mob_pos,
                destination: current_target_pos,
                speed: self.speed,
            });
            self.last_target_position = Some(current_target_pos);
            self.update_countdown_ticks = 4 + mob.get_random().random_range(0..7);
            if dist_sq > 1024.0 {
                self.update_countdown_ticks += 10;
            } else if dist_sq > 256.0 {
                self.update_countdown_ticks += 5;
            }
        }

        self.cooldown = (self.cooldown - 1).max(0);

        if self.cooldown <= 0
            && mob.can_use_melee_attack()
            && mob.get_mob_entity().is_in_attack_range(target.as_ref())
            && mob.get_entity().world.load_full().has_line_of_sight(
                mob.get_entity().get_eye_pos(),
                target.get_entity().get_eye_pos(),
            )
        {
            self.cooldown = self.get_max_cooldown();
            mob.get_mob_entity().living_entity.swing_hand();
            mob.get_mob_entity().try_attack(mob, target.as_ref());
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
