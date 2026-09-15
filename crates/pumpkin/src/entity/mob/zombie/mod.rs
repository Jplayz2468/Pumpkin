use super::{Mob, MobEntity};
pub mod brain_lab;

use crate::entity::ai::goal::break_door::BreakDoorGoal;
use crate::entity::ai::goal::destroy_egg::DestroyEggGoal;
use crate::entity::ai::goal::look_around::RandomLookAroundGoal;
use crate::entity::ai::goal::revenge::RevengeGoal;
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::ai::goal::zombie_attack::ZombieAttackGoal;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{Goal, active_target::ActiveTargetGoal, look_at_entity::LookAtEntityGoal},
};
use crate::world::World;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

pub mod drowned;
pub mod husk;
#[allow(clippy::module_inception)]
pub mod zombie;
pub mod zombie_villager;

pub struct ZombieEntityBase {
    pub mob_entity: MobEntity,
    pub can_break_doors: AtomicBool,
    /// `Zombie.inWaterTime`: consecutive ticks with the eyes in water, or -1 when dry.
    in_water_time: AtomicI32,
    /// `Zombie.conversionTime`: ticks left once the conversion has actually begun.
    conversion_time: AtomicI32,
}

/// `Zombie.tick` (Zombie.java:213): 600 ticks with the eyes under water before the
/// conversion starts, then 300 more before the zombie becomes a drowned.
const TICKS_UNDER_WATER_BEFORE_CONVERSION: i32 = 600;
const DROWNED_CONVERSION_TICKS: i32 = 300;

impl ZombieEntityBase {
    pub fn new(entity: Entity) -> Arc<Self> {
        Self::with_can_break_doors(entity, false)
    }

    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let zombie = Self {
            mob_entity,
            can_break_doors: AtomicBool::new(can_break_doors),
            in_water_time: AtomicI32::new(-1),
            conversion_time: AtomicI32::new(-1),
        };
        let mob_arc = Arc::new(zombie);

        // Lab only (`local_safety.zombie_brain_lab`, off by default): vanilla zombies have
        // no brain. See `brain_lab` for why this exists.
        {
            let world = mob_arc.mob_entity.living_entity.entity.world.load_full();
            if brain_lab::enabled(&world) {
                *mob_arc
                    .mob_entity
                    .brain
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(brain_lab::build());
            }
        }

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

            // No float goal, deliberately. `Zombie.registerGoals` (Zombie.java:113) and
            // `addBehaviourGoals` register none: a zombie SINKS. That is the whole reason
            // it can stay under water long enough to convert into a drowned. Giving it
            // one makes it bob on the surface and the conversion never fires.
            if can_break_doors {
                goal_selector.add_goal(1, Box::new(BreakDoorGoal::default()));
            }
            // Vanilla `Zombie.addBehaviourGoals` puts the attack goal at priority 3
            // (`Zombie.java:121`); slot 2 is `SpearUseGoal`, which is not implemented here.
            goal_selector.add_goal(3, ZombieAttackGoal::new(1.0, false));
            goal_selector.add_goal(4, DestroyEggGoal::new(1.0, 3));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            // Vanilla passes mustSee = false for villagers (`Zombie.java:126`), unlike
            // every other zombie target: a zombie homes in on a villager it cannot see,
            // which is what lets it path to one shut inside a house.
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, false),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
            target_selector.add_goal(
                5,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::TURTLE, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn can_break_doors(&self) -> bool {
        self.can_break_doors.load(Ordering::Relaxed)
    }

    pub fn set_can_break_doors(&self, can_break_doors: bool, mob: &dyn Mob) {
        if self
            .can_break_doors
            .swap(can_break_doors, Ordering::Relaxed)
            != can_break_doors
        {
            let mut stopped = {
                let mut goal_selector = self
                    .mob_entity
                    .goals_selector
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if can_break_doors {
                    goal_selector.add_goal(1, Box::new(BreakDoorGoal::default()));
                    Vec::new()
                } else {
                    goal_selector.remove_goals::<BreakDoorGoal>()
                }
            };
            for goal in &mut stopped {
                goal.stop(mob);
            }
        }
    }
}

