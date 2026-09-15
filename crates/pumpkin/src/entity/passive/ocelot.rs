use crate::entity::ageable::{AgeableData, AgeableMob};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, leap_at_target::LeapAtTargetGoal,
        look_at_entity::LookAtEntityGoal, ocelot_attack::OcelotAttackGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::COD, &Item::SALMON];

/// Vanilla `Turtle.BABY_ON_LAND_SELECTOR` (`Turtle.java:76`). Duplicated locally per-mob
/// (see `wolf.rs`'s copy of the same helper); not shared because it lives outside
/// `entity/ai/goal/`.
fn turtle_baby_on_land(living_entity: &LivingEntity, _world: &World) -> bool {
    living_entity.entity.age.load(Ordering::Relaxed) < 0 && !living_entity.is_in_water()
}

/// Represents an Ocelot, a shy passive mob found in jungles.
///
/// Wiki: <https://minecraft.wiki/w/Ocelot>
pub struct OcelotEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub is_trusting: AtomicBool,
}

impl OcelotEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let ocelot = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            is_trusting: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(ocelot);
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

            // Goal selector (matching vanilla Ocelot.registerGoals, Ocelot.java:103-111).
            // NOTE: unlike Wolf/Cat, vanilla Ocelot has no panic/escape-danger goal at all,
            // and no player-avoidance goal (that behavior was removed from Ocelot when Cat
            // was split out in 1.14) -- both are intentionally absent here.
            // 1: FloatGoal
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            // 3: OcelotTemptGoal(0.6, OCELOT_FOOD tag, true)
            goal_selector.add_goal(3, Box::new(TemptGoal::new(0.6, TEMPT_ITEMS)));
            // 7: LeapAtTargetGoal(0.3)
            goal_selector.add_goal(7, Box::new(LeapAtTargetGoal::new(0.3)));
            // 8: OcelotAttackGoal
            goal_selector.add_goal(8, Box::new(OcelotAttackGoal::new()));
            // 9: BreedGoal(0.8)
            goal_selector.add_goal(9, BreedGoal::new(0.8));
            // 10: WaterAvoidingRandomStrollGoal(0.8, 1.0000001E-5F)
            // NOTE: the low target-reroll probability param isn't exposed by
            // `WanderAroundGoal::water_avoiding`; only the speed is ported.
            goal_selector.add_goal(10, Box::new(WanderAroundGoal::water_avoiding(0.8)));
            // 11: LookAtPlayerGoal(10.0) -- vanilla Ocelot has no RandomLookAroundGoal.
            goal_selector.add_goal(
                11,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 10.0),
            );
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Target selector (Ocelot.java:112-113):
            // 1: NearestAttackableTargetGoal<Chicken>(false)
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::CHICKEN, false),
            );
            // 1: NearestAttackableTargetGoal<Turtle>(10, false, false, BABY_ON_LAND_SELECTOR)
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

    pub fn is_trusting(&self) -> bool {
        self.is_trusting.load(Ordering::Relaxed)
    }

    pub fn set_trusting(&self, trusting: bool) {
        self.is_trusting.store(trusting, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data_compat(pumpkin_data::tracked_data::ocelot::TRUSTING,pumpkin_data::tracked_data::ocelot::DATA_TRUSTING, trusting);
    }
}

impl Animal for OcelotEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        item.has_tag(&tag::Item::MINECRAFT_OCELOT_FOOD)
            || item == &Item::COD
            || item == &Item::SALMON
    }
}

impl AgeableMob for OcelotEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Mob for OcelotEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("Trusting", self.is_trusting.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(trusting) = nbt.get_bool("Trusting") {
            self.is_trusting.store(trusting, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::ocelot::BABY_ID, true);
        }
        entity.set_synced_data_compat(
            pumpkin_data::tracked_data::ocelot::TRUSTING,
            pumpkin_data::tracked_data::ocelot::DATA_TRUSTING,
            self.is_trusting.load(Ordering::Relaxed),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let is_food = self.is_food(item_stack);
        let dist_sqr = self
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&player.get_entity().pos.load());

        if !self.is_trusting() && is_food && dist_sqr < 9.0 {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);

            let mut rng = rand::rng();
            if rng.random_range(0..3) == 0 {
                self.set_trusting(true);
                self.get_entity().world.load().send_entity_status(
                    self.get_entity(),
                    EntityStatus::TrustingSucceeded,
                    Some(ActorEventID::TamingSucceeded),
                );
            } else {
                self.get_entity().world.load().send_entity_status(
                    self.get_entity(),
                    EntityStatus::TrustingFailed,
                    Some(ActorEventID::TamingFailed),
                );
            }

            return true;
        }

        self.animal_interact(
            player,
            item_stack,
            pumpkin_data::sound::Sound::EntityOcelotAmbient,
        )
    }
}
