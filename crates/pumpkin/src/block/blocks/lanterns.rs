use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

#[pumpkin_block_from_tag("minecraft:lanterns")]
pub struct LanternBlock;

impl BlockBehaviour for LanternBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = pumpkin_data::block_properties::LanternLikeProperties::default(args.block);
        props.r#waterlogged = args.replacing.water_source();

        // LanternBlock.java:44: scan placement directions, checking the support
        // corresponding to each orientation rather than always preferring ceilings.
        for direction in super::vine::get_nearest_looking_directions(
            args.player,
            *args.position == args.use_item_on.position,
            args.direction.opposite(),
        ) {
            if !matches!(direction, BlockDirection::Up | BlockDirection::Down) {
                continue;
            }
            props.hanging = direction == BlockDirection::Up;
            if can_survive(args.world, args.position, props.hanging) {
                return props.to_state_id(args.block);
            }
        }

        pumpkin_data::BlockStateId::AIR
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let props =
            pumpkin_data::block_properties::LanternLikeProperties::from_state_id(args.state.id);
        can_survive(args.block_accessor, args.position, props.hanging)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props =
            pumpkin_data::block_properties::LanternLikeProperties::from_state_id(args.state_id);
        super::schedule_waterlogged_tick(args.world, args.position, props.waterlogged);
        let support_direction = if props.hanging {
            BlockDirection::Up
        } else {
            BlockDirection::Down
        };
        if args.direction == support_direction
            && !can_survive(args.world, args.position, props.hanging)
        {
            pumpkin_data::Block::AIR.default_state.id
        } else {
            args.state_id
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn can_survive(world: &dyn BlockAccessor, position: &BlockPos, hanging: bool) -> bool {
    // Block.canSupportCenter rejects unstable bottom faces; an unrelated support
    // on the other side cannot hold a lantern with this orientation.
    let direction = if hanging {
        BlockDirection::Up
    } else {
        BlockDirection::Down
    };
    let (block, state) = world.get_block_and_state(&position.offset(direction.to_offset()));
    !(hanging && block.has_tag(&tag::Block::MINECRAFT_UNSTABLE_BOTTOM_CENTER))
        && state.is_center_solid(direction.opposite())
}
