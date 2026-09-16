use pumpkin_data::BlockStateId;
use pumpkin_data::{Block, HorizontalFacingExt};
use pumpkin_data::{BlockDirection, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

type WallTorchProps = pumpkin_data::block_properties::WallTorchLikeProperties;
// Normal tourches don't have properties

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
};

pub struct TorchBlock;

impl BlockMetadata for TorchBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::TORCH,
            BlockId::SOUL_TORCH,
            BlockId::WALL_TORCH,
            BlockId::SOUL_WALL_TORCH,
            BlockId::COPPER_TORCH,
            BlockId::COPPER_WALL_TORCH,
        ]
        .into()
    }
}

impl BlockBehaviour for TorchBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let directions = super::vine::get_nearest_looking_directions(
            args.player,
            *args.position == args.use_item_on.position,
            args.direction.opposite(),
        );
        let (standing, wall) = match args.block.id {
            BlockId::TORCH | BlockId::WALL_TORCH => (&Block::TORCH, &Block::WALL_TORCH),
            BlockId::SOUL_TORCH | BlockId::SOUL_WALL_TORCH => {
                (&Block::SOUL_TORCH, &Block::SOUL_WALL_TORCH)
            }
            _ => (&Block::COPPER_TORCH, &Block::COPPER_WALL_TORCH),
        };
        let wall_state = directions.iter().find_map(|direction| {
            if !direction.is_horizontal() || !can_place_at(args.world, args.position, *direction) {
                return None;
            }
            let mut props = WallTorchProps::default(wall);
            props.facing = direction.opposite().to_cardinal_direction();
            Some(props.to_state_id(wall))
        });
        for direction in directions {
            if direction == BlockDirection::Up {
                continue;
            }
            if direction == BlockDirection::Down {
                if args
                    .world
                    .get_block_state(&args.position.down())
                    .is_center_solid(BlockDirection::Up)
                {
                    return standing.default_state.id;
                }
            } else if let Some(state) = wall_state {
                return state;
            }
        }
        BlockStateId::AIR
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        if matches!(
            args.block.id,
            BlockId::WALL_TORCH | BlockId::SOUL_WALL_TORCH | BlockId::COPPER_WALL_TORCH
        ) {
            let props = WallTorchProps::from_state_id(args.state.id);
            can_place_at(
                args.block_accessor,
                args.position,
                props.facing.to_block_direction().opposite(),
            )
        } else {
            args.block_accessor
                .get_block_state(&args.position.down())
                .is_center_solid(BlockDirection::Up)
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.block == &Block::WALL_TORCH
            || args.block == &Block::SOUL_WALL_TORCH
            || args.block == &Block::COPPER_WALL_TORCH
        {
            let props = WallTorchProps::from_state_id(args.state_id);
            if props.facing.to_block_direction().opposite() == args.direction
                && !can_place_at(
                    args.world,
                    args.position,
                    props.facing.to_block_direction().opposite(),
                )
            {
                return BlockStateId::AIR;
            }
        } else if args.direction == BlockDirection::Down {
            let support_block = args.world.get_block_state(&args.position.down());
            if !support_block.is_center_solid(BlockDirection::Up) {
                return BlockStateId::AIR;
            }
        }
        args.state_id
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos, facing: BlockDirection) -> bool {
    world
        .get_block_state(&block_pos.offset(facing.to_offset()))
        .is_side_solid(facing.opposite())
}
