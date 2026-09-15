use std::sync::Arc;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::blocks::plant::big_dripleaf::{
    can_grow_into, can_plant_dripleaf_on_top, grow_dripleaf_from_head,
};
use crate::block::{
    BlockBehaviour, BonemealArgs, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
};
use crate::world::World;
use pumpkin_data::Block;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{BigDripleafLikeProperties, LadderLikeProperties};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block("minecraft:big_dripleaf_stem")]
pub struct BigDripleafStemBlock;

pub type BigDripleafStemLikeProperties = LadderLikeProperties;

impl BlockBehaviour for BigDripleafStemBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
    fn broken(&self, args: BrokenArgs<'_>) {
        handle_big_dripleaf_breaking(args.world, args.position);
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
            let head_props = BigDripleafLikeProperties::from_state_id(head_state_id);
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
impl PlantBlockBase for BigDripleafStemBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let support_block = block_accessor.get_block(pos);
        can_plant_dripleaf_on_top(support_block)
    }

    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            let _block = block_accessor.get_block(block_pos);

            let dripleaf_stem_props = BigDripleafStemLikeProperties::from_state_id(block_state);
            if dripleaf_stem_props.waterlogged {
                return Block::WATER.default_state.id;
            }
            return Block::AIR.default_state.id;
        }
        block_state
    }
}
pub fn handle_big_dripleaf_breaking(world: &Arc<World>, position: &BlockPos) {
    let support_pos = position.down();
    let (support_block, support_state_id) = world.get_block_and_state_id(&support_pos);
    if support_block == &Block::BIG_DRIPLEAF_STEM {
        let dripleaf_stem_props = BigDripleafStemLikeProperties::from_state_id(support_state_id);

        let mut dripleaf_props = BigDripleafLikeProperties::default(&Block::BIG_DRIPLEAF);
        dripleaf_props.facing = dripleaf_stem_props.facing;
        dripleaf_props.waterlogged = dripleaf_stem_props.waterlogged;
        world.set_block_state(
            &support_pos,
            dripleaf_props.to_state_id(&Block::BIG_DRIPLEAF),
            BlockFlags::empty(),
        );
    }
}
