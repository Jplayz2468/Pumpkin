//! Shared BaseContainerBlockEntity lock storage and opening checks.
use super::BlockEntity;
use pumpkin_data::{
    data_component::DataComponent, data_component_impl::LockImpl, item_stack::ItemStack,
};
use pumpkin_inventory::{Inventory, screen_handler::InventoryPlayer};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::{math::vector3::Vector3, text::TextComponent};
use std::sync::Mutex;
#[derive(Default)]
pub struct ContainerLock(Mutex<Option<NbtCompound>>);
impl ContainerLock {
    pub fn read_nbt(&self, nbt: &NbtCompound) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = nbt
            .get_compound("lock")
            .and_then(crate::item::predicate_codec::load);
    }
    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(lock) = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            nbt.put_compound("lock", lock.clone());
        }
    }
    pub fn apply(&self, stack: &ItemStack) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<LockImpl>()
            .and_then(|v| crate::item::predicate_codec::load(&v.predicate));
    }
    pub fn collect(&self, stack: &mut ItemStack) {
        stack.remove_data_component(DataComponent::Lock);
        if let Some(predicate) = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|v| !unlocked(v))
        {
            stack.set_data_component(LockImpl {
                predicate: predicate.clone(),
            });
        }
    }
    pub fn can_open(&self, stack: &ItemStack, spectator: bool) -> bool {
        spectator
            || self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_none_or(|p| crate::item::predicate::matches(p, stack))
    }
}
fn unlocked(predicate: &NbtCompound) -> bool {
    predicate.get("items").is_none()
        && ["count", "components", "predicates"].iter().all(|key| {
            predicate.get(key).is_none_or(|value| {
                value
                    .extract_compound()
                    .is_some_and(|v| v.child_tags.is_empty())
            })
        })
}
pub fn can_open(entity: &dyn BlockEntity, player: &dyn InventoryPlayer) -> bool {
    entity.container_lock().is_none_or(|lock| {
        lock.can_open(
            &player
                .get_inventory()
                .get_stack_in_hand(pumpkin_util::Hand::Right),
            player.is_spectator(),
        )
    })
}
pub fn notify_locked(player: &dyn InventoryPlayer, position: Vector3<f64>, name: TextComponent) {
    use crate::entity::EntityBase;
    if let Some(player) = player
        .as_any()
        .downcast_ref::<crate::entity::player::Player>()
    {
        player.send_system_message_raw(
            &pumpkin_macros::translate_java!(
                pumpkin_data::translation::java::CONTAINER_ISLOCKED,
                name
            ),
            true,
        );
        player.world().play_sound(
            pumpkin_data::sound::Sound::BlockChestLocked,
            pumpkin_data::sound::SoundCategory::Blocks,
            &position,
        );
    }
}
pub fn can_open_inventory(
    inventory: &dyn Inventory,
    player: &dyn InventoryPlayer,
    name: TextComponent,
) -> bool {
    macro_rules! container {
        ($($module:ident::$ty:ident),* $(,)?)=>{$(
            if let Some(entity)=inventory.as_any().downcast_ref::<super::$module::$ty>() {
                if can_open(entity,player) {return true;}
                notify_locked(player,entity.get_position().to_centered_f64(),name);return false;
            }
        )*};
    }
    container!(
        chest::ChestBlockEntity,
        trapped_chest::TrappedChestBlockEntity,
        barrel::BarrelBlockEntity,
        shulker_box::ShulkerBoxBlockEntity,
        furnace::FurnaceBlockEntity,
        blasting_furnace::BlastingFurnaceBlockEntity,
        smoker::SmokerBlockEntity,
        brewing_stand::BrewingStandBlockEntity,
        dispenser::DispenserBlockEntity,
        dropper::DropperBlockEntity,
        hopper::HopperBlockEntity,
        crafter::CrafterBlockEntity
    );
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use pumpkin_data::{data_component_impl::CustomNameImpl, item::Item};
    use pumpkin_util::math::position::BlockPos;
    fn predicate() -> NbtCompound {
        let mut p = NbtCompound::new();
        let mut components = NbtCompound::new();
        components.put_string("minecraft:custom_name", "key".into());
        p.put_compound("components", components);
        p
    }
    fn check<E: BlockEntity>(entity: E) {
        let mut key = ItemStack::new(1, &Item::STICK);
        key.set_data_component(CustomNameImpl {
            name: TextComponent::text("key"),
        });
        let wrong = ItemStack::new(1, &Item::STICK);
        let mut source = ItemStack::new(1, &Item::CHEST);
        source.set_data_component(LockImpl {
            predicate: predicate(),
        });
        entity.apply_components_from_item_stack(&source);
        let lock = entity
            .container_lock()
            .expect("container has shared lock storage");
        assert!(lock.can_open(&key, false));
        assert!(!lock.can_open(&wrong, false));
        assert!(lock.can_open(&wrong, true));
        let mut saved = NbtCompound::new();
        entity.write_nbt(&mut saved);
        assert_eq!(saved.get_compound("lock"), Some(&predicate()));
        assert!(
            saved
                .get_compound("components")
                .is_none_or(|v| v.get("minecraft:lock").is_none())
        );
        let loaded = E::from_nbt(&saved, BlockPos::new(0, 0, 0));
        assert!(!loaded.container_lock().unwrap().can_open(&wrong, false));
        let mut collected = ItemStack::new(1, &Item::AIR);
        loaded.collect_components(&mut collected);
        assert_eq!(
            collected
                .get_data_component::<LockImpl>()
                .unwrap()
                .predicate,
            predicate()
        );
        loaded.apply_components_from_item_stack(&ItemStack::new(1, &Item::CHEST));
        assert!(loaded.container_lock().unwrap().can_open(&wrong, false));
        loaded.collect_components(&mut collected);
        assert!(collected.get_data_component::<LockImpl>().is_none());
    }
    #[test]
    fn all_container_locks_survive_placement_save_reload_and_drop_collection() {
        use super::super::*;
        let p = BlockPos::new(0, 0, 0);
        check(chest::ChestBlockEntity::new(p));
        check(trapped_chest::TrappedChestBlockEntity::new(p));
        check(barrel::BarrelBlockEntity::new(p));
        check(shulker_box::ShulkerBoxBlockEntity::new(p));
        check(furnace::FurnaceBlockEntity::new(p));
        check(blasting_furnace::BlastingFurnaceBlockEntity::new(p));
        check(smoker::SmokerBlockEntity::new(p));
        check(brewing_stand::BrewingStandBlockEntity::new(p));
        check(dispenser::DispenserBlockEntity::new(p));
        check(dropper::DropperBlockEntity::new(p));
        check(crafter::CrafterBlockEntity::new(p));
        check(hopper::HopperBlockEntity::new(
            p,
            pumpkin_data::block_properties::FacingHopper::Down,
        ));
    }
}

