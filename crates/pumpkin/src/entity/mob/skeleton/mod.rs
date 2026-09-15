use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::Difficulty;

use crate::entity::{
    Entity,
    ai::goal::{
        active_target::ActiveTargetGoal, bow_attack::BowAttackGoal,
        flee_sun::FleeSunGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal,
        restrict_sun::RestrictSunGoal, revenge::RevengeGoal, wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty},
};
use crate::world::World;

/// `AbstractSkeleton.java:51` `HARD_ATTACK_INTERVAL`, the default
/// `reassessWeaponGoal` interval used by `Skeleton`/`Stray`/`WitherSkeleton`
/// (none of which override `getHardAttackInterval`).
pub const DEFAULT_BOW_ATTACK_INTERVAL: i32 = 20;
/// `Bogged.java:117-119` / `Parched.java:57-59` both override
/// `getHardAttackInterval` to `INCREASED_HARD_ATTACK_INTERVAL` (50).
pub const INCREASED_BOW_ATTACK_INTERVAL: i32 = 50;

/// Vanilla `Turtle.BABY_ON_LAND_SELECTOR` (`Turtle.java:76`):
/// `(target, level) -> target.isBaby() && !target.isInWater()`.
///
/// There is no generic "is baby" query on `LivingEntity` (that lives on the
/// `AgeableMob` trait, which this predicate closure cannot downcast to), so
/// this mirrors `AgeableMob::is_baby`'s own check directly: a negative age
/// ticker is vanilla's universal baby marker (`ageable.rs:38`).
fn turtle_baby_on_land(living_entity: &LivingEntity, _world: &World) -> bool {
    living_entity.entity.age.load(Relaxed) < 0 && !living_entity.is_in_water()
}

pub mod bogged;
pub mod parched;
#[allow(clippy::module_inception)]
pub mod skeleton;
pub mod stray;
pub mod wither;

pub struct SkeletonEntityBase {
    pub mob_entity: MobEntity,
}

impl SkeletonEntityBase {
    /// `bow_attack_interval` is `AbstractSkeleton.reassessWeaponGoal`'s
    /// `minAttackInterval` on Hard difficulty (`AbstractSkeleton.java:138-141`):
    /// `getHardAttackInterval()`, 20 by default but 50 for `Bogged`/`Parched`.
    ///
    /// Vanilla recomputes this every time `reassessWeaponGoal` runs (weapon
    /// pickup, load, difficulty change) and also lowers it to
    /// `getAttackInterval()` (40 / 70) off Hard difficulty. `BowAttackGoal`
    /// only takes a fixed interval at construction and exposes no setter, so
    /// this only reproduces the Hard-difficulty value; see the skeleton
    /// family report for the follow-up needed in `bow_attack.rs`.
    pub fn new(entity: Entity, bow_attack_interval: i32) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mob = Self { mob_entity };
        let mob_arc = Arc::new(mob);
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
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // AbstractSkeleton.java:75-87 `registerGoals`. Vanilla has no
            // FloatGoal/SwimGoal entry here (confirmed by the same absence in
            // the sibling `Zombie.java:112-117`): skeletons, like zombies,
            // sink and walk the sea floor rather than float.
            goal_selector.add_goal(2, Box::new(RestrictSunGoal::default()));
            goal_selector.add_goal(3, Box::new(FleeSunGoal::new(1.0)));
            // AbstractSkeleton.java:79 also adds
            // `new AvoidEntityGoal<>(this, Wolf.class, 6.0F, 1.0, 1.2)` at
            // priority 3, but no `AvoidEntityGoal` type exists under
            // `entity/ai/goal/` yet, so it is intentionally left out here.
            //
            // AbstractSkeleton.java:132-149 `reassessWeaponGoal` swaps a single
            // priority-4 slot between the bow and melee goals depending on the
            // held item, re-running on spawn/equip/load. Nothing in this crate
            // currently calls back into mob AI on an equipment change, so both
            // goals are registered permanently at priority 4 instead; they
            // share MOVE|LOOK controls and `BowAttackGoal::can_start`/
            // `should_continue` already gate on holding a bow
            // (`bow_attack.rs`), so only one can ever run at a time and bow
            // is preferred whenever one is held, matching vanilla's effective
            // behaviour even though the mechanism differs.
            goal_selector.add_goal(
                4,
                Box::new(BowAttackGoal::new(1.0, bow_attack_interval, 15.0)),
            );
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.2, false)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
            // AbstractSkeleton.java:86 / Turtle.BABY_ON_LAND_SELECTOR
            // (`Turtle.java:76`): reciprocal chance 10, mustSee = true,
            // mustReach = false, restricted to baby turtles out of water.
            target_selector.add_goal(
                3,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::TURTLE,
                    10,
                    true,
                    false,
                    Some(turtle_baby_on_land),
                )),
            );
        };

        mob_arc
    }
}

impl Mob for SkeletonEntityBase {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        difficulty: &RegionalDifficulty,
    ) {
        // Default armor slots (super.populateDefaultEquipmentSlots)
        if rand::random::<f32>()
            < MobEntity::MAX_WEARING_ARMOR_CHANCE * difficulty.special_multiplier
        {
            let mut armor_type = rand::random_range(0..3);
            for _ in 1..=3 {
                if rand::random::<f32>() < MobEntity::WEARING_ARMOR_UPGRADE_MATERIAL_CHANCE {
                    armor_type += 1;
                }
            }

            let partial_chance = if difficulty.base_difficulty == Difficulty::Hard {
                0.1f32
            } else {
                0.25f32
            };

            let living = &self.mob_entity.living_entity;
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut first = true;

            for slot in &MobEntity::EQUIPMENT_POPULATION_ORDER {
                let current = equipment.get(slot);
                if !first && rand::random::<f32>() < partial_chance {
                    break;
                }
                first = false;
                if current.is_empty()
                    && let Some(item) = MobEntity::get_equipment_for_slot(slot, armor_type)
                {
                    equipment.put(slot, ItemStack::new(1, item));
                }
            }
        }

        // AbstractSkeleton sets BOW on MAIN_HAND
        let living = &self.mob_entity.living_entity;
        let mut equipment = living
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, &Item::BOW));
    }
}
