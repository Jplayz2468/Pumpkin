use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        destroy_egg::DestroyEggGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, revenge::RevengeGoal,
        wander_around::WanderAroundGoal, zombie_attack::ZombieAttackGoal,
    },
    mob::{Mob, MobEntity, equipment::RegionalDifficulty},
};
use crate::world::World;

pub struct ZombifiedPiglinEntity {
    pub mob_entity: MobEntity,
    anger_time: AtomicI32,
    ticks_until_next_alert: AtomicI32,
}

impl ZombifiedPiglinEntity {
    pub const XP_REWARD: u32 = 5;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let piglin = Self {
            mob_entity,
            anger_time: AtomicI32::new(0),
            ticks_until_next_alert: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(piglin);
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

            // `ZombifiedPiglin` does not override `registerGoals`, so it still
            // inherits the turtle-egg and look-around goals from
            // `Zombie.registerGoals` (Zombie.java:112-115); only the goals
            // below come from its own `addBehaviourGoals`
            // (ZombifiedPiglin.java:71-78).
            // No float goal, deliberately. ZombifiedPiglin.java registers none and there is no
            // `Swim` brain behaviour either, so this mob sinks in vanilla.
            goal_selector.add_goal(4, DestroyEggGoal::new(1.0, 3));
            // Vanilla uses `ZombieAttackGoal(this, 1.0, false)`
            // (ZombifiedPiglin.java:73), not a generic melee goal.
            goal_selector.add_goal(2, ZombieAttackGoal::new(1.0, false));
            // `WaterAvoidingRandomStrollGoal` at priority 7
            // (ZombifiedPiglin.java:74), not a plain stroll at 5.
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::water_avoiding(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
            // Not ported: `SpearUseGoal` (ZombifiedPiglin.java:72) has no
            // equivalent goal type in `entity::ai::goal`.

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // Not ported: vanilla's `NearestAttackableTargetGoal<Player>` gated
            // on `isAngryAt` (ZombifiedPiglin.java:76) and
            // `ResetUniversalAngerTargetGoal` (ZombifiedPiglin.java:77) belong
            // to the `NeutralMob` universal-anger system, which has no
            // equivalent here. This entity instead targets attackers directly
            // in `on_damage`/`mob_tick` below (a simpler, non-goal-based
            // stand-in for that system) — adding a competing goal-based target
            // here would fight with that logic rather than replace it.
        };

        mob_arc
    }

    pub fn is_angry(&self) -> bool {
        self.anger_time.load(Ordering::Relaxed) > 0
    }

    pub fn set_anger_time(&self, ticks: i32) {
        self.anger_time.store(ticks, Ordering::Relaxed);
    }
}

impl Mob for ZombifiedPiglinEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        _difficulty: &RegionalDifficulty,
    ) {
        let living = &self.mob_entity.living_entity;
        let mut equipment = living
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let weapon = if rand::random_range(0..20) == 0 {
            &Item::GOLDEN_SPEAR
        } else {
            &Item::GOLDEN_SWORD
        };
        equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, weapon));
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        let anger = self.anger_time.load(Ordering::Relaxed);
        if anger > 0 {
            nbt.put_int("AngerTime", anger);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(anger) = nbt.get_int("AngerTime") {
            self.anger_time.store(anger, Ordering::Relaxed);
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let anger = self.anger_time.load(Ordering::Relaxed);
        if anger > 0 {
            self.anger_time.store(anger - 1, Ordering::Relaxed);
        }

        if let Some(target) = self.mob_entity.get_target() {
            let next_alert = self.ticks_until_next_alert.load(Ordering::Relaxed);
            if next_alert > 0 {
                self.ticks_until_next_alert
                    .store(next_alert - 1, Ordering::Relaxed);
            } else {
                self.ticks_until_next_alert
                    .store(rand::random_range(80..120), Ordering::Relaxed);
                let world = entity.world.load();
                let my_pos = entity.pos.load();
                let nearby_zombies = world.get_nearby_entities(my_pos, 35.0);
                for (_uuid, other) in nearby_zombies {
                    if other.get_entity().entity_id != entity.entity_id
                        && other.get_entity().entity_type == &EntityType::ZOMBIFIED_PIGLIN
                        && let Some(mob) = other.get_mob()
                    {
                        mob.get_mob_entity().set_target(Some(target.clone()));
                    }
                }
            }
        }
    }

    fn on_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        source: Option<&dyn EntityBase>,
    ) {
        let anger_ticks = rand::random_range(400..780);
        self.set_anger_time(anger_ticks);

        if let Some(attacker) = source
            && let Some(player) = attacker.get_player()
        {
            let world = self.mob_entity.living_entity.entity.world.load();
            if let Some(player_arc) = world.get_player_by_id(player.living_entity.entity.entity_id)
            {
                self.mob_entity.set_target(Some(player_arc));
            }
        }
    }
}
