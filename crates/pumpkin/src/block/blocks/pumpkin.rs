use crate::block::UseWithItemArgs;
use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::item::ItemEntity;
use pumpkin_data::Block;
use pumpkin_data::block_properties::WallTorchLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

#[pumpkin_block("minecraft:pumpkin")]
pub struct PumpkinBlock;

impl crate::block::BlockBehaviour for PumpkinBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item != &Item::SHEARS {
            return BlockActionResult::Pass;
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
        args.world.set_block_state(
            args.position,
            props.to_state_id(&Block::CARVED_PUMPKIN),
            BlockFlags::NOTIFY_ALL,
        );
        // PumpkinBlock.java:72 fires GameEvent.SHEAR (not BLOCK_CHANGE) for this interaction.
        args.world
            .emit_game_event(GameEvent::Shear.name(), args.position.to_centered_f64());
        let entity = Entity::new(
            args.world.clone(),
            args.position.to_f64(),
            &EntityType::ITEM,
        );
        let item_entity = Arc::new(ItemEntity::new(
            entity,
            ItemStack::new(4, &Item::PUMPKIN_SEEDS),
        ));
        args.world.spawn_entity(item_entity);
        args.player.damage_item_in_slot(args.equipment_slot, 1);
        BlockActionResult::Consume
    }
}
