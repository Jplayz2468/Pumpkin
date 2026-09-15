use std::any::Any;
use std::sync::Arc;

use crate::block::entities::brushable_block::BrushableBlockBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::entity::item::ItemEntity;
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;
use pumpkin_data::block_properties::SuspiciousSandLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockDirection, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct BrushItem;

impl ItemMetadata for BrushItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::BRUSH.id])
    }
}
fn get_archaeology_loot(is_sand: bool, location: BlockPos, world: &World) -> ItemStack {
    if let Some(block_entity) = world.get_block_entity(&location)
        && let Some(brushable) = block_entity
            .as_any()
            .downcast_ref::<BrushableBlockBlockEntity>()
    {
        if let Ok(mut item_guard) = brushable.item.lock()
            && let Some(item) = item_guard.take()
        {
            return item;
        }

        // BrushableBlockEntity#unpackLootTable (BrushableBlockEntity.java:91-111): a
        // structure (desert pyramid / trail ruins) bakes a specific loot table + seed
        // into the block entity's NBT. That must be used ahead of any generic default,
        // so a brushed block yields the loot the structure actually placed there.
        if let Ok(mut loot_table_guard) = brushable.loot_table.lock()
            && let Some(table_key) = loot_table_guard.take()
            && let Some(table) = pumpkin_data::loot_table::get_loot_table(&table_key)
        {
            let seed = brushable
                .loot_table_seed
                .lock()
                .map(|seed| *seed)
                .unwrap_or(0);
            let items = crate::world::loot::generate_loot(table, seed);
            if let Some(first) = items.into_iter().next() {
                return first;
            }
        }
    }

    let loot_key = if is_sand {
        "minecraft:archaeology/desert_pyramid"
    } else {
        "minecraft:archaeology/trail_ruins_common"
    };

    if let Some(table) = pumpkin_data::loot_table::get_loot_table(loot_key) {
        let items = crate::world::loot::generate_loot(table, rand::random());
        if let Some(first) = items.into_iter().next() {
            return first;
        }
    }

    ItemStack::new(1, &Item::SNORT_POTTERY_SHERD)
}

impl ItemBehaviour for BrushItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        player.world().play_sound(
            Sound::ItemBrushBrushingGeneric,
            SoundCategory::Players,
            &player.position(),
        );
        let stack = player.inventory().held_item();
        player
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack, Self::USE_DURATION);
    }

    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        let is_sand = block.id == BlockId::SUSPICIOUS_SAND;
        let is_gravel = block.id == BlockId::SUSPICIOUS_GRAVEL;
        let block_center = location.to_centered_f64();

        if !(is_sand || is_gravel) {
            world.play_sound(
                Sound::ItemBrushBrushingGeneric,
                SoundCategory::Blocks,
                &block_center,
            );
            let stack = player.inventory().held_item();
            player.living_entity.set_active_hand(
                pumpkin_util::Hand::Right,
                stack,
                Self::USE_DURATION,
            );
            return BlockActionResult::Success;
        }

        if let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id)
            && let Some(server) = player.world().server.upgrade()
        {
            let mut event = crate::plugin::api::events::block::block_brush::BlockBrushEvent::new(
                location,
                world.clone(),
                player_arc,
                player.inventory().held_item(),
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return BlockActionResult::Fail;
            }
        }

        let current_state_id = world.get_block_state_id(&location);
        let mut props = SuspiciousSandLikeProperties::from_state_id(current_state_id);

        if props.dusted < 3 {
            props.dusted += 1;
            let next_stage_id = props.to_state_id(block);
            world.set_block_state(&location, next_stage_id, BlockFlags::NOTIFY_ALL);

            world.play_sound(
                if is_sand {
                    Sound::ItemBrushBrushingSand
                } else {
                    Sound::ItemBrushBrushingGravel
                },
                SoundCategory::Blocks,
                &block_center,
            );
        } else {
            let replacement_state_id = if is_sand {
                Block::SAND.default_state.id
            } else {
                Block::GRAVEL.default_state.id
            };

            // Read the saved archaeology item before replacing the block removes
            // its block entity and with it the structure's loot table/seed.
            let loot_item = get_archaeology_loot(is_sand, location, &world);
            world.set_block_state(&location, replacement_state_id, BlockFlags::NOTIFY_ALL);

            // BrushableBlockEntity#brushingCompleted plays no direct SoundEvent; it fires
            // level event 3008 (BrushableBlockEntity.java:126-136), which the client
            // renders as the block-break particles/sound for `current_state_id`. Vanilla
            // never actually plays SoundEvents.BRUSH_*_COMPLETED here (BrushableBlock is
            // registered with the same sound for both `brush_sound` and
            // `brush_completed_sound`, see Blocks.java:328-357, and the completed getter
            // has no caller), so use the world event Pumpkin already models instead.
            world.sync_world_event(
                WorldEvent::ParticlesAndSoundBrushBlockComplete,
                location,
                i32::from(current_state_id.as_u16()),
            );

            let spawn_pos = Vector3::new(
                f64::from(location.0.x) + 0.5,
                f64::from(location.0.y) + 1.0,
                f64::from(location.0.z) + 0.5,
            );
            let item_entity = Arc::new(ItemEntity::new(
                Entity::new(world.clone(), spawn_pos, &EntityType::ITEM),
                loot_item,
            ));
            world.spawn_entity(item_entity);

            // BrushItem.onUseTick only calls `itemStack.hurtAndBreak(1, ...)` when
            // `BrushableBlockEntity#brush` returns true, which happens solely on the
            // final call that completes the block (BrushItem.java:82-92,
            // BrushableBlockEntity.java:60-82) -- not on every intermediate dusted stage.
            if player.gamemode.load() != pumpkin_util::GameMode::Creative {
                let _ = item.damage_item(1);
            }
        }

        let stack = player.inventory().held_item();
        player
            .living_entity
            .set_active_hand(pumpkin_util::Hand::Right, stack, Self::USE_DURATION);

        BlockActionResult::Success
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl BrushItem {
    pub const USE_DURATION: i32 = 200;
}
