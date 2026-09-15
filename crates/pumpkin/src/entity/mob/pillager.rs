use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        ranged_crossbow_attack::RangedCrossbowAttackGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    living::LivingEntity,
    mob::{
        Mob, MobEntity,
        crossbow_attack_mob::CrossbowAttackMob,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            HoldGroundAttackGoal, ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, Raider,
            RaiderCelebrationGoal, RaiderData, RaiderMoveThroughVillageGoal,
        },
    },
};
use crate::world::World;

pub struct PillagerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    pub is_charging_crossbow: AtomicBool,
    pub inventory: Mutex<Vec<ItemStack>>,
}

impl PillagerEntity {
    pub const INVENTORY_SIZE: usize = 5;

    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_arc = Arc::new(Self {
            mob_entity: MobEntity::new(entity),
            raider_data: RaiderData::default(),
            is_charging_crossbow: AtomicBool::new(false),
            inventory: Mutex::new(Vec::new()),
        });

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

            // Pillager.java:71 `FloatGoal` -> SwimGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Pillager.java:72 `AvoidEntityGoal<>(this, Creaking.class, 8.0F, 1.0, 1.2)`
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::CREAKING, 8.0, 1.0, 1.2)),
            );
            // Raider.java:64 `ObtainRaidLeaderBannerGoal` priority 1
            goal_selector.add_goal(1, Box::new(ObtainRaidLeaderBannerGoal));
            // Pillager.java:73 `Raider.HoldGroundAttackGoal(this, 10.0F)`
            goal_selector.add_goal(2, Box::new(HoldGroundAttackGoal::new(10.0)));
            // Pillager.java:74 `RangedCrossbowAttackGoal<>(this, 1.0, 8.0F)`
            goal_selector.add_goal(3, Box::new(RangedCrossbowAttackGoal::new(1.0, 8.0)));
            // Raider.java:65 `PathfindToRaidGoal<>(this)` priority 3
            goal_selector.add_goal(3, Box::new(PathfindToRaidGoal::default()));
            // PatrollingMonster.java:40 `LongDistancePatrolGoal<>(this, 0.7, 0.595)` priority 4
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            // Raider.java:66 `RaiderMoveThroughVillageGoal(this, 1.05F, 1)` priority 4
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            // Raider.java:67 `RaiderCelebration(this)` priority 5
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            // Pillager.java:75 `RandomStrollGoal(this, 0.6)` priority 8
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.6)));
            // Pillager.java:76 `LookAtPlayerGoal(this, Player.class, 15.0F, 1.0F)` priority 9
            goal_selector.add_goal(
                9,
                Box::new(LookAtEntityGoal::new(
                    mob_weak,
                    &EntityType::PLAYER,
                    15.0,
                    1.0,
                    false,
                )),
            );
            // Pillager.java:77 `LookAtPlayerGoal(this, Mob.class, 15.0F)` priority 10 wants
            // "look at any nearby Mob", but LookAtEntityGoal (ai/goal/look_at_entity.rs, not
            // ours to modify) only supports a single concrete EntityType, not a wildcard class.
            // Kept as the closest available idle-look behaviour; not a faithful port.
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Pillager.java:78 `HurtByTargetGoal(this, Raider.class).setAlertOthers()` priority 1.
            // RevengeGoal (ai/goal/revenge.rs) is the closest existing type but its
            // "alert nearby raiders" behaviour is an explicit TODO in that file (not ours to
            // extend), so struck raiders will retaliate themselves but won't call in allies.
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            // Pillager.java:79 `NearestAttackableTargetGoal<>(this, Player.class, true)`
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            // Pillager.java:80 `NearestAttackableTargetGoal<>(this, AbstractVillager.class, false)`
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
            // Pillager.java:81 `NearestAttackableTargetGoal<>(this, IronGolem.class, true)`
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_charging_crossbow(&self) -> bool {
        self.is_charging_crossbow.load(Ordering::Relaxed)
    }

    pub fn set_charging_crossbow(&self, is_charging: bool) {
        self.is_charging_crossbow
            .store(is_charging, Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::pillager::IS_CHARGING_CROSSBOW, is_charging);
    }

    pub fn add_to_inventory(&self, item: ItemStack) -> Option<ItemStack> {
        let mut inv = self
            .inventory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if inv.len() < Self::INVENTORY_SIZE {
            inv.push(item);
            None
        } else {
            Some(item)
        }
    }

    pub fn drop_inventory(&self) {
        let items = self
            .inventory
            .try_lock()
            .map_or_else(|_| Vec::new(), |mut inv| std::mem::take(&mut *inv));
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        for item in items {
            if !item.is_empty() {
                let item_entity = crate::entity::item::ItemEntity::new(
                    Entity::new(world.clone(), pos, &EntityType::ITEM),
                    item,
                );
                world.spawn_entity(Arc::new(item_entity));
            }
        }
    }
}

impl Mob for PillagerEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_patrolling_monster(&self) -> Option<&dyn PatrollingMonster> {
        Some(self)
    }

    fn as_raider(&self) -> Option<&dyn Raider> {
        Some(self)
    }

    fn as_crossbow_attack_mob(&self) -> Option<&dyn CrossbowAttackMob> {
        Some(self)
    }

    fn mob_init_data_tracker(&self) {
        self.set_charging_crossbow(self.is_charging_crossbow.load(Ordering::Relaxed));
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_raider_nbt(nbt);
        nbt.put_bool("CanPickUpLoot", true);

        let inv = self
            .inventory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !inv.is_empty() {
            let mut items_tag = Vec::new();
            for item in inv.iter() {
                if !item.is_empty() {
                    let mut item_nbt = NbtCompound::new();
                    item.write_item_stack(&mut item_nbt);
                    items_tag.push(NbtTag::Compound(item_nbt));
                }
            }
            if !items_tag.is_empty() {
                nbt.put_list("Inventory", items_tag);
            }
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);

        if let Some(inv_list) = nbt.get_list("Inventory") {
            let mut inv = self
                .inventory
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inv.clear();
            for tag in inv_list {
                if let Some(compound) = tag.extract_compound()
                    && let Some(stack) = ItemStack::read_item_stack(compound)
                {
                    inv.push(stack);
                }
            }
        }
    }

    fn on_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        _source: Option<&dyn EntityBase>,
    ) {
        if self.mob_entity.living_entity.dead.load(Ordering::Relaxed) {
            self.drop_inventory();
        }
    }
}

impl PatrollingMonster for PillagerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }
}

impl Raider for PillagerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityPillagerCelebrate
    }
}

impl CrossbowAttackMob for PillagerEntity {
    fn set_charging_crossbow(&self, is_charging: bool) {
        self.set_charging_crossbow(is_charging);
    }

    fn is_charging_crossbow(&self) -> bool {
        self.is_charging_crossbow()
    }
}
