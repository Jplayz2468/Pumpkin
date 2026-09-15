use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, RandomTickArgs,
    blocks::plant::{
        PlantBlockBase,
        crop::{CropBlockBase, get_available_moisture},
    },
};
use pumpkin_data::{
    Block, BlockId, BlockStateId, HorizontalFacingExt,
    block_properties::{HorizontalFacing, WallTorchLikeProperties, WheatLikeProperties},
    tag::{self, Taggable},
};
use pumpkin_world::world::BlockFlags;

type StemProperties = WheatLikeProperties;
type AttachedStemProperties = WallTorchLikeProperties;

pub struct StemBlock;

impl BlockMetadata for StemBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::PUMPKIN_STEM, BlockId::MELON_STEM].into()
    }
}

impl StemBlock {
    fn state_with_age(block: &Block, state: BlockStateId, age: i32) -> BlockStateId {
        let mut props = StemProperties::from_state_id(state);
        props.age = age as u8;
        props.to_state_id(block)
    }

    fn get_attached_stem(dir: HorizontalFacing, block: &Block) -> BlockStateId {
        let attached_block = match block.id {
            id if id == Block::PUMPKIN_STEM.id => &Block::ATTACHED_PUMPKIN_STEM,
            id if id == Block::MELON_STEM.id => &Block::ATTACHED_MELON_STEM,
            _ => &Block::ATTACHED_MELON_STEM, // Should never happen
        };
        let mut props = AttachedStemProperties::default(attached_block);
        props.facing = dir;
        props.to_state_id(attached_block)
    }

    fn get_gourd(block: &Block) -> &Block {
        match block.id {
            id if id == Block::PUMPKIN_STEM.id => &Block::PUMPKIN,
            id if id == Block::MELON_STEM.id => &Block::MELON,
            _ => &Block::MELON, // Should never happen
        }
    }
}

impl BlockBehaviour for StemBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        <Self as CropBlockBase>::is_valid_bonemeal_target(self, args.world, args.position)
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        <Self as CropBlockBase>::perform_bonemeal(self, args.world, args.position);
        let (_, state) = args.world.get_block_and_state_id(args.position);
        if StemProperties::from_state_id(state).age == 7 {
            let mut random = crate::block::random::BlockRandom::Shared(&args.world.random);
            BlockBehaviour::random_tick(
                self,
                RandomTickArgs {
                    world: args.world,
                    block: args.block,
                    position: args.position,
                    random: &mut random,
                },
            );
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        stem_supports(
            args.block,
            args.block_accessor.get_block(&args.position.down()),
        )
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if stem_supports(args.block, args.world.get_block(&args.position.down())) {
            args.state_id
        } else {
            Block::AIR.default_state.id
        }
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        if args.world.get_raw_brightness(args.position, 0) < 9 {
            return;
        }
        let f: f32 = get_available_moisture(args.world, args.position, args.block);
        if args.rand_bounded_i32((25.0f32 / f) as i32 + 1) == 0 {
            let (block, state) = args.world.get_block_and_state_id(args.position);
            let props = StemProperties::from_state_id(state);
            let age = i32::from(props.age);
            if age < 7 {
                args.world.set_block_state(
                    args.position,
                    Self::state_with_age(block, state, age + 1),
                    BlockFlags::NOTIFY_LISTENERS,
                );
            } else {
                let horizontals = [
                    HorizontalFacing::North,
                    HorizontalFacing::East,
                    HorizontalFacing::South,
                    HorizontalFacing::West,
                ];
                let facing = horizontals[args.rand_bounded_i32(horizontals.len() as i32) as usize];
                let dir = facing.to_block_direction();
                let plant_block_pos = args.position.offset(dir.to_offset());
                let plant_block_state = args.world.get_block_state(&plant_block_pos);
                let under_block: &Block = args.world.get_block(&plant_block_pos.down());
                if plant_block_state.is_air()
                    && under_block.has_tag(if block == &Block::PUMPKIN_STEM {
                        &tag::Block::MINECRAFT_SUPPORTS_PUMPKIN_STEM_FRUIT
                    } else {
                        &tag::Block::MINECRAFT_SUPPORTS_MELON_STEM_FRUIT
                    })
                {
                    let attached_stem = Self::get_attached_stem(facing, block);
                    let gourd = Self::get_gourd(block);
                    args.world.set_block_state(
                        &plant_block_pos,
                        gourd.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.set_block_state(
                        args.position,
                        attached_stem,
                        BlockFlags::NOTIFY_ALL,
                    );
                }
            }
        }
    }
}

pub(super) fn stem_supports(stem: &Block, support: &Block) -> bool {
    support.has_tag(
        if stem == &Block::PUMPKIN_STEM || stem == &Block::ATTACHED_PUMPKIN_STEM {
            &tag::Block::MINECRAFT_SUPPORTS_PUMPKIN_STEM
        } else {
            &tag::Block::MINECRAFT_SUPPORTS_MELON_STEM
        },
    )
}

impl PlantBlockBase for StemBlock {}
impl CropBlockBase for StemBlock {}
