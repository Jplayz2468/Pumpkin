use crate::entity::ageable::{AgeableData, AgeableMob};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_owner::FollowOwnerGoal, leap_at_target::LeapAtTargetGoal,
        look_at_entity::LookAtEntityGoal, ocelot_attack::OcelotAttackGoal,
        sit_when_ordered_to::SitWhenOrderedToGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity},
    passive::{
        animal::Animal,
        tamable::{TamableAnimal, TamableData},
    },
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::COD, &Item::SALMON];

/// Vanilla `Turtle.BABY_ON_LAND_SELECTOR` (`Turtle.java:76`): `target.isBaby() &&
/// !target.isInWater()`. Duplicated locally per-mob (see `wolf.rs`'s copy of the same
/// helper); not shared because it lives outside `entity/ai/goal/`.
fn turtle_baby_on_land(living_entity: &LivingEntity, _world: &World) -> bool {
    living_entity.entity.age.load(Ordering::Relaxed) < 0 && !living_entity.is_in_water()
}

fn get_dye_color_from_item(item: &Item) -> Option<u8> {
    let key = item.registry_key;
    if key.contains("white") {
        Some(0)
    } else if key.contains("orange") {
        Some(1)
    } else if key.contains("magenta") {
        Some(2)
    } else if key.contains("light_blue") {
        Some(3)
    } else if key.contains("yellow") {
        Some(4)
    } else if key.contains("lime") {
        Some(5)
    } else if key.contains("pink") {
        Some(6)
    } else if key.contains("light_gray") {
        Some(8)
    } else if key.contains("gray") {
        Some(7)
    } else if key.contains("cyan") {
        Some(9)
    } else if key.contains("purple") {
        Some(10)
    } else if key.contains("blue") {
        Some(11)
    } else if key.contains("brown") {
        Some(12)
    } else if key.contains("green") {
        Some(13)
    } else if key.contains("red") {
        Some(14)
    } else if key.contains("black") {
        Some(15)
    } else {
        None
    }
}

pub struct CatEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicU8,
    pub sound_variant: AtomicU8,
    pub collar_color: AtomicU8,
    pub tamable_data: TamableData,
    pub is_lying: AtomicBool,
    pub relax_state_one: AtomicBool,
}

