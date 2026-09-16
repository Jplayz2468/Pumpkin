use crate::block::{
    BlockBehaviour, BlockIsReplacing, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::block_properties::{Facing, WallTorchLikeProperties, WhiteBannerLikeProperties};
use pumpkin_data::{Block, BlockDirection, BlockStateId, FacingExt, HorizontalFacingExt};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

#[pumpkin_block_from_tag("minecraft:banners")]
pub struct BannerBlock;

fn support_direction(block: &Block, state: BlockStateId) -> BlockDirection {
    if block.name.contains("_wall_banner") {
        WallTorchLikeProperties::from_state_id(state)
            .facing
            .opposite()
            .to_block_direction()
    } else {
        BlockDirection::Down
    }
}

fn can_survive(
    world: &dyn BlockAccessor,
    block: &Block,
    state: BlockStateId,
    pos: &BlockPos,
) -> bool {
    world
        .get_block_state(&pos.offset(support_direction(block, state).to_offset()))
        .is_solid()
}

impl BlockBehaviour for BannerBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut directions = args.player.get_entity().get_entity_facing_order();
        if args.replacing == BlockIsReplacing::None {
            let face = args.direction.to_facing();
            if let Some(i) = directions.iter().position(|dir| *dir == face) {
                directions.copy_within(0..i, 1);
                directions[0] = face;
            }
        }
        let wall = Block::from_name(&args.block.name.replace("_banner", "_wall_banner"));
        let wall_state = wall.and_then(|block| {
            directions.iter().find_map(|dir| {
                let horizontal = dir.to_horizontal_facing()?;
                let mut props = WallTorchLikeProperties::default(block);
                props.facing = horizontal.opposite();
                let state = props.to_state_id(block);
                can_survive(args.world, block, state, args.position).then_some(state)
            })
        });
        for dir in directions {
            match dir {
                Facing::Up => {}
                Facing::Down => {
                    let mut props = WhiteBannerLikeProperties::default(args.block);
                    props.rotation = args.player.get_entity().get_flipped_rotation_16();
                    let state = props.to_state_id(args.block);
                    if can_survive(args.world, args.block, state, args.position) {
                        return state;
                    }
                }
                _ => {
                    if let Some(state) = wall_state {
                        return state;
                    }
                }
            }
        }
        BlockStateId::AIR
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_survive(
            args.block_accessor,
            args.block,
            args.state.id,
            args.position,
        )
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == support_direction(args.block, args.state_id)
            && !can_survive(args.world, args.block, args.state_id, args.position)
        {
            BlockStateId::AIR
        } else {
            args.state_id
        }
    }
}
