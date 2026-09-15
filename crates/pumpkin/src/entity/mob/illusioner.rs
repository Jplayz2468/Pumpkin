use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal,
        bow_attack::BowAttackGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{
        Mob, MobEntity,
        evoker::IllagerSpell,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, Raider, RaiderCelebrationGoal,
            RaiderData, RaiderMoveThroughVillageGoal,
        },
    },
};
use crate::world::World;

/// Represents an Illusioner, a rare spellcasting illager that fights at range with a bow
/// while casting invisibility ("mirror image") and blindness spells.
///
/// Wiki: <https://minecraft.wiki/w/Illusioner>
pub struct IllusionerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    spell_casting_tick_count: AtomicI32,
    current_spell: AtomicU8,
}

impl IllusionerEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let illusioner = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            spell_casting_tick_count: AtomicI32::new(0),
            current_spell: AtomicU8::new(IllagerSpell::None as u8),
        };
        let mob_arc = Arc::new(illusioner);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let illusioner_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Illusioner.java:65 `FloatGoal` -> SwimGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Illusioner.java:66 `SpellcasterIllager.SpellcasterCastingSpellGoal()`
            goal_selector.add_goal(
                1,
                Box::new(IllusionerCastingSpellGoal::new(illusioner_weak.clone())),
            );
            // Raider.java:64 `ObtainRaidLeaderBannerGoal` priority 1
            goal_selector.add_goal(1, Box::new(ObtainRaidLeaderBannerGoal));
            // Illusioner.java:67 `AvoidEntityGoal<>(this, Creaking.class, 8.0F, 1.0, 1.2)` —
            // was entirely missing before.
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::new(&EntityType::CREAKING, 8.0, 1.0, 1.2)),
            );
            // Raider.java:65 `PathfindToRaidGoal<>(this)` priority 3
            goal_selector.add_goal(3, Box::new(PathfindToRaidGoal::default()));
            // Illusioner.java:68 `Illusioner.IllusionerMirrorSpellGoal()` — was entirely
            // missing before (no spellcasting of any kind existed for this mob).
            goal_selector.add_goal(
                4,
                Box::new(IllusionerMirrorSpellGoal::new(illusioner_weak.clone())),
            );
            // Raider.java:66 `RaiderMoveThroughVillageGoal(this, 1.05F, 1)` priority 4
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            // PatrollingMonster.java:40 `LongDistancePatrolGoal<>(this, 0.7, 0.595)` priority 4
            // — was entirely missing before.
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            // Illusioner.java:69 `Illusioner.IllusionerBlindnessSpellGoal()` — was entirely
            // missing before.
            goal_selector.add_goal(5, Box::new(IllusionerBlindnessSpellGoal::new(illusioner_weak)));
            // Raider.java:67 `RaiderCelebration(this)` priority 5
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            // Illusioner.java:70 `RangedBowAttackGoal<>(this, 0.5, 20, 15.0F)` — was entirely
            // missing before; the old goal list had a `HoldGroundAttackGoal` here instead,
            // which vanilla's Illusioner never registers at all (only Pillager and Vindicator
            // do), so it has been removed rather than kept alongside the bow attack.
            goal_selector.add_goal(6, Box::new(BowAttackGoal::new(0.5, 20, 15.0)));
            // Illusioner.java:71 `RandomStrollGoal(this, 0.6)`
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.6)));
            // Illusioner.java:72 `LookAtPlayerGoal(this, Player.class, 3.0F, 1.0F)`
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
            // Illusioner.java:73 `LookAtPlayerGoal(this, Mob.class, 8.0F)` priority 10 wants
            // "look at any nearby Mob"; LookAtEntityGoal (ai/goal/look_at_entity.rs, not ours
            // to modify) only supports one concrete EntityType. Kept as closest idle fallback.
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Illusioner.java:74 `HurtByTargetGoal(this, Raider.class).setAlertOthers()`
            // priority 1 — was entirely missing before. RevengeGoal (ai/goal/revenge.rs) is
            // the closest existing type, but its "alert nearby raiders" behaviour is an
            // explicit TODO in that file (not ours to extend), so a struck illusioner
            // retaliates itself but won't call in allies.
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // Illusioner.java:75 `NearestAttackableTargetGoal<>(this, Player.class,
            // true).setUnseenMemoryTicks(300)`. `ActiveTargetGoal::with_default` has no way to
            // configure unseen-memory ticks (that constant lives on the private
            // `TrackTargetGoal` it builds internally, in the restricted active_target.rs), so
            // the 300-tick memory extension can't be ported without touching that file.
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            // Illusioner.java:76 `NearestAttackableTargetGoal<>(this, AbstractVillager.class,
            // false).setUnseenMemoryTicks(300)`
            target_selector.add_goal(
                3,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::VILLAGER,
                    10,
                    false,
                    false,
                    Some(|_target: &LivingEntity, _world: &World| true),
                )),
            );
            // Illusioner.java:77 `NearestAttackableTargetGoal<>(this, IronGolem.class,
            // false).setUnseenMemoryTicks(300)`
            target_selector.add_goal(
                3,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::IRON_GOLEM,
                    10,
                    false,
                    false,
                    Some(|_target: &LivingEntity, _world: &World| true),
                )),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_casting_spell(&self) -> bool {
        self.spell_casting_tick_count.load(Ordering::Relaxed) > 0
    }

    pub fn set_is_casting_spell(&self, spell: IllagerSpell) {
        self.current_spell.store(spell as u8, Ordering::Relaxed);
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_synced_data(
            pumpkin_data::tracked_data::illusioner::SPELL_CASTING_ID,
            spell as u8 as i8,
        );
    }

    #[must_use]
    pub fn get_spell_casting_time(&self) -> i32 {
        self.spell_casting_tick_count.load(Ordering::Relaxed)
    }

    pub fn set_spell_casting_time(&self, ticks: i32) {
        self.spell_casting_tick_count
            .store(ticks, Ordering::Relaxed);
    }
}

