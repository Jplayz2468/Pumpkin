use std::sync::Arc;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{KelpLikeProperties, WaterLikeProperties};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
pub struct KelpBlock;

impl BlockMetadata for KelpBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::KELP, BlockId::KELP_PLANT].into()
    }
}

impl BlockBehaviour for KelpBlock {
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
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::KELP {
                args.world.set_block_state(
                    &support_pos,
                    Block::KELP_PLANT.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            let support_pos = args.position.down();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::KELP_PLANT {
                args.world.set_block_state(
                    &support_pos,
                    Block::KELP.default_state.id,
                    BlockFlags::empty(),
                );
                args.world.set_block_state(
                    args.position,
                    Block::WATER.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }

    // GrowingPlantHeadBlock.java:114 isValidBonemealTarget, delegated for the body
    // (KELP_PLANT) by GrowingPlantBodyBlock.java:68 which first walks up to the head.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let Some(head_pos) = kelp_head_pos(args.world, args.block, args.position) else {
            return false;
        };
        let growth_pos = head_pos.up();
        kelp_can_grow_into(args.world.get_block(&growth_pos))
            && args.world.is_in_build_limit(growth_pos)
    }

    // GrowingPlantHeadBlock.java:125 performBonemeal (getBlocksToGrowWhenBonemealed = 1
    // for kelp, see KelpBlock.java:61). The body delegates to the head's growth logic
    // (GrowingPlantBodyBlock.java:84).
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let Some(head_pos) = kelp_head_pos(args.world, args.block, args.position) else {
            return;
        };
        let head_state_id = args.world.get_block_state_id(&head_pos);
        let props = KelpLikeProperties::from_state_id(head_state_id);
        let next_age = (props.age + 1).min(25);
        let forward_pos = head_pos.up();
        if kelp_can_grow_into(args.world.get_block(&forward_pos))
            && args.world.is_in_build_limit(forward_pos)
        {
            let mut new_props = props;
            new_props.age = next_age;
            args.world.set_block_state(
                &forward_pos,
                new_props.to_state_id(&Block::KELP),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}

// KelpBlock.java:36 canGrowInto: only grows into water.
fn kelp_can_grow_into(block: &Block) -> bool {
    block == &Block::WATER
}

/// Resolves the KELP head position for either the head (KELP) or body (KELP_PLANT)
/// block, mirroring `GrowingPlantBodyBlock.getHeadPos` / `BlockUtil.getTopConnectedBlock`
/// (growth direction UP for kelp).
fn kelp_head_pos(world: &Arc<World>, block: &Block, position: &BlockPos) -> Option<BlockPos> {
    if block == &Block::KELP {
        return Some(*position);
    }
    if block != &Block::KELP_PLANT {
        return None;
    }
    let max_steps = world.dimension.height + 8;
    let mut current = *position;
    for _ in 0..max_steps {
        current = current.up();
        let found = world.get_block(&current);
        if found == &Block::KELP_PLANT {
            continue;
        }
        return if found == &Block::KELP {
            Some(current)
        } else {
            None
        };
    }
    None
}

impl PlantBlockBase for KelpBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        // Determine support block
        let support_pos = pos;
        let (replacing_block, replacing_block_state) =
            block_accessor.get_block_and_state(&pos.up());
        let (support_block, support_block_state) = block_accessor.get_block_and_state(support_pos);
        if replacing_block == &Block::WATER {
            let water_props = WaterLikeProperties::from_state_id(replacing_block_state.id);

            //Only allow placing kelp on either full water or downward flowing water
            if water_props.level != 0 && water_props.level != 8 {
                return false;
            }
        } else {
            //Replacing block can also be a kelp_plant or kelp in case this is an neighbour update check
            if replacing_block != &Block::KELP_PLANT && replacing_block != &Block::KELP {
                return false;
            }
        }
        // If placing the base kelp block, allow placement on water or on other kelp segments.
        if support_block == &Block::KELP || support_block == &Block::KELP_PLANT {
            return true;
        }
        if support_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_KELP) {
            return false;
        }
        if support_block_state.is_side_solid(pumpkin_data::BlockDirection::Up)
            && support_block.is_solid()
        {
            return true;
        }
        false
    }
    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            return Block::WATER.default_state.id;
        }
        block_state
    }
}
