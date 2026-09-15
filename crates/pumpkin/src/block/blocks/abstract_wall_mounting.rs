use crate::entity::EntityBase;
use pumpkin_data::{
    Block, BlockDirection, BlockStateId, HorizontalFacingExt,
    block_properties::{AttachFace, HorizontalFacing},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

use crate::{block::GetStateForNeighborUpdateArgs, entity::player::Player};

pub trait WallMountedBlock: Send + Sync {
    fn get_direction(&self, state_id: BlockStateId, block: &Block) -> BlockDirection;

    /// FaceAttachedHorizontalDirectionalBlock tries the context's ordered support directions.
    fn placement(
        &self,
        args: &crate::block::OnPlaceArgs<'_>,
    ) -> Option<(AttachFace, HorizontalFacing)> {
        let mut directions = args.player.get_entity().get_entity_facing_order();
        if args.use_item_on.position != *args.position {
            if let Some(index) = directions
                .iter()
                .position(|direction| *direction == args.direction.to_facing())
            {
                directions[..=index].rotate_right(1);
            }
        }
        for direction in directions {
            let direction = direction.to_block_direction();
            let (face, facing) = if direction.is_horizontal() {
                (
                    AttachFace::Wall,
                    direction.opposite().to_cardinal_direction(),
                )
            } else {
                (
                    if direction == BlockDirection::Up {
                        AttachFace::Ceiling
                    } else {
                        AttachFace::Floor
                    },
                    args.player.get_entity().get_horizontal_facing(),
                )
            };
            if self.can_place_at(args.world, args.position, direction) {
                return Some((face, facing));
            }
        }
        None
    }

    fn get_placement_face(
        &self,
        player: &Player,
        direction: BlockDirection,
    ) -> (AttachFace, HorizontalFacing) {
        let face = match direction {
            BlockDirection::Up => AttachFace::Ceiling,
            BlockDirection::Down => AttachFace::Floor,
            _ => AttachFace::Wall,
        };

        let facing = if direction == BlockDirection::Up || direction == BlockDirection::Down {
            player.get_entity().get_horizontal_facing()
        } else {
            direction.opposite().to_cardinal_direction()
        };

        (face, facing)
    }

    /// Gets the direction to check for placement validation based on clicked face
    /// This returns the `BlockDirection` that should have a solid surface for placement
    fn get_placement_direction(
        &self,
        player: &Player,
        direction: BlockDirection,
    ) -> BlockDirection {
        let (face, facing) = self.get_placement_face(player, direction);
        match face {
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Wall => facing.to_block_direction(),
        }
    }

    fn can_place_at(
        &self,
        world: &dyn BlockAccessor,
        pos: &BlockPos,
        direction: BlockDirection,
    ) -> bool {
        let block_pos = pos.offset(direction.to_offset());
        let block_state = world.get_block_state(&block_pos);
        block_state.is_side_solid(direction.opposite())
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if self.get_direction(args.state_id, args.block).opposite() == args.direction
            && !self.can_place_at(args.world, args.position, args.direction)
        {
            Block::AIR.default_state.id
        } else {
            args.state_id
        }
    }
}
