use crate::block::{BrokenArgs, PlacedArgs};
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{DoubleBlockHalf, TallSeagrassLikeProperties};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::PlantBlockBase,
};

pub struct TallPlantBlock;

/// Vanilla registers sunflower/lilac/rose_bush/peony with `TallFlowerBlock` (which extends
/// `DoublePlantBlock` and implements `BonemealableBlock`), while tall_grass/large_fern/
/// pitcher_plant are plain `DoublePlantBlock` with no bonemeal support at all
/// (Blocks.java:3087-3160, 3760-3765 in comparison/vanilla-src). Since this struct covers
/// both groups, bonemeal must only ever apply to the four flower variants.
fn tall_flower_item(block: &Block) -> Option<&'static Item> {
    if block == &Block::SUNFLOWER {
        Some(&Item::SUNFLOWER)
    } else if block == &Block::LILAC {
        Some(&Item::LILAC)
    } else if block == &Block::ROSE_BUSH {
        Some(&Item::ROSE_BUSH)
    } else if block == &Block::PEONY {
        Some(&Item::PEONY)
    } else {
        None
    }
}

impl BlockMetadata for TallPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::TALL_GRASS,
            BlockId::LARGE_FERN,
            BlockId::PITCHER_PLANT,
            // TallFlowerBlocks
            BlockId::SUNFLOWER,
            BlockId::LILAC,
            BlockId::PEONY,
            BlockId::ROSE_BUSH,
        ]
        .into()
    }
}

impl BlockBehaviour for TallPlantBlock {
    // TallFlowerBlock.isValidBonemealTarget / isBonemealSuccess (TallFlowerBlock.java:26-33)
    // are unconditionally true for the flower half you clicked (upper or lower); there is no
    // growth stage to check, unlike other bonemealable plants.
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        tall_flower_item(args.block).is_some()
    }

    fn is_bonemeal_success(&self, args: BonemealArgs<'_>) -> bool {
        tall_flower_item(args.block).is_some()
    }

    // TallFlowerBlock.performBonemeal (TallFlowerBlock.java:36-38) does not grow anything; it
    // just pops one extra copy of the flower itself at the clicked position, giving a
    // renewable way to duplicate sunflowers/lilacs/rose bushes/peonies with bone meal.
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if let Some(item) = tall_flower_item(args.block) {
            args.world.drop_stack(args.position, ItemStack::new(1, item));
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let up_pos = args.position.up();

        let upper_state = args.block_accessor.get_block_state(&up_pos);
        let Some(world) = args.world else {
            return <Self as PlantBlockBase>::can_place_at(
                self,
                args.block_accessor,
                args.position,
            ) && upper_state.is_air();
        };

        if up_pos.0.y > world.get_top_y() {
            return false;
        }
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
            && upper_state.is_air()
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state_id);
        let (support_block_pos, other_block_pos) = match tall_plant_props.half {
            DoubleBlockHalf::Upper => (args.position.down_height(2), args.position.down()),
            DoubleBlockHalf::Lower => (args.position.down(), args.position.up()),
        };
        if !<Self as PlantBlockBase>::can_place_at(self, args.world, &support_block_pos.up()) {
            return Block::AIR.default_state.id;
        }

        let (other_block, other_state_id) = args.world.get_block_and_state_id(&other_block_pos);
        if Self::ids().contains(&other_block.id) {
            let other_props = TallSeagrassLikeProperties::from_state_id(other_state_id);
            let opposite_half = match tall_plant_props.half {
                DoubleBlockHalf::Upper => DoubleBlockHalf::Lower,
                DoubleBlockHalf::Lower => DoubleBlockHalf::Upper,
            };
            if other_props.half == opposite_half {
                return args.state_id;
            }
        }
        Block::AIR.default_state.id
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let mut tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state_id);
            tall_plant_props.half = DoubleBlockHalf::Upper;
            args.world.set_block_state(
                &args.position.offset(BlockDirection::Up.to_offset()),
                tall_plant_props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK,
            );
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        {
            // When one half of a tall plant is broken, break the other half too
            let tall_plant_props = TallSeagrassLikeProperties::from_state_id(args.state.id);
            let other_block_pos = match tall_plant_props.half {
                DoubleBlockHalf::Upper => args.position.down(),
                DoubleBlockHalf::Lower => args.position.up(),
            };
            let (other_block, other_state_id) = args.world.get_block_and_state_id(&other_block_pos);
            if Self::ids().contains(&other_block.id) {
                let other_props = TallSeagrassLikeProperties::from_state_id(other_state_id);
                let opposite_half = match tall_plant_props.half {
                    DoubleBlockHalf::Upper => DoubleBlockHalf::Lower,
                    DoubleBlockHalf::Lower => DoubleBlockHalf::Upper,
                };
                if other_props.half == opposite_half {
                    // Break the other half, using SKIP_DROPS to prevent double drops
                    args.world.break_block(
                        &other_block_pos,
                        None,
                        BlockFlags::SKIP_DROPS
                            | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
                            | BlockFlags::NOTIFY_ALL,
                    );
                }
            }
        }
    }
}

impl PlantBlockBase for TallPlantBlock {}
