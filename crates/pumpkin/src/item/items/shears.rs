use std::any::Any;

use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::block_properties::BeeNestLikeProperties;
use pumpkin_data::block_properties::BlockProperties;
use pumpkin_data::block_properties::CaveVinesLikeProperties;
use pumpkin_data::block_properties::KelpLikeProperties;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct ShearsItem;

impl ItemMetadata for ShearsItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::SHEARS.id])
    }
}

impl ItemBehaviour for ShearsItem {
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
        let state_id = world.get_block_state_id(&location);

        if handle_growing_plant(item, player, &location, block, state_id) {
            return BlockActionResult::Success;
        }

        if handle_beehive(item, player, &location, block, state_id) {
            return BlockActionResult::Success;
        }

        // Pumpkin carving is handled by `PumpkinBlock::use_with_item`
        // (crates/pumpkin/src/block/blocks/pumpkin.rs), which mirrors vanilla's
        // PumpkinBlock.useItemOn and is tried before this item-side handler runs
        // (see call_use_item_on in net/java/play/use_item_on.rs). There is
        // nothing left for shears to do here for pumpkin blocks.
        BlockActionResult::Pass
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn handle_growing_plant(
    item: &mut ItemStack,
    player: &Player,
    location: &BlockPos,
    block: &Block,
    state_id: BlockStateId,
) -> bool {
    let new_state_id = if KelpLikeProperties::handles_block_id(block.id) {
        let mut props = KelpLikeProperties::from_state_id(state_id);
        if props.age >= 25 {
            return false;
        }
        props.age = 25;
        props.to_state_id(block)
    } else if CaveVinesLikeProperties::handles_block_id(block.id) {
        let mut props = CaveVinesLikeProperties::from_state_id(state_id);
        if props.age >= 25 {
            return false;
        }
        props.age = 25;
        props.to_state_id(block)
    } else {
        return false;
    };

    let world = player.world();
    world.play_sound(
        Sound::BlockGrowingPlantCrop,
        SoundCategory::Blocks,
        &location.to_f64(),
    );
    world.emit_game_event_with_source(
        GameEvent::BlockChange.name(),
        location.to_centered_f64(),
        Some(player.living_entity.entity.entity_id),
    );
    if player.gamemode.load() != pumpkin_util::GameMode::Creative {
        let _ = item.damage_item(1);
    }
    true
}

fn handle_beehive(
    item: &mut ItemStack,
    player: &Player,
    location: &BlockPos,
    block: &Block,
    state_id: BlockStateId,
) -> bool {
    if !BeeNestLikeProperties::handles_block_id(block.id) {
        return false;
    }

    let props = BeeNestLikeProperties::from_state_id(state_id);

    if props.honey_level != 5 {
        return false;
    }

    let world = player.world();
    use pumpkin_util::random::RandomImpl;
    let seed = world
        .random
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .next_i64();
    let params = crate::world::loot::LootContextParameters {
        block_state: Some(state_id.to_state()),
        tool: Some(item.clone()),
        this_entity: Some(player.living_entity.entity.entity_type),
        position: Some(location.to_centered_f64()),
        world_time: world.level_info.load().day_time as u64,
        is_raining: Some(world.is_raining()),
        is_thundering: Some(world.is_thundering()),
        ..Default::default()
    };
    let mut drops = crate::world::loot::generate_loot_with_context(
        &pumpkin_data::loot_table::HARVEST_BEEHIVE,
        seed,
        &params,
    );
    if let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id)
        && let Some(server) = player.world().server.upgrade()
    {
        let mut event =
            crate::plugin::api::events::player::player_harvest_block::PlayerHarvestBlockEvent {
                player: player_arc,
                block_pos: *location,
                harvested_items: drops.clone(),
                cancelled: false,
            };
        server.plugin_manager.fire_blocking(&server, &mut event);
        if event.cancelled {
            return false;
        }
        drops = event.harvested_items;
    }

    for stack in drops {
        world.drop_stack(location, stack);
    }
    if player.gamemode.load() != pumpkin_util::GameMode::Creative {
        let _ = item.damage_item(1);
    }
    // BeehiveBlock.java:166 plays this at the player's position, not the hive's.
    let player_pos = player.living_entity.entity.pos.load();
    world.play_sound(Sound::BlockBeehiveShear, SoundCategory::Blocks, &player_pos);
    world.emit_game_event_with_source(
        GameEvent::Shear.name(),
        location.to_centered_f64(),
        Some(player.living_entity.entity.entity_id),
    );

    crate::block::blocks::beehive::finish_harvest(&world, location, state_id.to_state(), player);
    player.increment_stat(
        pumpkin_data::statistic::StatisticCategory::Used,
        i32::from(Item::SHEARS.id),
        1,
    );

    true
}