impl Mob for IllusionerEntity {
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
        nbt.put_int("SpellTicks", self.get_spell_casting_time());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
        if let Some(ticks) = nbt.get_int("SpellTicks") {
            self.set_spell_casting_time(ticks);
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let ticks = self.spell_casting_tick_count.load(Ordering::Relaxed);
        if ticks > 0 {
            self.spell_casting_tick_count
                .store(ticks - 1, Ordering::Relaxed);
        }
    }

    /// Illusioner.java:84-90 `finalizeSpawn`: unconditionally equips a bow. This does not go
    /// through the weighted-random `populate_default_equipment_slots` machinery (mod.rs:1029)
    /// at all in vanilla, so it's ported as a direct equip here instead of overriding that hook.
    fn mob_finalize_spawn(&self, _reason: crate::entity::spawn::SpawnReason) {
        let living = &self.mob_entity.living_entity;
        let stack = ItemStack::new(1, &Item::BOW);
        {
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment.put(&EquipmentSlot::MAIN_HAND, stack.clone());
        }
        living.send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, stack)]);
    }
}

impl PatrollingMonster for IllusionerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }
}

impl Raider for IllusionerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        // Illusioner.java:131-133: `getCelebrateSound()` deliberately returns
        // `SoundEvents.ILLUSIONER_AMBIENT`, not a dedicated celebrate sound (Illusioner has
        // none). Previously this incorrectly returned the Evoker's celebrate sound.
        Sound::EntityIllusionerAmbient
    }
}

/// SpellcasterIllager.java:144-173 `SpellcasterCastingSpellGoal`, used directly by Illusioner
/// (Illusioner.java:66) with no subclass override, unlike Evoker's.
struct IllusionerCastingSpellGoal {
    illusioner: Weak<IllusionerEntity>,
}

impl IllusionerCastingSpellGoal {
    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self { illusioner }
    }
}

