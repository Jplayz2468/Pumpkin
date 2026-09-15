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
            crate::enchantment::EnchantmentHelper::modify_trident_spin_attack_strength(
                &stack, 0.0,
            );
        if riptide_strength > 0.0 && !Self::is_in_water_or_rain(player) {
            return;
        }

        player
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack, 72000);
    }

    fn on_stopped_using(&self, _stack: &ItemStack, player: &Player) {
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
        let stack_guard = player.inventory().held_item();

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
            player.damage_held_item(1);

            // TridentItem.java:98-108: push velocity is `riptideStrength / dist`, not a
            // `(1.0 + strength * 0.75)` scaled multiplier.
            let f = f64::from(riptide_strength);
            let (yaw, pitch) = player.rotation();
            let f_yaw = f32::to_radians(yaw);
            let f_pitch = f32::to_radians(pitch);

            let vx = f64::from(-f32::sin(f_yaw) * f32::cos(f_pitch));
            let vy = f64::from(-f32::sin(f_pitch));
            let vz = f64::from(f32::cos(f_yaw) * f32::cos(f_pitch));

            let sq = (vx * vx + vy * vy + vz * vz).sqrt();
            if sq > 0.0 {
                let mult = f / sq;
                player.living_entity.entity.velocity.store(Vector3::new(
                    vx * mult,
                    vy * mult,
                    vz * mult,
                ));
            }

            player.living_entity.clear_active_hand();
            return;
        }

        // TridentItem.java:83-95: normal throw. `hurtWithoutBreaking` is applied first so the
        // thrown entity carries the same (now slightly more worn) item stack.
        player.damage_held_item(1);

        let (yaw, pitch) = player.rotation();
        let thrown_stack = player.inventory().held_item().copy_with_count(1);
        let entity = Entity::new(world.clone(), player.position(), &EntityType::TRIDENT);
        // TridentItem.java:89-91: a creative shooter's thrown trident can only be picked back
        // up in creative.
        let pickup = if player.gamemode.load() == GameMode::Creative {
            ArrowPickup::CreativeOnly
        } else {
            ArrowPickup::Allowed
        };
        let trident = TridentEntity::new_shot(
            entity,
            player.get_entity(),
            thrown_stack.clone(),
            pickup,
        );
        trident.set_velocity_from_rotation(pitch, yaw, 0.0, 2.5, 1.0);
        trident.apply_on_projectile_spawned(&thrown_stack);
        world.spawn_entity(Arc::new(trident));

        world.play_sound(
            Sound::ItemTridentThrow,
            pumpkin_data::sound::SoundCategory::Players,
            &player.position(),
        );

        if player.gamemode.load() != GameMode::Creative {
            let inventory = player.inventory();
            let selected_slot = inventory.get_selected_slot() as usize;

            let main_hand_item = inventory.get_slot(selected_slot);
            if main_hand_item.item.id == Item::TRIDENT.id {
                inventory.set_slot(selected_slot, ItemStack::EMPTY.clone());
                player.sync_hand_slot(selected_slot, ItemStack::EMPTY.clone());
            } else {
                let off_hand_slot =
                    pumpkin_inventory::player::player_inventory::PlayerInventory::OFF_HAND_SLOT;
                let off_hand_item = inventory.get_slot(off_hand_slot);
                if off_hand_item.item.id == Item::TRIDENT.id {
                    inventory.set_slot(off_hand_slot, ItemStack::EMPTY.clone());
                    player.sync_hand_slot(off_hand_slot, ItemStack::EMPTY.clone());
                }
            }
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
