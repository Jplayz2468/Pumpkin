use std::sync::{Arc, atomic::Ordering};

use pumpkin_macros::pumpkin_block;

use crate::block::entities::chiseled_bookshelf::ChiseledBookshelfBlockEntity;
use crate::{
    block::{
        BlockBehaviour, BlockHitResult, GetComparatorOutputArgs, NormalUseArgs, OnPlaceArgs,
        OnStateReplacedArgs, UseWithItemArgs, registry::BlockActionResult,
    },
    entity::{EntityBase, player::Player},
    world::World,
};
use pumpkin_data::{
    BlockStateId,
    block_properties::{ChiseledBookshelfLikeProperties, HorizontalFacing},
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    tag,
    tag::Taggable,
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_util::math::{position::BlockPos, vector2::Vector2};

#[pumpkin_block("minecraft:chiseled_bookshelf")]
pub struct ChiseledBookshelfBlock;

impl BlockBehaviour for ChiseledBookshelfBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut properties = ChiseledBookshelfLikeProperties::default(args.block);

        // Face in the opposite direction the player is facing
        properties.facing = args.player.get_entity().get_horizontal_facing().opposite();

        properties.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(entity) = entity
            .as_any()
            .downcast_ref::<ChiseledBookshelfBlockEntity>()
        else {
            return BlockActionResult::Pass;
        };
        let props = ChiseledBookshelfLikeProperties::from_state_id(
            args.world.get_block_state_id(args.position),
        );
        let Some(slot) = Self::get_slot_for_hit(args.hit, props.facing) else {
            return BlockActionResult::Pass;
        };
        if !Self::is_slot_used(props, slot) {
            return BlockActionResult::Consume;
        }
        Self::try_remove_book(args.world, args.player, args.position, entity, slot);
        BlockActionResult::Success
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(entity) = entity
            .as_any()
            .downcast_ref::<ChiseledBookshelfBlockEntity>()
        else {
            return BlockActionResult::Pass;
        };
        if !args
            .item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_BOOKSHELF_BOOKS)
        {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        let props = ChiseledBookshelfLikeProperties::from_state_id(
            args.world.get_block_state_id(args.position),
        );
        let Some(slot) = Self::get_slot_for_hit(args.hit, props.facing) else {
            return BlockActionResult::Pass;
        };
        if Self::is_slot_used(props, slot) {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        Self::try_add_book(
            args.world,
            args.player,
            args.position,
            entity,
            slot,
            args.item_stack,
        );
        BlockActionResult::Success
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(block_entity) = block_entity
                .as_any()
                .downcast_ref::<ChiseledBookshelfBlockEntity>()
        {
            return Some((block_entity.last_interacted_slot.load(Ordering::Relaxed) + 1) as u8);
        }
        Some(0)
    }
}

impl ChiseledBookshelfBlock {
    fn try_add_book(
        world: &Arc<World>,
        player: &Player,
        position: &BlockPos,
        entity: &ChiseledBookshelfBlockEntity,
        slot: i8,
        item: &mut ItemStack,
    ) {
        player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            i32::from(item.item.id),
            1,
        );

        let sound = if item.get_item() == &Item::ENCHANTED_BOOK {
            Sound::BlockChiseledBookshelfInsertEnchanted
        } else {
            Sound::BlockChiseledBookshelfInsert
        };

        let mut book = item.clone();
        book.item_count = 1;
        if !player.has_infinite_materials() {
            item.decrement(1);
        }
        entity.set_book(slot as usize, book);

        world.play_sound(sound, SoundCategory::Blocks, &position.to_centered_f64());
    }

    fn try_remove_book(
        world: &Arc<World>,
        player: &Arc<Player>,
        position: &BlockPos,
        entity: &ChiseledBookshelfBlockEntity,
        slot: i8,
    ) {
        let mut stack = entity.remove_book(slot as usize, 1);

        let sound = if stack.get_item() == &Item::ENCHANTED_BOOK {
            Sound::BlockChiseledBookshelfPickupEnchanted
        } else {
            Sound::BlockChiseledBookshelfPickup
        };

        world.play_sound(sound, SoundCategory::Blocks, &position.to_centered_f64());
        if !player.get_inventory().insert_stack_anywhere(&mut stack) {
            // Drop the item on the ground if the player cannot hold it because of a full inventory
            player.drop_item(stack);
        }
        world.emit_game_event_from_entity(
            "block_change",
            position.to_centered_f64(),
            Some(player.as_ref()),
            None,
        );
    }

    fn get_slot_for_hit(hit: &BlockHitResult<'_>, facing: HorizontalFacing) -> Option<i8> {
        Self::get_hit_pos(hit, facing).map(|position| {
            let i = Self::get_section(1.0 - position.y, 2);
            let j = Self::get_section(position.x, 3);
            j + i * 3
        })
    }

    fn get_hit_pos(hit: &BlockHitResult<'_>, facing: HorizontalFacing) -> Option<Vector2<f32>> {
        // If the direction is not horizontal, we cannot hit a slot
        let direction = hit.face.to_horizontal_facing()?;

        // If the facing direction does not match the block's facing, we cannot hit a slot
        if facing != direction {
            return None;
        }

        match direction {
            HorizontalFacing::North => Some(Vector2::new(1.0 - hit.cursor_pos.x, hit.cursor_pos.y)),
            HorizontalFacing::South => Some(Vector2::new(hit.cursor_pos.x, hit.cursor_pos.y)),
            HorizontalFacing::West => Some(Vector2::new(hit.cursor_pos.z, hit.cursor_pos.y)),
            HorizontalFacing::East => Some(Vector2::new(1.0 - hit.cursor_pos.z, hit.cursor_pos.y)),
        }
    }

    // SelectableSlotContainer in 26.2 divides the full face into equal sections.
    fn get_section(coordinate: f32, count: i8) -> i8 {
        ((coordinate * 16.0 / (16.0 / f32::from(count))).floor() as i32)
            .clamp(0, i32::from(count) - 1) as i8
    }

    const fn is_slot_used(properties: ChiseledBookshelfLikeProperties, slot: i8) -> bool {
        match slot {
            0 => properties.slot_0_occupied,
            1 => properties.slot_1_occupied,
            2 => properties.slot_2_occupied,
            3 => properties.slot_3_occupied,
            4 => properties.slot_4_occupied,
            5 => properties.slot_5_occupied,
            _ => false,
        }
    }
}
