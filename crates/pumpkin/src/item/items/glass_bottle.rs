use std::any::Any;
use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::area_effect_cloud::AreaEffectCloudEntity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;
use pumpkin_data::block_properties::{WaterCauldronLikeProperties, WaterProperties};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct GlassBottleItem;

impl ItemMetadata for GlassBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::GLASS_BOTTLE.id])
    }
}

/// Replaces one glass bottle in the player's main or off hand with `filled`, following
/// `ItemUtils.createFilledResult` (`ItemUtils.java:16-33`): the last bottle in the stack is
/// swapped in place, otherwise one bottle is consumed from the stack and `filled` is inserted
/// elsewhere in the inventory (or dropped if there is no room). Returns `false` if the player
/// is not holding a glass bottle in either hand.
fn fill_held_bottle(player: &Player, world: &Arc<World>, filled: ItemStack) -> bool {
    let mut held = player.inventory().held_item();
    let mut is_main = true;
    if held.is_empty() || held.item.id != Item::GLASS_BOTTLE.id {
        held = player.inventory().off_hand_item();
        is_main = false;
        if held.is_empty() || held.item.id != Item::GLASS_BOTTLE.id {
            return false;
        }
    }

    if held.item_count == 1 && player.gamemode.load() != pumpkin_util::GameMode::Creative {
        if is_main {
            player.inventory().set_held_item(filled);
        } else {
            player
                .inventory()
                .set_stack_in_hand(pumpkin_util::Hand::Left, filled);
        }
    } else {
        held.decrement_unless_creative(player.gamemode.load(), 1);
        if is_main {
            player.inventory().set_held_item(held);
        } else {
            player
                .inventory()
                .set_stack_in_hand(pumpkin_util::Hand::Left, held);
        }
        let mut stack_to_give = filled;
        let was_added = player.inventory().insert_stack_anywhere(&mut stack_to_give);
        if !was_added && !stack_to_give.is_empty() {
            world.drop_stack(&player.position().to_block_pos(), stack_to_give);
        }
    }
    true
}

impl ItemBehaviour for GlassBottleItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let world = player.world();

        // BottleItem.use (BottleItem.java:31-45): if the player is standing inside a still-alive
        // area-effect cloud created by an ender dragon's breath attack, siphon dragon's breath
        // from it instead of raycasting for water. Area-effect-cloud entities don't track an
        // explicit owner yet, so a dragon-breath cloud is identified the same way the dragon's
        // breath-attack phases construct one: its stored item stack is `DRAGON_BREATH`
        // (see `EnderDragon` strafing/sit-breathing phases).
        let cloud_aabb = player.get_entity().bounding_box.load().expand_all(2.0);
        for candidate in world.get_entities_at_box(&cloud_aabb) {
            if !candidate.get_entity().is_alive() {
                continue;
            }
            let Some(cloud) = candidate.cast_any().downcast_ref::<AreaEffectCloudEntity>() else {
                continue;
            };
            let is_dragon_breath = {
                let stack = cloud
                    .item_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                stack.item.id == Item::DRAGON_BREATH.id
            };
            if !is_dragon_breath {
                continue;
            }

            {
                let mut radius = cloud
                    .radius
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *radius = (*radius - 0.5).clamp(0.0, 32.0);
            }

            world.play_sound(
                Sound::ItemBottleFillDragonbreath,
                SoundCategory::Neutral,
                &player.position(),
            );
            fill_held_bottle(player, &world, ItemStack::new(1, &Item::DRAGON_BREATH));
            return;
        }

        let (start_pos, end_pos) = self.get_start_and_end_pos(player);
        let checker = |pos: &BlockPos, world_inner: &Arc<World>| {
            let state_id = world_inner.get_block_state_id(pos);
            let block = Block::from_state_id(state_id);
            if state_id == Block::AIR.default_state.id {
                return false;
            }
            if block.id == Block::WATER.id {
                // BottleItem.java:46: getPlayerPOVHitResult(..., ClipContext.Fluid.SOURCE_ONLY)
                // only registers a hit on a still-water *source* (level 0), not flowing water.
                return WaterProperties::from_state_id(state_id).level == 0;
            }
            block.is_waterlogged(state_id)
        };

        if let Some((hit_pos, _)) = world.raycast(start_pos, end_pos, checker) {
            world.play_sound(
                Sound::ItemBottleFill,
                SoundCategory::Players,
                &hit_pos.to_f64(),
            );

            fill_held_bottle(player, &world, ItemStack::new(1, &Item::POTION));
        }
    }

    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();

        let is_water_target = block.id == Block::WATER.id || block.id == Block::WATER_CAULDRON.id;

        let check_pos = if is_water_target {
            location
        } else {
            location.offset(face.to_offset())
        };

        let (check_block, check_state_id) = world.get_block_and_state_id(&check_pos);

        if !matches!(check_block.id, BlockId::WATER | BlockId::WATER_CAULDRON) {
            return BlockActionResult::Pass;
        }

        if check_block.id == BlockId::WATER_CAULDRON {
            let mut props = WaterCauldronLikeProperties::from_state_id(check_state_id);
            let new_cauldron = if props.level > 1 {
                props.level -= 1;
                props.to_state_id(check_block)
            } else {
                Block::CAULDRON.default_state.id
            };
            world.set_block_state(&check_pos, new_cauldron, BlockFlags::NOTIFY_ALL);
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::UseCauldron as i32,
                1,
            );
        }

        world.play_sound(
            Sound::ItemBottleFill,
            SoundCategory::Players,
            &check_pos.to_f64(),
        );

        let mut water_bottle = ItemStack::new(1, &Item::POTION);
        if item.item_count == 1 && player.gamemode.load() != pumpkin_util::GameMode::Creative {
            *item = water_bottle;
        } else {
            item.decrement_unless_creative(player.gamemode.load(), 1);
            let was_added = player.inventory().insert_stack_anywhere(&mut water_bottle);
            if !was_added && !water_bottle.is_empty() {
                world.drop_stack(&player.position().to_block_pos(), water_bottle);
            }
        }
        BlockActionResult::Success
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
