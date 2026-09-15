use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockState, BlockStateId, block_properties::SnowLikeProperties, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BlockIsReplacing, CanPlaceAtArgs, CanUpdateAtArgs,
    GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType, RandomTickArgs, drop_loot,
};

#[pumpkin_block("minecraft:snow")]
pub struct LayeredSnowBlock;

impl BlockBehaviour for LayeredSnowBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SnowLikeProperties::default(args.block);
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            props = SnowLikeProperties::from_state_id(state_id);
            props.layers = (props.layers + 1).min(8);
        }
        props.to_state_id(args.block)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        SnowLikeProperties::from_state_id(args.state_id).layers < 8
            && (!args.replacing_clicked || args.direction == pumpkin_data::BlockDirection::Up)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        if args.world.get_block_light_level(args.position).unwrap_or(0) > 11 {
            let state = args.world.get_block_state(args.position);
            drop_loot(
                args.world,
                args.block,
                args.position,
                false,
                &crate::world::loot::LootContextParameters {
                    block_state: Some(state),
                    position: Some(args.position.to_centered_f64()),
                    ..Default::default()
                },
            );
            args.world.set_block_state(
                args.position,
                Block::AIR.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if can_place_at(args.world, args.position) {
            args.state_id
        } else {
            Block::AIR.default_state.id
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        computation_type == PathComputationType::Land
            && SnowLikeProperties::from_state_id(state.id).layers < 5
    }
}

pub(crate) fn can_place_at(block_accessor: &dyn BlockAccessor, position: &BlockPos) -> bool {
    let below_pos = position.down();
    let (below_block, state) = block_accessor.get_block_and_state(&below_pos);

    if below_block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_SNOW_LAYER) {
        return false;
    }
    if below_block.has_tag(&tag::Block::MINECRAFT_SUPPORT_OVERRIDE_SNOW_LAYER) {
        return true;
    }

    // Block.isFaceFullSquare(collisionShape, Direction.UP): the collision shape must fully cover
    // the top face, e.g. leaves are not "side solid" but do support snow layers.
    state.get_block_collision_shapes().any(|shape| {
        shape.max.y >= 1.0
            && shape.min.x <= 0.0
            && shape.max.x >= 1.0
            && shape.min.z <= 0.0
            && shape.max.z >= 1.0
    }) || (below_block == &Block::SNOW && SnowLikeProperties::from_state_id(state.id).layers == 8)
}
