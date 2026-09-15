use std::sync::Arc;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs,
    GetStateForNeighborUpdateArgs, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::KelpLikeProperties;
use pumpkin_data::{Block, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

pub struct WeepingVinesBlock;
impl BlockMetadata for WeepingVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::WEEPING_VINES, BlockId::WEEPING_VINES_PLANT].into()
    }
}

impl BlockBehaviour for WeepingVinesBlock {
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
            let support_pos = args.position.up();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::WEEPING_VINES {
                args.world.set_block_state(
                    &support_pos,
                    Block::WEEPING_VINES_PLANT.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            let support_pos = args.position.up();
            let support_block = args.world.get_block(&support_pos);
            if support_block == &Block::WEEPING_VINES_PLANT {
                args.world.set_block_state(
                    &support_pos,
                    Block::WEEPING_VINES.default_state.id,
                    BlockFlags::empty(),
                );
            }
        }
    }

    // GrowingPlantHeadBlock.java:114 isValidBonemealTarget, delegated for the body
    // (WEEPING_VINES_PLANT) via GrowingPlantBodyBlock.java:68.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let Some(head_pos) = weeping_vines_head_pos(args.world, args.block, args.position) else {
            return false;
        };
        let growth_pos = head_pos.down();
        weeping_vines_can_grow_into(args.world.get_block(&growth_pos))
            && args.world.is_in_build_limit(growth_pos)
    }

    // GrowingPlantHeadBlock.java:125 performBonemeal, blocksToGrow via
    // NetherVines.getBlocksToGrowWhenBonemealed (WeepingVinesBlock.java:24).
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let Some(head_pos) = weeping_vines_head_pos(args.world, args.block, args.position) else {
            return;
        };
        let head_state_id = args.world.get_block_state_id(&head_pos);
        let props = KelpLikeProperties::from_state_id(head_state_id);
        let mut next_age = (props.age + 1).min(25);
        let blocks_to_grow = nether_vines_blocks_to_grow();
        let mut forward_pos = head_pos.down();
        for _ in 0..blocks_to_grow {
            if !weeping_vines_can_grow_into(args.world.get_block(&forward_pos))
                || !args.world.is_in_build_limit(forward_pos)
            {
                break;
            }
            let mut new_props = props;
            new_props.age = next_age;
            args.world.set_block_state(
                &forward_pos,
                new_props.to_state_id(&Block::WEEPING_VINES),
                BlockFlags::NOTIFY_ALL,
            );
            forward_pos = forward_pos.down();
            next_age = (next_age + 1).min(25);
        }
    }
}

// NetherVines.isValidGrowthState: grows only into air.
fn weeping_vines_can_grow_into(block: &Block) -> bool {
    block == &Block::AIR
}

// NetherVines.getBlocksToGrowWhenBonemealed: geometric falloff with 0.826 decay.
fn nether_vines_blocks_to_grow() -> i32 {
    let mut grow_probability = 1.0_f64;
    let mut count = 0;
    loop {
        let roll: f64 = rand::rng().random();
        if roll >= grow_probability {
            break;
        }
        grow_probability *= 0.826;
        count += 1;
    }
    count
}

/// Resolves the WEEPING_VINES head position for either the head or the body
/// (WEEPING_VINES_PLANT), mirroring `BlockUtil.getTopConnectedBlock` (growth
/// direction DOWN for weeping vines).
fn weeping_vines_head_pos(world: &Arc<World>, block: &Block, position: &BlockPos) -> Option<BlockPos> {
    if block == &Block::WEEPING_VINES {
        return Some(*position);
    }
    if block != &Block::WEEPING_VINES_PLANT {
        return None;
    }
    let max_steps = world.dimension.height + 8;
    let mut current = *position;
    for _ in 0..max_steps {
        current = current.down();
        let found = world.get_block(&current);
        if found == &Block::WEEPING_VINES_PLANT {
            continue;
        }
        return if found == &Block::WEEPING_VINES {
            Some(current)
        } else {
            None
        };
    }
    None
}

impl PlantBlockBase for WeepingVinesBlock {
    fn can_place_at(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        // Determine support block
        let support_pos = pos.up();
        let (support_block, support_block_state) = block_accessor.get_block_and_state(&support_pos);

        if support_block == &Block::WEEPING_VINES || support_block == &Block::WEEPING_VINES_PLANT {
            return true;
        }
        if support_block_state.is_side_solid(pumpkin_data::BlockDirection::Down)
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
            return Block::AIR.default_state.id;
        }
        block_state
    }
}