impl ZombieEntityBase {
    /// `Zombie.isUnderWaterConverting` (Zombie.java:148).
    #[must_use]
    pub fn is_under_water_converting(&self) -> bool {
        self.conversion_time.load(Ordering::Relaxed) >= 0
    }

    /// What this zombie turns into after long enough under water, or `None` if it does
    /// not convert.
    ///
    /// `convertsInWater` is overridden across the family: Zombie.java:208 returns true and
    /// converts to a drowned; Husk.java:77 returns true but converts to a plain zombie
    /// (Husk.java:83); Drowned, ZombieVillager and ZombifiedPiglin all return false.
    fn water_conversion_target(&self) -> Option<&'static EntityType> {
        water_conversion_target_for(self.mob_entity.living_entity.entity.entity_type.resource_name)
    }

    /// `Zombie.startUnderWaterConversion` (Zombie.java:235). The synced flag is what makes
    /// the client shake the zombie while it converts.
    fn start_under_water_conversion(&self, time: i32) {
        self.conversion_time.store(time, Ordering::Relaxed);
        self.get_entity().set_synced_data_compat(
            tracked_data::zombie::CONVERTING_IN_WATER,
            tracked_data::zombie::DATA_DROWNED_CONVERSION_ID,
            true,
        );
    }

    /// `Zombie.doUnderWaterConversion` (Zombie.java:240) -> `convertToZombieType`.
    fn do_under_water_conversion(&self, converted_type: &'static EntityType) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let drowned =
            crate::entity::r#type::from_type(converted_type, pos, &world, uuid::Uuid::new_v4());

        let drowned_base = drowned.get_entity();
        drowned_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        drowned_base.head_yaw.store(entity.head_yaw.load());
        drowned_base.velocity.store(entity.velocity.load());
        // ConversionParams.single(this, true, true) keeps the baby flag and the name.
        drowned_base
            .age
            .store(entity.age.load(Ordering::Relaxed), Ordering::Relaxed);

        if let Some(living) = drowned.get_living_entity() {
            living.set_health(self.mob_entity.living_entity.health.load());
        }
        if let Some(custom_name) = &**entity.custom_name.load() {
            drowned_base.set_custom_name(custom_name.clone());
        }
        {
            let src_equip = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = drowned.get_living_entity() {
                let mut dst_equip = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (slot, item) in &src_equip.equipment {
                    dst_equip.put(slot, item.clone());
                }
            }
        }

        world.spawn_entity(drowned);
        entity.remove();

        // levelEvent 1040 (zombie -> drowned) / 1041 (husk -> zombie): the conversion
        // splash the client plays.
        if !entity.silent.load(Ordering::Relaxed) {
            let sound = if converted_type.id == EntityType::DROWNED.id {
                Sound::EntityZombieConvertedToDrowned
            } else {
                Sound::EntityHuskConvertedToZombie
            };
            world.play_sound(sound, SoundCategory::Hostile, &pos);
        }
    }
}

impl Mob for ZombieEntityBase {
    fn as_zombie_base(&self) -> Option<&ZombieEntityBase> {
        Some(self)
    }

    /// `Zombie.tick` (Zombie.java:213). A zombie whose eyes stay under water for 600
    /// ticks begins converting, and becomes a drowned 300 ticks after that. Leaving the
    /// water before the conversion starts resets the counter outright.
    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() || self.mob_entity.is_no_ai() {
            return;
        }