impl CatEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let cat = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicU8::new(1),       // Default to black
            sound_variant: AtomicU8::new(0), // Default to classic
            collar_color: AtomicU8::new(14), // Default to red
            tamable_data: TamableData::default(),
            is_lying: AtomicBool::new(false),
            relax_state_one: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(cat);
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

            // Goal selector (matching vanilla Cat.registerGoals, Cat.java:105-122):
            // 1: FloatGoal
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            // 1: TamableAnimalPanicGoal(1.5)
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.5));
            // 2: SitWhenOrderedToGoal
            goal_selector.add_goal(2, Box::new(SitWhenOrderedToGoal::new()));
            // 3: CatRelaxOnOwnerGoal -- NOT PORTED. Needs "owner is lying in a bed and cat is
            // tamed" tracking (bed-sleep state + owner lookup) that has no equivalent
            // machinery here yet. Missing.
            // 4: CatTemptGoal(0.6, CAT_FOOD tag, true)
            goal_selector.add_goal(4, Box::new(TemptGoal::new(0.6, TEMPT_ITEMS)));
            // 5: CatLieOnBedGoal(1.1, 8) -- NOT PORTED. Needs bed-block search + navigate-to
            // machinery; skipped, see report.
            // 6: FollowOwnerGoal(1.0, 10.0, 5.0)
            goal_selector.add_goal(6, FollowOwnerGoal::new(1.0, 10.0, 5.0));
            // 7: CatSitOnBlockGoal(0.8) -- NOT PORTED. Needs chest/furnace-block search +
            // navigate-to machinery; skipped, see report.
            // 8: LeapAtTargetGoal(0.3)
            goal_selector.add_goal(8, Box::new(LeapAtTargetGoal::new(0.3)));
            // 9: OcelotAttackGoal
            goal_selector.add_goal(9, Box::new(OcelotAttackGoal::new()));
            // 10: BreedGoal(0.8)
            goal_selector.add_goal(10, BreedGoal::new(0.8));
            // 11: WaterAvoidingRandomStrollGoal(0.8, 1.0000001E-5F)
            // NOTE: the low target-reroll probability param isn't exposed by
            // `WanderAroundGoal::water_avoiding`; only the speed is ported.
            goal_selector.add_goal(11, Box::new(WanderAroundGoal::water_avoiding(0.8)));
            // 12: LookAtPlayerGoal(10.0) -- vanilla Cat has no RandomLookAroundGoal.
            goal_selector.add_goal(
                12,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 10.0),
            );
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Target selector (Cat.java:120-121):
            // 1: NonTameRandomTargetGoal<Rabbit>(false, null)
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::RABBIT, false),
            );
            // 1: NonTameRandomTargetGoal<Turtle>(false, BABY_ON_LAND_SELECTOR)
            target_selector.add_goal(
                1,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::TURTLE,
                    10,
                    false,
                    false,
                    Some(turtle_baby_on_land),
                )),
            );
        };

        mob_arc
    }

    pub fn get_tame_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_in_sitting_pose() {
            flags |= 0x01;
        }
        if self.is_tame() {
            flags |= 0x04;
        }
        flags
    }

    pub fn is_sitting(&self) -> bool {
        self.is_in_sitting_pose()
    }

    pub fn is_lying(&self) -> bool {
        self.is_lying.load(Ordering::Relaxed)
    }

    pub fn is_relax_state_one(&self) -> bool {
        self.relax_state_one.load(Ordering::Relaxed)
    }

    pub fn get_collar_color(&self) -> u8 {
        self.collar_color.load(Ordering::Relaxed)
    }

    pub fn set_collar_color(&self, color: u8) {
        self.collar_color.store(color, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::CAT_COLLAR_COLOR,
            VarInt(color as i32),
        );
    }

    pub fn set_sitting(&self, sitting: bool) {
        self.set_in_sitting_pose(sitting);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::TAMEABLE_FLAGS,
            self.get_tame_flags(),
        );
    }

    pub fn set_tame(&self, tame: bool, owner: Option<Uuid>) {
        self.tamable_data.is_tame.store(tame, Ordering::Relaxed);
        self.set_owner(owner);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::TAMEABLE_FLAGS,
            self.get_tame_flags(),
        );
        entity.set_synced_data(pumpkin_data::tracked_data::cat::OWNER_UUID, owner);
    }

    pub fn set_variant(&self, variant: u8) {
        self.variant.store(variant, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::cat::CAT_VARIANT,
            pumpkin_data::tracked_data::cat::DATA_VARIANT_ID,
            VarInt(variant as i32),
        );
    }

    pub fn set_lying(&self, lying: bool) {
        self.is_lying.store(lying, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::cat::IS_LYING, lying);
    }

    pub fn set_relax_state_one(&self, relax: bool) {
        self.relax_state_one.store(relax, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::cat::RELAX_STATE_ONE, relax);
    }

    pub fn play_eating_sound(&self) {
        let mob_entity = self.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.play_sound(
            pumpkin_data::sound::Sound::EntityCatEat,
            pumpkin_data::sound::SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }
}

impl Animal for CatEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        item.has_tag(&tag::Item::MINECRAFT_CAT_FOOD) || item == &Item::COD || item == &Item::SALMON
    }
}

impl TamableAnimal for CatEntity {
    fn get_tamable_data(&self) -> &TamableData {
        &self.tamable_data
    }
}

