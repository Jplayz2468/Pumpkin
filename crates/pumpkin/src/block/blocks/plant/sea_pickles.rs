use crate::block::BlockIsReplacing;
use crate::block::blocks::plant::PlantBlockBase;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, CanUpdateAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, PathComputationType, UseWithItemArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::entity::EntityPose;
use pumpkin_data::item::Item;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

type SeaPickleProperties = pumpkin_data::block_properties::SeaPickleLikeProperties;

#[pumpkin_block("minecraft:sea_pickle")]
pub struct SeaPickleBlock;

impl BlockBehaviour for SeaPickleBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        {
            if args.item_stack.item != &Item::BONE_MEAL
                || !args
                    .world
                    .get_block(&args.position.down())
                    .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
                || !SeaPickleProperties::from_state_id(args.world.get_block_state_id(args.position))
                    .waterlogged
            {
                return BlockActionResult::Pass;
            }

            //1:1 vanilla algorithm
            //TODO use pumpkin random

            //let mut j = 1;
            let mut count = 0;
            let base_x = args.position.0.x - 2;
            let mut removed_z = 0;
            for added_x in 0..5 {
                for added_z in 0..1 {
                    let temp_y = 2 + args.position.0.y - 1;
                    for y in (temp_y - 2)..temp_y {
                        //let mut lv2: BlockState;
                        let lv = BlockPos::new(
                            base_x + added_x,
                            y,
                            args.position.0.z - removed_z + added_z,
                        );
                        if &lv == args.position
                            || rand::rng().random_range(0..6) != 0
                            || !args.world.get_block(&lv).eq(&Block::WATER)
                            || !args
                                .world
                                .get_block(&lv.down())
                                .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
                        {
                            continue;
                        }
                        let mut sea_pickle_prop = SeaPickleProperties::default(args.block);

                        sea_pickle_prop.pickles = rand::rng().random_range(1..=4);
                        args.world.set_block_state(
                            &lv,
                            sea_pickle_prop.to_state_id(args.block),
                            BlockFlags::NOTIFY_ALL,
                        );
                    }
                }
                if count < 2 {
                    //j += 2;
                    removed_z += 1;
                } else {
                    //j -= 2;
                    removed_z -= 1;
                }
                count += 1;
            }
            let mut sea_pickle_prop = SeaPickleProperties::default(args.block);
            sea_pickle_prop.pickles = 4;
            args.world.set_block_state(
                args.position,
                sea_pickle_prop.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );

            BlockActionResult::Consume
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if args.player.get_entity().pose.load() != EntityPose::Crouching
            && let BlockIsReplacing::Itself(state_id) = args.replacing
        {
            let mut sea_pickle_prop = SeaPickleProperties::from_state_id(state_id);
            if sea_pickle_prop.pickles < 4 {
                sea_pickle_prop.pickles += 1;
            }
            return sea_pickle_prop.to_state_id(args.block);
        }

        let mut sea_pickle_prop = SeaPickleProperties::default(args.block);
        sea_pickle_prop.waterlogged = args.replacing.water_source();
        sea_pickle_prop.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support_block = args.block_accessor.get_block_state(&args.position.down());
        support_block.is_center_solid(BlockDirection::Up)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        args.player.get_entity().pose.load() != EntityPose::Crouching
            && SeaPickleProperties::from_state_id(args.state_id).pickles < 4
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
                        && rand::rng().random_range(0..6) == 0
                        && args.world.get_block(&pos) == &Block::WATER
                        && args
                            .world
                            .get_block(&pos.down())
                            .has_tag(&tag::Block::MINECRAFT_CORAL_BLOCKS)
                    {
                        let mut props = SeaPickleProperties::default(&Block::SEA_PICKLE);
                        props.pickles = rand::rng().random_range(1..=4);
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

impl PlantBlockBase for SeaPickleBlock {}
