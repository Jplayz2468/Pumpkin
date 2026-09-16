use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, run_around_like_crazy::RunAroundLikeCrazyGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

pub struct SkeletonHorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl SkeletonHorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(horse);
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

            // SkeletonHorse.addBehaviourGoals (SkeletonHorse.java:66-68) is overridden to an
            // *empty* body, unlike Horse/Donkey/Mule. That means SkeletonHorse gets NONE of
            // AbstractHorse.addBehaviourGoals's FloatGoal(0)/MountPanicGoal(1)/TemptGoal(3)
            // (AbstractHorse.java:147-151) — no swimming, no panic, and it can't be tempted
            // with food. It still gets the rest of the base list from registerGoals itself
            // (AbstractHorse.java:134-145, not overridden): RunAroundLikeCrazyGoal(1),
            // BreedGoal(2), FollowParentGoal(4), WaterAvoidingRandomStrollGoal(6),
            // LookAtPlayerGoal(7), RandomLookAroundGoal(8). Previously this file had the
            // inverse mistake: it kept SwimGoal/EscapeDangerGoal (which vanilla drops) and
            // dropped RunAroundLikeCrazyGoal/BreedGoal/FollowParentGoal (which vanilla keeps).
            goal_selector.add_goal(1, Box::new(RunAroundLikeCrazyGoal::new(1.2)));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
            // RandomStandGoal is still added at priority 9 (canPerformRearing() not
            // overridden by SkeletonHorse). Not wired in: see the AmbientStandGoal note in
            // horse.rs — that goal has no public constructor and its `can_start` is
            // permanently stubbed to `false`.
            //
            // Also not ported: SkeletonHorse.setTrap's dynamic `SkeletonTrapGoal` insertion
            // at priority 1 (SkeletonHorse.java:161-169), which drives the "skeleton horse
            // trap" mechanic (lightning strikes summon skeleton riders). That needs a
            // SkeletonTrapGoal port plus the trap-lightning spawn logic in
            // finalizeSpawn/thunder handling, which doesn't exist in this codebase yet and is
            // well beyond a goal-list port.
        };

        mob_arc
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_tame(&self) -> bool {
        self.has_flag(FLAG_TAME)
    }

    pub fn set_tame(&self, val: bool) {
        self.set_flag(FLAG_TAME, val);
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }
}

impl AgeableMob for SkeletonHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for SkeletonHorseEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl Mob for SkeletonHorseEntity {
    fn is_saddled(&self) -> bool {
        SkeletonHorseEntity::is_saddled(self)
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    // See HorseEntity::is_tamed (crates/pumpkin/src/entity/passive/horse.rs) for why this
    // override is required for RunAroundLikeCrazyGoal to behave correctly.
    fn is_tamed(&self) -> bool {
        self.is_tame()
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(tame) = nbt.get_bool("Tame") {
            self.set_tame(tame);
        }
        if let Some(temper) = nbt.get_int("Temper") {
            self.temper.store(temper, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(
                pumpkin_data::tracked_data::skeleton_horse::DATA_BABY_ID,
                true,
            );
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_tame() && item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityHorseSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if !self.is_baby() {
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        self.animal_interact(player, item_stack, Sound::EntitySkeletonHorseAmbient)
    }
}
