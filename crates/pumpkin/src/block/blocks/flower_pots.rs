use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, NormalUseArgs, PathComputationType, RandomTickArgs, UseWithItemArgs,
};
use pumpkin_data::flower_pot_transformations::get_potted_item;
use pumpkin_data::{
    Block, BlockId, BlockState,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block_from_tag("minecraft:flower_pots")]
pub struct FlowerPotBlock;

impl BlockBehaviour for FlowerPotBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let potted = get_potted_item(args.item_stack.item.id);
        if args.item_stack.is_empty() || potted == BlockId::AIR {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if args.block != &Block::FLOWER_POT {
            return BlockActionResult::Consume;
        }
        args.world.set_block_state(
            args.position,
            Block::from_id(potted).default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            None,
        );
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::PotFlower as i32,
            1,
        );
        if !args.player.has_infinite_materials() {
            args.item_stack.decrement(1);
        }
        BlockActionResult::Success
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if args.block == &Block::FLOWER_POT {
            return BlockActionResult::Consume;
        }
        let name = args.block.name.strip_prefix("potted_").unwrap_or("");
        let name = match name {
            "azalea_bush" => "azalea",
            "flowering_azalea_bush" => "flowering_azalea",
            _ => name,
        };
        if let Some(item) = Item::from_registry_key(name) {
            let mut plant = ItemStack::new(1, item);
            if !args
                .player
                .get_inventory()
                .insert_stack_anywhere(&mut plant)
            {
                args.player.drop_item(plant);
            }
        }
        args.world.set_block_state(
            args.position,
            Block::FLOWER_POT.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            None,
        );
        BlockActionResult::Success
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let is_open_potted = args.block.eq(&Block::POTTED_OPEN_EYEBLOSSOM);
        let is_closed_potted = args.block.eq(&Block::POTTED_CLOSED_EYEBLOSSOM);
        if !is_open_potted && !is_closed_potted {
            return;
        }

        let is_open = is_open_potted;
        let should_be_open = args.world.eyeblossom_open(args.position).unwrap_or(is_open);

        if is_open != should_be_open {
            let next_block = if should_be_open {
                &Block::POTTED_OPEN_EYEBLOSSOM
            } else {
                &Block::POTTED_CLOSED_EYEBLOSSOM
            };
            args.world.set_block_state(
                args.position,
                next_block.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            super::plant::eyeblossom::spawn_transform_particle(
                args.world,
                args.position,
                should_be_open,
                &mut Some(args.random),
            );
            args.world.play_sound(
                if should_be_open {
                    Sound::BlockEyeblossomOpenLong
                } else {
                    Sound::BlockEyeblossomCloseLong
                },
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
