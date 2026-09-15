use crate::block::BlockIsReplacing;
use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, CanUpdateAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, PathComputationType,
};
use crate::entity::EntityBase;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

type SeaPickleProperties = pumpkin_data::block_properties::SeaPickleLikeProperties;

#[pumpkin_block("minecraft:sea_pickle")]
pub struct SeaPickleBlock;

impl BlockBehaviour for SeaPickleBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let mut sea_pickle_prop = SeaPickleProperties::from_state_id(state_id);
            if sea_pickle_prop.pickles < 4 {
                sea_pickle_prop.pickles += 1;
            }
            return sea_pickle_prop.to_state_id(args.block);
        }

        let mut sea_pickle_prop = SeaPickleProperties::default(args.block);
        let (fluid, state) = crate::world::World::fluid_state_from_block_state(
            args.world.get_block_state_id(args.position),
        );
        sea_pickle_prop.waterlogged =
            fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) && state.is_source;
        sea_pickle_prop.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        supports_pickle(args.block_accessor, &args.position.down())
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        !args.player.get_entity().is_sneaking()
            && SeaPickleProperties::from_state_id(args.state_id).pickles < 4
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !supports_pickle(args.world, &args.position.down()) {
            return Block::AIR.default_state.id;
        }
        if SeaPickleProperties::from_state_id(args.state_id).waterlogged {
            args.world.schedule_fluid_tick(
                &pumpkin_data::fluid::Fluid::WATER,
                *args.position,
                pumpkin_data::fluid::Fluid::WATER.flow_speed as u32,
                pumpkin_world::tick::TickPriority::Normal,
            );
        }
        args.state_id
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }

    // SeaPickleBlock.java:124 isValidBonemealTarget: alive (waterlogged) pickle on coral.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let props = SeaPickleProperties::from_state_id(args.state_id);
        props.waterlogged
            && args
                .world
                .get_block(&args.position.down())
                .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
    }

    // SeaPickleBlock.java:134 performBonemeal: 1:1 port, including the diamond-shaped
    // x/z search pattern (zSpan grows 1,3,5,3,1 across the five x columns).
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let mut z_span = 1;
        let mut z_offset = 0;
        let x_start = args.position.0.x - 2;

        for (count, x) in (0..5).enumerate() {
            for z in 0..z_span {
                let end_y = 2 + args.position.0.y - 1;
                for start_y in (end_y - 2)..end_y {
                    let pos = BlockPos::new(x_start + x, start_y, args.position.0.z - z_offset + z);
                    if &pos != args.position
                        && args.world.rand_bounded_i32(6) == 0
                        && args.world.get_block(&pos) == &Block::WATER
                        && args
                            .world
                            .get_block(&pos.down())
                            .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
                    {
                        let mut props = SeaPickleProperties::default(&Block::SEA_PICKLE);
                        props.pickles = (args.world.rand_bounded_i32(4) + 1) as u8;
                        args.world.set_block_state(
                            &pos,
                            props.to_state_id(&Block::SEA_PICKLE),
                            BlockFlags::NOTIFY_ALL,
                        );
                    }
                }
            }
            if count < 2 {
                z_span += 2;
                z_offset += 1;
            } else {
                z_span -= 2;
                z_offset -= 1;
            }
        }

        let mut props = SeaPickleProperties::from_state_id(args.state_id);
        props.pickles = 4;
        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }
}

fn supports_pickle(accessor: &dyn pumpkin_world::world::BlockAccessor, pos: &BlockPos) -> bool {
    let state = accessor.get_block_state(pos);
    state.is_side_solid(BlockDirection::Up)
        || state.get_block_collision_shapes_at(pos).any(|shape| {
            // VoxelShape.getFaceShape(UP) slices at 1 - 1e-7.
            shape.min.y <= 0.9999999
                && shape.max.y > 0.9999999
                && shape.max.x > shape.min.x
                && shape.max.z > shape.min.z
        })
}
