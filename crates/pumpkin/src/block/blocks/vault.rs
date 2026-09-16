use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{VaultLikeProperties, VaultState};
use pumpkin_macros::pumpkin_block;

use crate::block::entities::vault::VaultBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, OnPlaceArgs, UseWithItemArgs};

#[pumpkin_block("minecraft:vault")]
pub struct VaultBlock;

impl BlockBehaviour for VaultBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = VaultLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state =
            VaultLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if args.item_stack.is_empty() || state.vault_state != VaultState::Active {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(vault_entity) = block_entity.as_any().downcast_ref::<VaultBlockEntity>()
        {
            vault_entity.try_insert(args.world, args.player, args.item_stack);

            return BlockActionResult::Success;
        }

        BlockActionResult::PassToDefaultBlockAction
    }
}
