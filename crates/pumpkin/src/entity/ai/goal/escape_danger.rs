use std::sync::atomic::Ordering::Relaxed;

use super::{Controls, Goal};
use crate::entity::ai::goal::try_find_water::TryFindWaterGoal;
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob};
use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

/// `RandomPos`'s horizontal search distance that `PanicGoal.findRandomPosition` passes to
/// `DefaultRandomPos.getPos(this.mob, 5, 4)` (PanicGoal.java:65), and that
/// `lookForWater` also uses as `xzDist` (PanicGoal.java:48).
const HORIZONTAL_RANGE: i32 = 5;
/// `DefaultRandomPos.getPos`'s `verticalDist` argument (PanicGoal.java:65).
const VERTICAL_RANGE: i32 = 4;
/// `PanicGoal.WATER_CHECK_DISTANCE_VERTICAL` (PanicGoal.java:17).
const WATER_CHECK_DISTANCE_VERTICAL: i32 = 1;
/// `lookForWater`'s `xzDist` parameter, called with `5` from `canUse` (PanicGoal.java:44).
const WATER_CHECK_DISTANCE_HORIZONTAL: i32 = 5;
/// `LivingEntity.getLastDamageSource` nulls the source out after this many ticks
/// (LivingEntity.java:1419-1425): `if (level().getGameTime() - lastDamageStamp > 40L)`.
/// This is a different (shorter) window than `HurtByTargetGoal`'s 100-tick
/// `getLastHurtByMob` memory (LivingEntity.java:492) — the two goals key off different
/// pieces of vanilla state and must not share a constant.
const RECENT_DAMAGE_TICKS: i32 = 40;

/// Pure core of `is_in_danger`'s "was recently hit" branch, split out so the
/// `RECENT_DAMAGE_TICKS` boundary can be pinned without constructing a `dyn Mob`.
/// `last_attacked == 0` is Pumpkin's sentinel for "never attacked" (see
/// `RevengeGoal::can_start` for the same convention).
const fn recently_attacked(now: i32, last_attacked: i32) -> bool {
    last_attacked != 0 && now - last_attacked < RECENT_DAMAGE_TICKS
}

pub struct EscapeDangerGoal {
    speed: f64,
    goal_control: Controls,
    target: Option<Vector3<f64>>,
}

impl EscapeDangerGoal {
    #[must_use]
    pub fn new(speed: f64) -> Box<Self> {
        Box::new(Self {
            speed,
            goal_control: Controls::MOVE,
            target: None,
        })
    }

    /// Approximates `PanicGoal.shouldPanic()`, which checks that
    /// `getLastDamageSource()` (self-expiring after 40 ticks, see `RECENT_DAMAGE_TICKS`)
    /// is tagged `DamageTypeTags.PANIC_CAUSES`. Pumpkin's `LivingEntity` does not track
    /// the *type* of the last damage taken (only the last attacker and when), so this
    /// cannot filter by tag yet — that would need a new `last_damage_type` field on
    /// `LivingEntity`, set from `damage()`, which is shared machinery outside a goal file.
    /// The `fire_ticks` check stands in for `is_fire`/`on_fire`/`in_fire`/`lava`, which
    /// are themselves members of `PANIC_CAUSES` (tag.rs:7309-7346), so it does not add a
    /// case vanilla lacks; it only exists because the generic "was hit recently" check
    /// below can't see that a currently-burning mob is (re-)taking fire damage every tick.
    /// `PanicGoal.shouldPanic` (PanicGoal.java:61-63): the last damage source must
    /// still be live -- `getLastDamageSource` clears it after 40 ticks -- and must
    /// carry the goal's damage-type tag, `panic_causes` by default.
    ///
    /// This reads the last damage of *any* kind, not just damage with an attacker.
    /// Vanilla records `lastDamageSource` for every successful hit, so a mob flees
    /// a cactus, a fall or a fire; keying off the attacker alone meant it only ever
    /// fled things that punched it.
    fn is_in_danger(mob: &dyn Mob) -> bool {
        let living = &mob.get_mob_entity().living_entity;

        let Some(damage_type) = living.last_damage_type.load() else {
            return false;
        };
        if !damage_type.has_tag(&tag::DamageType::MINECRAFT_PANIC_CAUSES) {
            return false;
        }

        let last_damage = living.last_damage_time.load(Relaxed);
        let age = living.entity.tick_count.load(Relaxed);
        recently_attacked(age, last_damage)
    }