#[cfg(test)]
pub(crate) mod menu_tests {
    use super::*;
    use pumpkin_data::{
        data_component_impl::{CustomNameImpl, EquipmentSlot},
        item::Item,
        screen::WindowType,
        statistic::StatisticCategory,
    };
    use pumpkin_inventory::{
        entity_equipment::EntityEquipment, player::player_inventory::PlayerInventory,
        screen_handler::ScreenHandlerFactory,
    };
    use pumpkin_protocol::java::client::play::*;
    use std::{
        any::Any,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    pub struct TestPlayer {
        pub inventory: Arc<PlayerInventory>,
        pub spectator: AtomicBool,
    }
    impl TestPlayer {
        pub fn new() -> Self {
            Self {
                inventory: Arc::new(PlayerInventory::new(
                    Arc::new(Mutex::new(EntityEquipment::new())),
                    Arc::new(rustc_hash::FxHashMap::default()),
                )),
                spectator: AtomicBool::new(false),
            }
        }
    }
    impl InventoryPlayer for TestPlayer {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn drop_item(&self, _item: ItemStack, _retain_ownership: bool) {}
        fn get_inventory(&self) -> Arc<PlayerInventory> {
            self.inventory.clone()
        }
        fn has_infinite_materials(&self) -> bool {
            false
        }
        fn is_creative(&self) -> bool {
            false
        }
        fn is_spectator(&self) -> bool {
            self.spectator.load(Ordering::Relaxed)
        }
        fn experience_level(&self) -> i32 {
            0
        }
        fn add_experience_levels(&self, _levels: i32) {}
        fn enchantment_seed(&self) -> i32 {
            0
        }
        fn set_enchantment_seed(&self, _seed: i32) {}
        fn enqueue_inventory_packet(
            &self,
            _packet: &CSetContainerContent,
            _window_type: Option<WindowType>,
        ) {
        }
        fn enqueue_slot_packet(
            &self,
            _packet: &CSetContainerSlot,
            _window_type: Option<WindowType>,
            _total_slots: usize,
        ) {
        }
        fn enqueue_cursor_packet(&self, _packet: &CSetCursorItem) {}
        fn enqueue_property_packet(&self, _packet: &CSetContainerProperty) {}
        fn enqueue_slot_set_packet(&self, _packet: &CSetPlayerInventory) {}
        fn enqueue_set_held_item_packet(&self, _packet: &CSetSelectedSlot) {}
        fn enqueue_equipment_change(&self, _slot: &EquipmentSlot, _stack: &ItemStack) {}
        fn award_experience(&self, _amount: i32) {}
        fn increment_stat(&self, _category: StatisticCategory, _stat_id: i32, _amount: i32) {}
        fn play_block_sound(&self, _sound: pumpkin_data::sound::Sound, _pitch: f32) {}
    }
    pub fn lock(entity: &dyn BlockEntity, name: &str) {
        let mut p = NbtCompound::new();
        let mut c = NbtCompound::new();
        c.put_string("minecraft:custom_name", name.into());
        p.put_compound("components", c);
        let mut stack = ItemStack::new(1, &Item::CHEST);
        stack.set_data_component(LockImpl { predicate: p });
        entity.apply_components_from_item_stack(&stack);
    }
    pub fn check(factory: &dyn ScreenHandlerFactory, entities: &[&dyn BlockEntity]) {
        for entity in entities {
            lock(*entity, "key");
        }
        let player = TestPlayer::new();
        let mut key = ItemStack::new(1, &Item::STICK);
        key.set_data_component(CustomNameImpl {
            name: TextComponent::text("key"),
        });
        assert!(
            factory
                .create_screen_handler(1, &player.inventory, &player)
                .is_none()
        );
        player.inventory.set_stack(40, key.clone());
        assert!(
            factory
                .create_screen_handler(1, &player.inventory, &player)
                .is_none(),
            "off-hand key must not unlock"
        );
        player.inventory.set_stack(0, key);
        assert!(
            factory
                .create_screen_handler(1, &player.inventory, &player)
                .is_some()
        );
        if entities.len() > 1 {
            lock(entities[1], "other");
            assert!(
                factory
                    .create_screen_handler(1, &player.inventory, &player)
                    .is_none(),
                "second chest lock must be checked"
            );
        }
        player.inventory.set_stack(0, ItemStack::EMPTY.clone());
        player.spectator.store(true, Ordering::Relaxed);
        assert!(
            factory
                .create_screen_handler(1, &player.inventory, &player)
                .is_some(),
            "spectator bypass"
        );
    }
}