        if self.is_under_water_converting() {
            if self.conversion_time.fetch_sub(1, Ordering::Relaxed) - 1 < 0
                && let Some(converted_type) = self.water_conversion_target()
            {
                self.do_under_water_conversion(converted_type);
            }
        } else if self.water_conversion_target().is_some() {
            if entity.is_under_water() {
                let time = self.in_water_time.fetch_add(1, Ordering::Relaxed) + 1;
                if time >= TICKS_UNDER_WATER_BEFORE_CONVERSION {
                    self.start_under_water_conversion(DROWNED_CONVERSION_TICKS);
                }
            } else {
                self.in_water_time.store(-1, Ordering::Relaxed);
            }
        }
    }
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

        let weapon_chance = if difficulty.base_difficulty == Difficulty::Hard {
            0.05f32
        } else {
            0.01f32
        };
        if rand::random::<f32>() < weapon_chance {
            let r = rand::random_range(0..6);
            let weapon_item = match r {
                0 => &Item::IRON_SWORD,
                1 => &Item::IRON_SPEAR,
                _ => &Item::IRON_SHOVEL,
            };
            let living = &self.mob_entity.living_entity;
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, weapon_item));
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if self.can_break_doors() {
            nbt.put_bool("CanBreakDoors", true);
        }
        // Zombie.addAdditionalSaveData (Zombie.java:406): -1 means "not converting".
        nbt.put_int(
            "DrownedConversionTime",
            if self.is_under_water_converting() {
                self.conversion_time.load(Ordering::Relaxed)
            } else {
                -1
            },
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(can_break_doors) = nbt.get_bool("CanBreakDoors") {
            self.set_can_break_doors(can_break_doors, self);
        }
        // Zombie.readAdditionalSaveData (Zombie.java:415).
        match nbt.get_int("DrownedConversionTime") {
            Some(time) if time != -1 => self.start_under_water_conversion(time),
            _ => {
                self.conversion_time.store(-1, Ordering::Relaxed);
                self.get_entity().set_synced_data_compat(
                    tracked_data::zombie::CONVERTING_IN_WATER,
                    tracked_data::zombie::DATA_DROWNED_CONVERSION_ID,
                    false,
                );
            }
        }
    }
}

/// See [`ZombieEntityBase::water_conversion_target`].
fn water_conversion_target_for(name: &str) -> Option<&'static EntityType> {
    match name {
        "zombie" => Some(&EntityType::DROWNED),
        "husk" => Some(&EntityType::ZOMBIE),
        _ => None,
    }
}

pub fn is_zombie_family(name: &str) -> bool {
    matches!(
        name,
        "zombie" | "husk" | "drowned" | "zombie_villager" | "zombified_piglin"
    )
}

/// Zombie babies retain IsBaby forever; AgeableMob growth does not apply.
pub fn set_baby<M: Mob + ?Sized>(mob: &M, baby: bool) {
    let living = &mob.get_mob_entity().living_entity;
    let previous_baby = living
        .entity
        .age
        .swap(if baby { -24000 } else { 0 }, Ordering::Relaxed)
        < 0;
    living
        .entity
        .set_synced_data(pumpkin_data::tracked_data::zombie::BABY, baby);
    living.update_attribute(
        &pumpkin_data::attributes::Attributes::MOVEMENT_SPEED,
        |attribute| {
            attribute.remove_modifier("minecraft:baby");
            if baby {
                attribute.add_or_replace_modifier(crate::entity::attributes::Modifier {
                    id: "minecraft:baby".to_owned(),
                    amount: 0.5,
                    operation: crate::entity::attributes::ModifierOperation::MultiplyBase,
                });
            }
        },
    );
    if previous_baby != baby {
        crate::entity::baby_dimensions::refresh(living, baby);
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    /// `Zombie.tick` (Zombie.java:213) counts 600 ticks with the eyes under water before
    /// the conversion begins, then 300 more before the zombie is replaced.
    #[test]
    fn conversion_timings_match_vanilla() {
        assert_eq!(TICKS_UNDER_WATER_BEFORE_CONVERSION, 600);
        assert_eq!(DROWNED_CONVERSION_TICKS, 300);
    }

    /// The conversion target is overridden across the family: a zombie becomes a drowned
    /// (Zombie.java:241), a husk becomes a plain zombie (Husk.java:83), and drowned,
    /// zombie villagers and zombified piglins do not convert at all -- each of those
    /// overrides `convertsInWater` to false.
    #[test]
    fn only_zombies_and_husks_convert_in_water() {
        for (name, expected) in [
            ("zombie", Some(EntityType::DROWNED.id)),
            ("husk", Some(EntityType::ZOMBIE.id)),
            ("drowned", None),
            ("zombie_villager", None),
            ("zombified_piglin", None),
        ] {
            let got = water_conversion_target_for(name).map(|t| t.id);
            assert_eq!(got, expected, "{name} converts to the wrong thing");
        }
    }
}
