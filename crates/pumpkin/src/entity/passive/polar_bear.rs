use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal,
        polar_bear_attack_players::PolarBearAttackPlayersGoal, revenge::RevengeGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

/// Vanilla `PolarBear.java:98`: `NearestAttackableTargetGoal<Fox>(10, true, true, (target,
/// level) -> !this.isBaby())`. The predicate closure captures the *bear* (`this`), not the
/// fox target, so it can't be expressed as `ActiveTargetGoal`'s target-side predicate
/// (`Fn(&LivingEntity, &World) -> bool`, which only sees the candidate target). Instead this
/// wraps `ActiveTargetGoal` and applies the "am I an adult" gate on the bear itself before
/// delegating, the same composition `PolarBearAttackPlayersGoal` uses for its own
/// self-side gate.
struct AdultOnlyTargetGoal {
    inner: ActiveTargetGoal,
}

impl Goal for AdultOnlyTargetGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if mob
            .get_entity()
            .age
            .load(std::sync::atomic::Ordering::Relaxed)
            < 0
        {
            return false;
        }
        self.inner.can_start(mob)
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

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

pub struct PolarBearEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub standing: AtomicBool,
}

impl PolarBearEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let polar_bear = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            standing: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(polar_bear);
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

            // Goal selector (matching vanilla PolarBear.registerGoals, PolarBear.java:87-94).
            // 0: FloatGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // 1: PolarBearMeleeAttackGoal(1.25, true) -- a plain MeleeAttackGoal in vanilla
            // too; only its `checkAndPerformAttack` override (raising a paw / playing the
            // warning growl via `setStanding`) is not ported.
            goal_selector.add_goal(1, Box::new(MeleeAttackGoal::new(1.25, true)));
            // 1: PanicGoal(2.0, baby ? PANIC_CAUSES : PANIC_ENVIRONMENTAL_CAUSES)
            // NOTE: EscapeDangerGoal doesn't take a damage-tag filter, so the baby/adult
            // distinction in which damage types trigger a panic is not ported.
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            // 4: FollowParentGoal(1.25)
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Target selector (PolarBear.java:95-99).
            // 1: PolarBearHurtByTargetGoal -- a HurtByTargetGoal(this) in vanilla too;
            // NOTE: its `start`/`alertOther` overrides (a hurt baby bear alerts nearby
            // adults and then clears its own target instead of fighting back, and only
            // non-baby bears are alerted) are not ported -- this always fights back like a
            // plain RevengeGoal, same simplification as Wolf's "TODO: group revenge".
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // 2: PolarBearAttackPlayersGoal (see polar_bear_attack_players.rs)
            target_selector.add_goal(
                2,
                Box::new(PolarBearAttackPlayersGoal::new(&mob_arc.mob_entity)),
            );
            // 3: NearestAttackableTargetGoal<Player>(10, true, false, this::isAngryAt)
            // NOT PORTED: same missing universal-anger system flagged in wolf.rs (no
            // anger/UniversalAnger machinery anywhere in this codebase outside
            // enderman/piglin teleport goals) -- without it there is nothing correct to
            // wire this priority to, so it is left empty rather than always-targeting.
            // 4: NearestAttackableTargetGoal<Fox>(10, true, true, (target, level) ->
            // !this.isBaby()) -- self-side "adult bear only" gate, see AdultOnlyTargetGoal.
            target_selector.add_goal(
                4,
                Box::new(AdultOnlyTargetGoal {
                    inner: ActiveTargetGoal::new(
                        &mob_arc.mob_entity,
                        &EntityType::FOX,
                        10,
                        true,
                        true,
                        None::<fn(&LivingEntity, &World) -> bool>,
                    ),
                }),
            );
            // 5: ResetUniversalAngerTargetGoal(this, false) -- part of the same missing
            // universal-anger system noted at priority 3. Not added.
        };

        mob_arc
    }

    #[must_use]
    pub fn is_standing(&self) -> bool {
        self.standing.load(Ordering::Relaxed)
    }

    pub fn set_standing(&self, standing: bool) {
        self.standing.store(standing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::polar_bear::DATA_STANDING_ID,
            standing,
        );
    }
}

impl AgeableMob for PolarBearEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for PolarBearEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl Mob for PolarBearEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::polar_bear::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::polar_bear::DATA_STANDING_ID,
            self.is_standing(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityPolarBearAmbient)
    }
}
