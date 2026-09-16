use crate::block::UseWithItemArgs;
use crate::block::registry::BlockActionResult;
use crate::entity::item::ItemEntity;
use crate::entity::{Entity, EntityBase};
use pumpkin_data::block_properties::WallTorchLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::{Block, HorizontalFacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

#[pumpkin_block("minecraft:pumpkin")]
pub struct PumpkinBlock;

impl crate::block::BlockBehaviour for PumpkinBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item != &Item::SHEARS {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        let mut props = WallTorchLikeProperties::default(&Block::CARVED_PUMPKIN);
        // PumpkinBlock.useItemOn: a side click carves the face towards the
        // clicked side; a top/bottom click carves it away from the player
        // (PumpkinBlock.java:49-50).
        props.facing = args.hit.face.to_horizontal_facing().unwrap_or_else(|| {
            args.player
                .living_entity
                .entity
                .get_horizontal_facing()
                .opposite()
        });
        let direction = props.facing.to_block_direction().to_offset();
        let position = args.position.to_f64().add_raw(
            0.5 + f64::from(direction.x) * 0.65,
            0.1,
            0.5 + f64::from(direction.z) * 0.65,
        );
        let params = crate::world::loot::LootContextParameters {
            block_state: Some(args.world.get_block_state(args.position)),
            tool: Some(args.item_stack.clone()),
            position: Some(args.position.to_centered_f64()),
            ..Default::default()
        };
        for stack in crate::world::loot::generate_loot_with_context(
            &pumpkin_data::loot_table::CARVE_PUMPKIN,
            args.world.rand_i64(),
            &params,
        ) {
            let entity = Entity::new(args.world.clone(), position, &EntityType::ITEM);
            let item = ItemEntity::new(entity, stack);
            item.get_entity()
                .velocity
                .store(pumpkin_util::math::vector3::Vector3::new(
                    0.05 * f64::from(direction.x) + args.world.rand_f64() * 0.02,
                    0.05,
                    0.05 * f64::from(direction.z) + args.world.rand_f64() * 0.02,
                ));
            args.world.spawn_entity(Arc::new(item));
        }
        args.world.play_sound_fine(
            pumpkin_data::sound::Sound::BlockPumpkinCarve,
            pumpkin_data::sound::SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            1.0,
            1.0,
        );
        args.world.set_block_state(
            args.position,
            props.to_state_id(&Block::CARVED_PUMPKIN),
            BlockFlags::NOTIFY_ALL,
        );
        args.player.damage_item_in_slot(args.equipment_slot, 1);
        args.world.emit_game_event_from_entity(
            GameEvent::Shear.name(),
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            None,
        );
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            Item::SHEARS.id.into(),
            1,
        );
        BlockActionResult::Success
    }
}