impl Goal for IllusionerCastingSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.illusioner
            .upgrade()
            .is_some_and(|i| i.is_casting_spell())
    }

    fn should_continue(&self, _mob: &dyn Mob) -> bool {
        self.illusioner
            .upgrade()
            .is_some_and(|i| i.is_casting_spell())
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_is_casting_spell(IllagerSpell::None);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if let Some(target) = mob.get_mob_entity().get_target() {
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Illusioner.java:253-283 `IllusionerMirrorSpellGoal`: grants itself 60s of invisibility
/// ("mirror image"), as long as it doesn't already have the effect.
struct IllusionerMirrorSpellGoal {
    illusioner: Weak<IllusionerEntity>,
    warmup_delay: i32,
    next_attack_tick: i32,
}

impl IllusionerMirrorSpellGoal {
    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self {
            illusioner,
            warmup_delay: 0,
            next_attack_tick: 0,
        }
    }
}

impl Goal for IllusionerMirrorSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(illusioner) = self.illusioner.upgrade() else {
            return false;
        };
        // SpellcasterUseSpellGoal.canUse (SpellcasterIllager.java:180-187): needs a live
        // target, not already casting, and past its own cooldown.
        let entity = &illusioner.mob_entity.living_entity.entity;
        let has_target = illusioner
            .mob_entity
            .get_target()
            .is_some_and(|t| t.get_entity().is_alive());
        if !has_target || illusioner.is_casting_spell() {
            return false;
        }
        if entity.tick_count.load(Ordering::Relaxed) < self.next_attack_tick {
            return false;
        }
        // Illusioner.java:256: `!hasEffect(INVISIBILITY)`.
        !illusioner
            .mob_entity
            .living_entity
            .has_effect(&StatusEffect::INVISIBILITY)
    }

    fn should_continue(&self, _mob: &dyn Mob) -> bool {
        self.warmup_delay > 0
    }

    fn start(&mut self, _mob: &dyn Mob) {
        // SpellcasterUseSpellGoal.start (default getCastWarmupTime() == 20).
        self.warmup_delay = 20;
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_spell_casting_time(20);
            let age = illusioner
                .mob_entity
                .living_entity
                .entity
                .tick_count
                .load(Ordering::Relaxed);
            self.next_attack_tick = age + 340;
            illusioner.set_is_casting_spell(IllagerSpell::Disappear);
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerPrepareMirror);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.warmup_delay -= 1;
        if self.warmup_delay == 0
            && let Some(illusioner) = self.illusioner.upgrade()
        {
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerCastSpell);
            illusioner.mob_entity.living_entity.add_effect(Effect {
                effect_type: &StatusEffect::INVISIBILITY,
                duration: 1200,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }
}

/// Illusioner.java:202-251 `IllusionerBlindnessSpellGoal`: blinds the current target for 20s,
/// gated to only ever fire once per distinct target and only above Normal difficulty.
struct IllusionerBlindnessSpellGoal {
    illusioner: Weak<IllusionerEntity>,
    warmup_delay: i32,
    next_attack_tick: i32,
    last_target_id: i32,
}

impl IllusionerBlindnessSpellGoal {
    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self {
            illusioner,
            warmup_delay: 0,
            next_attack_tick: 0,
            last_target_id: 0,
        }
    }
}

impl Goal for IllusionerBlindnessSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(illusioner) = self.illusioner.upgrade() else {
            return false;
        };
        let entity = &illusioner.mob_entity.living_entity.entity;
        let Some(target) = illusioner.mob_entity.get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() || illusioner.is_casting_spell() {
            return false;
        }
        if entity.tick_count.load(Ordering::Relaxed) < self.next_attack_tick {
            return false;
        }
        if target.get_entity().entity_id == self.last_target_id {
            return false;
        }
        // Illusioner.java:214: `getCurrentDifficultyAt(pos).isHarderThan(NORMAL.ordinal())`.
        // Vanilla's regional difficulty is a scaled float that can exceed the base world
        // difficulty over time near a player; that scaling isn't ported here, so this checks
        // the world's base difficulty setting directly (Hard only) as the closest
        // approximation without inventing new regional-difficulty plumbing.
        let world = entity.world.load();
        world.level_info.load().difficulty == Difficulty::Hard
    }

    fn should_continue(&self, _mob: &dyn Mob) -> bool {
        self.warmup_delay > 0
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.warmup_delay = 20;
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_spell_casting_time(20);
            let age = illusioner
                .mob_entity
                .living_entity
                .entity
                .tick_count
                .load(Ordering::Relaxed);
            self.next_attack_tick = age + 180;
            if let Some(target) = illusioner.mob_entity.get_target() {
                self.last_target_id = target.get_entity().entity_id;
            }
            illusioner.set_is_casting_spell(IllagerSpell::Blindness);
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerPrepareBlindness);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.warmup_delay -= 1;
        if self.warmup_delay == 0
            && let Some(illusioner) = self.illusioner.upgrade()
            && let Some(target) = illusioner.mob_entity.get_target()
            && let Some(target_living) = target.get_living_entity()
        {
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerCastSpell);
            target_living.add_effect(Effect {
                effect_type: &StatusEffect::BLINDNESS,
                duration: 400,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }
}
