use crate::{
    block::{
        BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
        OnEntityCollisionArgs, RandomTickArgs,
    },
    world::World,
};
use pumpkin_data::block_properties::{DoubleBlockHalf, PitcherCropLikeProperties};
use pumpkin_data::{Block, BlockDirection, BlockStateId, tag, tag::Taggable};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

pub const MAX_AGE: u8 = 4;

#[pumpkin_block("minecraft:pitcher_crop")]
pub struct PitcherCropBlock;

impl PitcherCropBlock {
    fn can_survive(
        world: Option<&World>,
        accessor: &dyn BlockAccessor,
        pos: &BlockPos,
        props: &PitcherCropLikeProperties,
    ) -> bool {
        let (below, state) = accessor.get_block_and_state(&pos.down());
        if props.half == DoubleBlockHalf::Lower {
            world.is_none_or(|world| world.get_raw_brightness(pos, 0) >= 8)
                && below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CROPS)
        } else {
            below == &Block::PITCHER_CROP
                && PitcherCropLikeProperties::from_state_id(state.id).half == DoubleBlockHalf::Lower
        }
    }

    fn lower_half(
        world: &World,
        pos: &BlockPos,
        state: BlockStateId,
    ) -> Option<(BlockPos, PitcherCropLikeProperties)> {
        if state.to_block() == &Block::PITCHER_CROP {
            let props = PitcherCropLikeProperties::from_state_id(state);
            if props.half == DoubleBlockHalf::Lower {
                return Some((*pos, props));
            }
        }
        let below_pos = pos.down();
        let (below, state) = world.get_block_and_state(&below_pos);
        if below != &Block::PITCHER_CROP {
            return None;
        }
        let props = PitcherCropLikeProperties::from_state_id(state.id);
        (props.half == DoubleBlockHalf::Lower).then_some((below_pos, props))
    }

    fn can_grow(world: &World, pos: &BlockPos, props: &PitcherCropLikeProperties, age: u8) -> bool {
        props.age < MAX_AGE
            && world.get_raw_brightness(pos, 0) >= 8
            && world.is_in_height_limit(pos.up().0.y)
            && (age < 3 || {
                let (above, state) = world.get_block_and_state(&pos.up());
                state.is_air() || above == &Block::PITCHER_CROP
            })
    }

    fn grow(world: &Arc<World>, pos: &BlockPos, mut props: PitcherCropLikeProperties) {
        let age = (props.age + 1).min(MAX_AGE);
        if !Self::can_grow(world, pos, &props, age) {
            return;
        }
        props.age = age;
        world.set_block_state(
            pos,
            props.to_state_id(&Block::PITCHER_CROP),
            BlockFlags::NOTIFY_LISTENERS,
        );
        if age >= 3 {
            props.half = DoubleBlockHalf::Upper;
            world.set_block_state(
                &pos.up(),
                props.to_state_id(&Block::PITCHER_CROP),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}

impl BlockBehaviour for PitcherCropBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(
            args.world,
            args.block_accessor,
            args.position,
            &PitcherCropLikeProperties::from_state_id(args.state.id),
        )
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = PitcherCropLikeProperties::from_state_id(args.state_id);
        if props.age >= 3 {
            let other_direction = if props.half == DoubleBlockHalf::Lower {
                BlockDirection::Up
            } else {
                BlockDirection::Down
            };
            if args.direction == other_direction {
                if args.neighbor_state_id.to_block() != &Block::PITCHER_CROP
                    || PitcherCropLikeProperties::from_state_id(args.neighbor_state_id).half
                        == props.half
                {
                    return Block::AIR.default_state.id;
                }
            }
        }
        if Self::can_survive(Some(args.world), args.world, args.position, &props) {
            args.state_id
        } else {
            Block::AIR.default_state.id
        }
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        let props =
            PitcherCropLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if props.half == DoubleBlockHalf::Lower && props.age < MAX_AGE {
            let speed = super::get_available_moisture(args.world, args.position, args.block);
            if args.rand_bounded_i32((25.0_f32 / speed) as i32 + 1) == 0 {
                Self::grow(args.world, args.position, props);
            }
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        Self::lower_half(args.world, args.position, args.state_id)
            .is_some_and(|(pos, props)| Self::can_grow(args.world, &pos, &props, props.age + 1))
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if let Some((pos, props)) = Self::lower_half(args.world, args.position, args.state_id) {
            Self::grow(args.world, &pos, props);
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        super::ravager_collision(args);
    }
}