impl AgeableMob for CatEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Mob for CatEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_tamable(&self) -> Option<&dyn TamableAnimal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        let variant_str = match self.variant.load(Ordering::Relaxed) {
            0 => "minecraft:all_black",
            1 => "minecraft:black",
            2 => "minecraft:british_shorthair",
            3 => "minecraft:calico",
            4 => "minecraft:jellie",
            5 => "minecraft:persian",
            6 => "minecraft:ragdoll",
            7 => "minecraft:red",
            8 => "minecraft:siamese",
            10 => "minecraft:white",
            _ => "minecraft:tabby",
        };
        nbt.put_string("variant", variant_str.to_string());
        nbt.put_string("sound_variant", "minecraft:classic".to_string());
        nbt.put_byte(
            "CollarColor",
            self.collar_color.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("variant") {
            let variant = match variant_str
                .strip_prefix("minecraft:")
                .unwrap_or(variant_str)
            {
                "all_black" => 0,
                "black" => 1,
                "british_shorthair" => 2,
                "calico" => 3,
                "jellie" => 4,
                "persian" => 5,
                "ragdoll" => 6,
                "red" => 7,
                "siamese" => 8,
                "white" => 10,
                _ => 9,
            };
            self.variant.store(variant, Ordering::Relaxed);
        }
        if let Some(collar) = nbt.get_byte("CollarColor") {
            self.collar_color.store(collar as u8, Ordering::Relaxed);
        } else if let Some(collar_int) = nbt.get_int("CollarColor") {
            self.collar_color.store(collar_int as u8, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        let variant = match name.strip_prefix("minecraft:").unwrap_or(name) {
            "all_black" => 0,
            "black" => 1,
            "british_shorthair" => 2,
            "calico" => 3,
            "jellie" => 4,
            "persian" => 5,
            "ragdoll" => 6,
            "red" => 7,
            "siamese" => 8,
            "white" => 10,
            _ => 9,
        };
        self.variant.store(variant, Ordering::Relaxed);
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::cat::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::TAMEABLE_FLAGS,
            self.get_tame_flags(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::OWNER_UUID,
            self.get_owner(),
        );
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::cat::CAT_VARIANT,
            pumpkin_data::tracked_data::cat::DATA_VARIANT_ID,
            VarInt(self.variant.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::IS_LYING,
            self.is_lying.load(Ordering::Relaxed),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::RELAX_STATE_ONE,
            self.relax_state_one.load(Ordering::Relaxed),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::CAT_COLLAR_COLOR,
            VarInt(self.collar_color.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::cat::SOUND_VARIANT,
            VarInt(self.sound_variant.load(Ordering::Relaxed) as i32),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        let is_food = self.is_food(item_stack);

        if self.is_tame() {
            if self.get_owner_uuid() == Some(player.gameprofile.id) {
                if item.has_tag(&tag::Item::MINECRAFT_CAT_COLLAR_DYES)
                    || item.has_tag(&tag::Item::C_DYES)
                {
                    if let Some(color) = get_dye_color_from_item(item)
                        && color != self.get_collar_color()
                    {
                        self.set_collar_color(color);
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        return true;
                    }
                } else if is_food
                    && self.mob_entity.living_entity.health.load()
                        < self.mob_entity.living_entity.get_max_health()
                {
                    item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    self.mob_entity.living_entity.heal(2.0);
                    self.play_eating_sound();
                    return true;
                }

                let parent_interaction = self.mob_entity.mob_interact(player, item_stack);
                if !parent_interaction {
                    self.set_sitting(!self.is_sitting());
                    return true;
                }
                return parent_interaction;
            }
        } else if is_food {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            self.play_eating_sound();

            let mut rng = rand::rng();
            if rng.random_range(0..3) == 0 {
                self.set_tame(true, Some(player.gameprofile.id));
                self.set_sitting(true);
                self.get_entity().world.load().send_entity_status(
                    self.get_entity(),
                    EntityStatus::TamingSucceeded,
                    Some(ActorEventID::TamingSucceeded),
                );
            } else {
                self.get_entity().world.load().send_entity_status(
                    self.get_entity(),
                    EntityStatus::TamingFailed,
                    Some(ActorEventID::TamingFailed),
                );
            }

            return true;
        }

        self.mob_entity.mob_interact(player, item_stack)
    }
}
