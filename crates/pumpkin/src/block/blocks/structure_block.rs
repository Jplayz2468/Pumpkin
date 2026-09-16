use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, NormalUseArgs, OnPlaceArgs};

use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::StructureBlockLikeProperties;
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:structure_block")]
pub struct StructureBlock;

impl BlockBehaviour for StructureBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let props = StructureBlockLikeProperties::default(args.block);
        props.to_state_id(args.block)
    }

    fn player_placed(&self, args: crate::block::PlayerPlacedArgs<'_>) {
        if let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(structure) = entity.as_any().downcast_ref::<crate::block::entities::structure_block::StructureBlockBlockEntity>() {
            *structure.author.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = args.player.gameprofile.name.clone();
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        {
            if !args.player.can_use_game_master_blocks() {
                return BlockActionResult::Pass;
            }
            let Some(block_entity) = args.world.get_block_entity(args.position) else {
                return BlockActionResult::Pass;
            };
            if !block_entity
                .as_any()
                .is::<crate::block::entities::structure_block::StructureBlockBlockEntity>()
            {
                return BlockActionResult::Pass;
            }
            args.world.update_block_entity(&block_entity);

            BlockActionResult::Success
        }
    }
}