    /// Port of `PanicGoal.lookForWater` (PanicGoal.java:92-97), used only while on fire.
    /// Bails out like vanilla when the mob's own block has a non-empty collision shape
    /// (approximated here with `is_solid`), then searches the closest matching block
    /// within `WATER_CHECK_DISTANCE_HORIZONTAL`/`WATER_CHECK_DISTANCE_VERTICAL`, ranked
    /// by Manhattan distance the way `BlockPos.findClosestMatch`'s `withinManhattan`
    /// iterates (ties are not guaranteed to break identically to vanilla's iteration
    /// order, which does not affect correctness of the search).
    fn look_for_water(mob: &dyn Mob) -> Option<BlockPos> {
        let me = mob.get_mob_entity();
        let world = me.living_entity.entity.world.load();
        let origin = me.living_entity.entity.block_pos.load();

        if world.get_block_state(&origin).is_solid() {
            return None;
        }

        let mut best: Option<(i32, BlockPos)> = None;
        for dx in -WATER_CHECK_DISTANCE_HORIZONTAL..=WATER_CHECK_DISTANCE_HORIZONTAL {
            for dz in -WATER_CHECK_DISTANCE_HORIZONTAL..=WATER_CHECK_DISTANCE_HORIZONTAL {
                for dy in -WATER_CHECK_DISTANCE_VERTICAL..=WATER_CHECK_DISTANCE_VERTICAL {
                    let pos = BlockPos(origin.0 + Vector3::new(dx, dy, dz));
                    if !TryFindWaterGoal::is_water(&world, &pos) {
                        continue;
                    }
                    let dist = dx.abs() + dy.abs() + dz.abs();
                    if best.is_none_or(|(best_dist, _)| dist < best_dist) {
                        best = Some((dist, pos));
                    }
                }
            }
        }
        best.map(|(_, pos)| pos)
    }

    /// Port of `PanicGoal.findRandomPosition` (PanicGoal.java:63-71), which delegates to
    /// `DefaultRandomPos.getPos(mob, 5, 4)` — the same pathfinding-validated random
    /// position search `WanderAroundGoal` already ports as `find_ground_target` with
    /// `land = false` (plain `DefaultRandomPos`, not the water-avoiding `LandRandomPos`).
    fn find_escape_target(mob: &dyn Mob) -> Option<Vector3<f64>> {
        WanderAroundGoal::find_ground_target(mob, HORIZONTAL_RANGE, VERTICAL_RANGE, false)
    }
}

impl Goal for EscapeDangerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if !Self::is_in_danger(mob) {
            return false;
        }

        // PanicGoal.java:44-51: a burning mob tries to path to nearby water first.
        if mob.get_mob_entity().living_entity.entity.fire_ticks.load(Relaxed) > 0
            && let Some(water) = Self::look_for_water(mob)
        {
            self.target = Some(Vector3::new(
                f64::from(water.0.x),
                f64::from(water.0.y),
                f64::from(water.0.z),
            ));
            return true;
        }

        self.target = Self::find_escape_target(mob);
        self.target.is_some()
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        let navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !navigator.is_idle()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            let pos = mob.get_mob_entity().living_entity.entity.pos.load();
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_progress(NavigatorGoal::new(pos, target, self.speed));
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.target = None;
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}

#[cfg(test)]
mod tests {
    use super::recently_attacked;

    /// Pins `LivingEntity.getLastDamageSource`'s 40-tick auto-expiry
    /// (LivingEntity.java:1419-1425: `if (gameTime - lastDamageStamp > 40L) source = null;`),
    /// which `PanicGoal.shouldPanic` relies on. Before this fix the goal used a stale
    /// 100-tick window borrowed from `HurtByTargetGoal`'s unrelated `getLastHurtByMob`
    /// memory (LivingEntity.java:492), so a mob would keep "panicking" for 60 ticks
    /// longer than vanilla after being hit once.
    #[test]
    fn java_oracle_recently_attacked_boundary() {
        assert!(recently_attacked(139, 100)); // 39 ticks ago: still within the window.
        assert!(!recently_attacked(140, 100)); // exactly 40 ticks ago: expired.
        assert!(!recently_attacked(200, 100)); // long expired.
        assert!(!recently_attacked(50, 0)); // 0 is the "never attacked" sentinel.
    }
}
