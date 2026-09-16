use std::any::Any;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::entity::projectile::arrow::ArrowPickup;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_util::GameMode;
use pumpkin_util::math::vector3::Vector3;

pub struct TridentItem;

impl ItemMetadata for TridentItem {
    fn ids() -> Box<[u16]> {
        [Item::TRIDENT.id].into()
    }
}

impl ItemBehaviour for TridentItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let inventory = player.inventory();
        let stack = inventory.held_item();

        // TridentItem.java:131-133: never even start charging a trident that would break on
        // its next use.
        if Self::next_damage_will_break(&stack) {
            return;
        }

        // TridentItem.java:135-137: a Riptide trident refuses to start charging unless the
        // player is already in water or rain.
        let riptide_strength =
            crate::enchantment::EnchantmentHelper::modify_trident_spin_attack_strength(&stack, 0.0);
        if riptide_strength > 0.0 && !Self::is_in_water_or_rain(player) {
            return;
        }

        player
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack, 72000);
    }

    fn on_stopped_using(&self, stack: &ItemStack, player: &Player) {
        let use_ticks = player
            .living_entity
            .item_use_time
            .load(std::sync::atomic::Ordering::Relaxed);
        let use_ticks = 72000 - use_ticks;

        // TridentItem.java:68-71.
        if use_ticks < 10 {
            return;
        }

        let world = player.world();
        let hand = player
            .living_entity
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unwrap_or(pumpkin_util::Hand::Right);
        let stack_guard = player.living_entity.get_stack_in_hand(player, hand);
        if stack_guard.uid != stack.uid {
            player.living_entity.clear_active_hand();
            return;
        }
        let equipment_slot = if hand == pumpkin_util::Hand::Left {
            pumpkin_data::data_component_impl::EquipmentSlot::OFF_HAND
        } else {
            pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND
        };

        // TridentItem.java:73: fully data-driven strength (Riptide), not a raw enchant level.
        let riptide_strength =
            crate::enchantment::EnchantmentHelper::modify_trident_spin_attack_strength(
                &stack_guard,
                0.0,
            );

        // TridentItem.java:74: `!(riptideStrength > 0) || (isInWaterOrRain && !isPassenger)`.
        // When that's false the whole release is a no-op -- no throw, no riptide.
        let can_act = riptide_strength <= 0.0
            || (Self::is_in_water_or_rain(player) && !player.is_passenger());
        if !can_act {
            player.living_entity.clear_active_hand();
            return;
        }

        // TridentItem.java:75-77: never break the trident by throwing/riptiding it.
        if Self::next_damage_will_break(&stack_guard) {
            player.living_entity.clear_active_hand();
            return;
        }

        let sound = crate::enchantment::EnchantmentHelper::trident_sound(&stack_guard);
        let sound = Sound::from_name(sound.strip_prefix("minecraft:").unwrap_or(sound))
            .unwrap_or(Sound::ItemTridentThrow);
        if riptide_strength > 0.0 {
            if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                && let Some(server) = world.server.upgrade()
            {
                let mut event =
                    crate::plugin::api::events::player::player_riptide::PlayerRiptideEvent {
                        player: player_arc,
                        item_name: "trident".to_string(),
                        cancelled: false,
                    };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    player.living_entity.clear_active_hand();
                    return;
                }
            }

            // TridentItem.java:83: `hurtWithoutBreaking(1, player)` -- safe here since the
            // `next_damage_will_break` guard above already ruled out actually breaking it.
            player.increment_stat(
                crate::entity::player::statistics::StatisticCategory::Used,
                Item::TRIDENT.id as i32,
                1,
            );
            player.damage_item_in_slot(&equipment_slot, 1);

            let (yaw, pitch) = player.rotation();
            let impulse = crate::entity::auto_spin::launch(yaw, pitch, riptide_strength);
            let entity = player.get_entity();
            entity.velocity.store(entity.velocity.load() + impulse);
            entity
                .velocity_dirty
                .store(true, std::sync::atomic::Ordering::Relaxed);
            player.living_entity.start_auto_spin_attack(
                20,
                8.0,
                player.living_entity.get_stack_in_hand(player, hand),
            );
            if entity.on_ground.load(std::sync::atomic::Ordering::Relaxed) {
                entity.move_entity(player, Vector3::new(0.0, f64::from(1.199_999_9_f32), 0.0));
            }
            world.play_entity_sound(
                entity,
                sound,
                pumpkin_data::sound::SoundCategory::Players,
                1.0,
                1.0,
            );

            player.living_entity.clear_active_hand();
            return;
        }

        // TridentItem.java:83-95: normal throw. `hurtWithoutBreaking` is applied first so the
        // thrown entity carries the same (now slightly more worn) item stack.
        player.increment_stat(
            crate::entity::player::statistics::StatisticCategory::Used,
            Item::TRIDENT.id as i32,
            1,
        );
        player.damage_item_in_slot(&equipment_slot, 1);

        let (yaw, pitch) = player.rotation();
        let thrown_stack = player
            .living_entity
            .get_stack_in_hand(player, hand)
            .copy_with_count(1);
        let entity = Entity::new(world.clone(), player.position(), &EntityType::TRIDENT);
        // TridentItem.java:89-91: a creative shooter's thrown trident can only be picked back
        // up in creative.
        let pickup = if player.gamemode.load() == GameMode::Creative {
            ArrowPickup::CreativeOnly
        } else {
            ArrowPickup::Allowed
        };
        let trident =
            TridentEntity::new_shot(entity, player.get_entity(), thrown_stack.clone(), pickup);
        trident.set_velocity_from_rotation(pitch, yaw, 0.0, 2.5, 1.0);
        trident.apply_on_projectile_spawned(&thrown_stack);
        let trident = Arc::new(trident);
        world.spawn_entity(trident.clone());

        world.play_entity_sound(
            trident.get_entity(),
            sound,
            pumpkin_data::sound::SoundCategory::Players,
            1.0,
            1.0,
        );

        if player.gamemode.load() != GameMode::Creative {
            let inventory = player.inventory();
            let slot = if hand == pumpkin_util::Hand::Left {
                pumpkin_inventory::player::player_inventory::PlayerInventory::OFF_HAND_SLOT
            } else {
                inventory.get_selected_slot() as usize
            };
            let mut remaining = inventory.get_slot(slot);
            remaining.decrement(1);
            inventory.set_slot(slot, remaining.clone());
            player.sync_hand_slot(slot, remaining);
        }

        player.living_entity.clear_active_hand();
    }

    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TridentItem {
    /// Matches vanilla `Entity.isInWaterOrRain()` (Entity.java:1611-1622): touching water, or
    /// exposed to rain at either the feet or the top of the bounding box.
    fn is_in_water_or_rain(player: &Player) -> bool {
        let entity = player.get_entity();
        if entity.is_in_water() {
            return true;
        }

        let world = player.world();
        let pos = entity.pos.load();
        let feet_pos = pumpkin_util::math::position::BlockPos::floored(pos.x, pos.y, pos.z);
        if world.is_raining_at(&feet_pos) {
            return true;
        }

        let max_y = entity.bounding_box.load().max.y;
        let head_pos = pumpkin_util::math::position::BlockPos::floored(pos.x, max_y, pos.z);
        world.is_raining_at(&head_pos)
    }

    /// Matches vanilla `ItemStack.nextDamageWillBreak()` (ItemStack.java:416-442): a
    /// damageable, non-unbreakable item that's already at (or past) its last durability
    /// point.
    fn next_damage_will_break(stack: &ItemStack) -> bool {
        !stack.is_unbreakable()
            && stack.is_damageable()
            && stack.get_damage() >= stack.get_max_damage().unwrap_or(0) - 1
    }
}
