use pumpkin_data::{Block, BlockState, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BrokenArgs, NormalUseArgs, PathComputationType};

use super::piston::PistonProps;

pub(crate) type MovingPistonProps = pumpkin_data::block_properties::MovingPistonLikeProperties;

#[pumpkin_block("minecraft:moving_piston")]
pub struct PistonExtensionBlock;

impl BlockBehaviour for PistonExtensionBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            let props = MovingPistonProps::from_state_id(args.state.id);
            let pos = args
                .position
                .offset(props.facing.opposite().to_block_direction().to_offset());
            let (new_block, new_state) = args.world.get_block_and_state_id(&pos);
            if &Block::PISTON == new_block || &Block::STICKY_PISTON == new_block {
                let props = PistonProps::from_state_id(new_state);
                if props.extended {
                    // TODO: use player
                    args.world.break_block(&pos, None, BlockFlags::SKIP_DROPS);
                }
            }
        }
    }

    /// Vanilla `MovingPistonBlock.useWithoutItem` (`MovingPistonBlock.java:82`): a
    /// `moving_piston` with no block entity is a stranded husk -- nothing will ever finish its
    /// animation, and it is invisible but solid-ish to the client. Vanilla lets a player clear
    /// it by right-clicking. This is the last-resort escape hatch for a leftover piston piece
    /// and it was missing entirely.
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if args.world.get_block_entity(args.position).is_none() {
            args.world
                .set_block_state(args.position, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            return BlockActionResult::Consume;
        }
        BlockActionResult::Pass
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
