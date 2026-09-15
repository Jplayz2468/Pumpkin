use crate::block::BlockBehaviour;
use pumpkin_data::block_state::BlockStateId;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::RandomImpl;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block("minecraft:netherrack")]
pub struct NetherrackBlock;

impl BlockBehaviour for NetherrackBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        let above_block = args.world.get_block_state(&args.position.up());

        // The cached vanilla light dampening handles glass and waterlogged
        // states, which cannot be inferred from collision fullness and isLiquid.
        if above_block.opacity != 0 {
            return false;
        }

        for block_pos in neighbors(*args.position) {
            if args
                .world
                .get_block(&block_pos)
                .has_tag(&tag::Block::MINECRAFT_NYLIUM)
            {
                return true;
            }
        }

        false
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        let mut warped = false;
        let mut crimson = false;

        for block_pos in neighbors(*args.position) {
            let block = args.world.get_block(&block_pos);

            if block.id == Block::WARPED_NYLIUM.id {
                warped = true;
            }

            if block.id == Block::CRIMSON_NYLIUM.id {
                crimson = true;
            }

            if warped && crimson {
                break;
            }
        }

        if !warped && !crimson {
            return;
        }

        let end_block: BlockStateId = match (warped, crimson) {
            (true, true) => {
                if crate::block::random::BlockRandom::Shared(&args.world.random).next_bool() {
                    Block::WARPED_NYLIUM.default_state.id
                } else {
                    Block::CRIMSON_NYLIUM.default_state.id
                }
            }
            (true, false) => Block::WARPED_NYLIUM.default_state.id,
            (false, true) => Block::CRIMSON_NYLIUM.default_state.id,
            (false, false) => Block::NETHERRACK.default_state.id,
        };

        args.world
            .set_block_state(args.position, end_block, BlockFlags::NOTIFY_ALL);
    }
}

fn neighbors(pos: BlockPos) -> impl Iterator<Item = BlockPos> {
    (-1..=1).flat_map(move |z| (-1..=1).flat_map(move |y| (-1..=1).map(move |x| pos.add(x, y, z))))
}
