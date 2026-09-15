use std::sync::Arc;

use super::{double_plant_neighbor_state, double_plant_survives};
use crate::block::blocks::plant::big_dripleaf::{can_grow_into, schedule_water, source_water_at};
use crate::block::blocks::plant::big_dripleaf_stem::BigDripleafStemLikeProperties;
use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    PlayerPlacedArgs,
};
use crate::world::World;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{
    BigDripleafLikeProperties, DoubleBlockHalf, HorizontalFacing, SmallDripleafLikeProperties,
};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block("minecraft:small_dripleaf")]
pub struct SmallDripleafBlock;

impl BlockBehaviour for SmallDripleafBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        if !double_plant_survives(
            args.block_accessor,
            args.block,
            args.state.id,
            args.position,
            lower_survives(args.block_accessor, args.position),
        ) {
            return false;
        }
        if args.use_item_on.is_some() {
            let above = args.position.up();
            let (block, state) = args.block_accessor.get_block_and_state(&above);
            return !args
                .world
                .is_some_and(|world| !world.is_in_height_limit(above.0.y))
                && crate::block::registry::can_replace_with_other_block(block, state);
        }
        true
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        let mut small_dripleaf_props = SmallDripleafLikeProperties::default(args.block);

        small_dripleaf_props.facing = facing;
        small_dripleaf_props.waterlogged = water_at(args.world, args.position);
        small_dripleaf_props.half = DoubleBlockHalf::Lower;

        small_dripleaf_props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if SmallDripleafLikeProperties::from_state_id(args.state_id).waterlogged {
            schedule_water(args.world, args.position);
        }
        double_plant_neighbor_state(&args, lower_survives(args.world, args.position))
    }
    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        {
            let lower_small_dripleaf_props =
                SmallDripleafLikeProperties::from_state_id(args.state_id);
            if lower_small_dripleaf_props.half != DoubleBlockHalf::Lower {
                return;
            }

            let mut upper_small_dripleaf_props =
                SmallDripleafLikeProperties::default(&Block::SMALL_DRIPLEAF);

            upper_small_dripleaf_props.facing = lower_small_dripleaf_props.facing;
            upper_small_dripleaf_props.waterlogged =
                water_at(args.world.as_ref(), &args.position.up());
            upper_small_dripleaf_props.half = DoubleBlockHalf::Upper;

            args.world.set_block_state(
                &args.position.up(),
                upper_small_dripleaf_props.to_state_id(&Block::SMALL_DRIPLEAF),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    // SmallDripleafBlock.java:117 isValidBonemealTarget: always true.
    fn is_valid_bonemeal_target(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    // SmallDripleafBlock.java:127 performBonemeal.
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        perform_small_dripleaf_bonemeal(args.world, args.position, args.state_id);
    }
}

fn perform_small_dripleaf_bonemeal(
    world: &Arc<World>,
    position: &BlockPos,
    state_id: BlockStateId,
) {
    let props = SmallDripleafLikeProperties::from_state_id(state_id);
    if props.half == DoubleBlockHalf::Lower {
        let above_pos = position.up();
        // level.getFluidState(above).createLegacyBlock(): water stays water, anything
        // else collapses to air.
        let (_, fluid) = World::fluid_state_from_block_state(world.get_block_state_id(&above_pos));
        let cleared_state_id = fluid.block_state_id;
        world.set_block_state(
            &above_pos,
            cleared_state_id,
            BlockFlags::SKIP_SHAPE_UPDATES | BlockFlags::NOTIFY_LISTENERS,
        );
        place_dripleaf_with_random_height(world, position, props.facing);
    } else {
        let below_pos = position.down();
        let below_state_id = world.get_block_state_id(&below_pos);
        perform_small_dripleaf_bonemeal(world, &below_pos, below_state_id);
    }
}

// BigDripleafBlock.java:89 placeWithRandomHeight.
fn place_dripleaf_with_random_height(
    world: &Arc<World>,
    stem_bottom_pos: &BlockPos,
    facing: HorizontalFacing,
) {
    let desired_height = world.rand_bounded_i32(4) + 2;
    let mut pos = *stem_bottom_pos;
    let mut height = 0;
    while height < desired_height && can_grow_into(world, &pos) {
        height += 1;
        pos = pos.up();
    }

    let leaf_y = stem_bottom_pos.0.y + height - 1;
    let mut cursor = *stem_bottom_pos;
    while cursor.0.y < leaf_y {
        let mut stem_props = BigDripleafStemLikeProperties::default(&Block::BIG_DRIPLEAF_STEM);
        stem_props.facing = facing;
        stem_props.waterlogged = source_water_at(world.as_ref(), &cursor);
        world.set_block_state(
            &cursor,
            stem_props.to_state_id(&Block::BIG_DRIPLEAF_STEM),
            BlockFlags::NOTIFY_ALL,
        );
        cursor = cursor.up();
    }

    let mut leaf_props = BigDripleafLikeProperties::default(&Block::BIG_DRIPLEAF);
    leaf_props.facing = facing;
    leaf_props.waterlogged = source_water_at(world.as_ref(), &cursor);
    world.set_block_state(
        &cursor,
        leaf_props.to_state_id(&Block::BIG_DRIPLEAF),
        BlockFlags::NOTIFY_ALL,
    );
}
fn lower_survives(accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let support = accessor.get_block(&pos.down());
    support.has_tag(&tag::Block::MINECRAFT_SUPPORTS_SMALL_DRIPLEAF)
        || (source_water_at(accessor, pos)
            && support.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION))
}
fn water_at(accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    World::fluid_state_from_block_state(accessor.get_block_state_id(pos))
        .0
        .matches_type(&pumpkin_data::fluid::Fluid::WATER)
}
