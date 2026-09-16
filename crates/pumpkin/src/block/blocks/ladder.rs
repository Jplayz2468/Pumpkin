use crate::block::{BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::LadderLikeProperties;
use pumpkin_data::{Block, BlockDirection, HorizontalFacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockAccessor;

#[pumpkin_block("minecraft:ladder")]
pub struct LadderBlock;

impl BlockBehaviour for LadderBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let clicked_pos = args.use_item_on.position;
        let (clicked_block, clicked_block_state_id) =
            args.world.get_block_and_state_id(&clicked_pos);
        if *args.position != clicked_pos && clicked_block == &Block::LADDER {
            //you can't click on a ladder and place a ladder
            let props = LadderLikeProperties::from_state_id(clicked_block_state_id);
            let sub = args.position.0.sub(&clicked_pos.0);
            if let Some(dir) = horizontal_facing_from_offset(sub)
                && let Some(horizontal_facing) = dir.to_horizontal_facing()
                && props.facing == horizontal_facing
            {
                return Block::AIR.default_state.id;
            }
        }
        let mut props = LadderLikeProperties::default(args.block);

        props.waterlogged = args.replacing.water_source();
        let directions = super::vine::get_nearest_looking_directions(
            args.player,
            *args.position == clicked_pos,
            args.direction.opposite(),
        );
        for dir in directions {
            if !dir.is_horizontal() {
                continue;
            }
            if !can_place_ladder_at(args.world, args.position, dir) {
                continue;
            }
            if let Some(facing) = dir.opposite().to_horizontal_facing() {
                props.facing = facing;
                return props.to_state_id(args.block);
            }
        }
        Block::AIR.default_state.id
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let props = LadderLikeProperties::from_state_id(args.state.id);
        can_place_ladder_at(
            args.block_accessor,
            args.position,
            props.facing.to_block_direction().opposite(),
        )
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = LadderLikeProperties::from_state_id(args.state_id);
        if props.facing.to_block_direction().opposite() == args.direction
            && !can_place_ladder_at(
                args.world,
                args.position,
                props.facing.to_block_direction().opposite(),
            )
        {
            return BlockStateId::AIR;
        }
        super::schedule_waterlogged_tick(args.world, args.position, props.waterlogged);
        args.state_id
    }
}
#[must_use]
pub const fn horizontal_facing_from_offset(offset: Vector3<i32>) -> Option<BlockDirection> {
    match (offset.x, offset.y, offset.z) {
        (0, 0, -1) => Some(BlockDirection::North),
        (0, 0, 1) => Some(BlockDirection::South),
        (-1, 0, 0) => Some(BlockDirection::West),
        (1, 0, 0) => Some(BlockDirection::East),
        _ => None,
    }
}
fn can_place_ladder_at(
    world: &dyn BlockAccessor,
    block_pos: &BlockPos,
    facing: BlockDirection,
) -> bool {
    world
        .get_block_state(&block_pos.offset(facing.to_offset()))
        .is_side_solid(facing.opposite())
}
