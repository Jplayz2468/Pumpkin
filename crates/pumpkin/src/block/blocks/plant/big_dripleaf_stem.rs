use std::sync::Arc;

use crate::block::blocks::plant::big_dripleaf::{
    can_grow_into, grow_dripleaf_from_head, schedule_water,
};
use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnScheduledTickArgs,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BigDripleafLikeProperties, LadderLikeProperties};
use pumpkin_data::{Block, BlockDirection, tag, tag::Taggable};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block("minecraft:big_dripleaf_stem")]
pub struct BigDripleafStemBlock;

pub type BigDripleafStemLikeProperties = LadderLikeProperties;

impl BlockBehaviour for BigDripleafStemBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_survive(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if matches!(args.direction, BlockDirection::Down | BlockDirection::Up)
            && !can_survive(args.world, args.position)
        {
            args.world.schedule_block_tick(
                args.block,
                *args.position,
                1,
                pumpkin_world::tick::TickPriority::Normal,
            );
        }
        if BigDripleafStemLikeProperties::from_state_id(args.state_id).waterlogged {
            schedule_water(args.world, args.position);
        }
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_survive(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }

    // BigDripleafStemBlock.java:105 isValidBonemealTarget: find the connected head
    // above (via BlockUtil.getTopConnectedBlock) and check it can grow further.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        find_dripleaf_head(args.world, args.position)
            .is_some_and(|head_pos| can_grow_into(args.world, &head_pos.up()))
    }

    // BigDripleafStemBlock.java:116 performBonemeal: delegates growth to the head.
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if let Some(head_pos) = find_dripleaf_head(args.world, args.position) {
            let head_state_id = args.world.get_block_state_id(&head_pos);
            let mut head_props = BigDripleafLikeProperties::from_state_id(head_state_id);
            head_props.facing = BigDripleafStemLikeProperties::from_state_id(args.state_id).facing;
            grow_dripleaf_from_head(args.world, &head_pos, head_props);
        }
    }
}

/// `BlockUtil.getTopConnectedBlock` (growth direction UP): walks up through
/// BIG_DRIPLEAF_STEM until it finds the connected BIG_DRIPLEAF head.
fn find_dripleaf_head(world: &Arc<World>, position: &BlockPos) -> Option<BlockPos> {
    let max_steps = world.dimension.height + 8;
    let mut current = *position;
    for _ in 0..max_steps {
        current = current.up();
        let found = world.get_block(&current);
        if found == &Block::BIG_DRIPLEAF_STEM {
            continue;
        }
        return if found == &Block::BIG_DRIPLEAF {
            Some(current)
        } else {
            None
        };
    }
    None
}
fn can_survive(accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let below = accessor.get_block(&pos.down());
    let above = accessor.get_block(&pos.up());
    (below == &Block::BIG_DRIPLEAF_STEM
        || below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_BIG_DRIPLEAF))
        && (above == &Block::BIG_DRIPLEAF_STEM || above == &Block::BIG_DRIPLEAF)
}
