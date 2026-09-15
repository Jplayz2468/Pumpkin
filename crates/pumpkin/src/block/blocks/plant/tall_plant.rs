use crate::block::{BrokenArgs, PlayerPlacedArgs};
use pumpkin_data::Block;
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{DoubleBlockHalf, TallSeagrassLikeProperties};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    blocks::plant::{PlantBlockBase, double_plant_neighbor_state, double_plant_survives},
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
            args.world
                .drop_stack(args.position, ItemStack::new(1, item));
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support =
            <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position);
        if !double_plant_survives(
            args.block_accessor,
            args.block,
            args.state.id,
            args.position,
            support,
        ) {
            return false;
        }
        // Block placement needs room for the upper half; survival checks do not.
        if args.use_item_on.is_some() {
            let above = args.position.up();
            let (block, state) = args.block_accessor.get_block_and_state(&above);
            if args
                .world
                .is_some_and(|world| !world.is_in_height_limit(above.0.y))
                || !crate::block::registry::can_replace_with_other_block(block, state)
            {
                return false;
            }
        }
        true
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let support = <Self as PlantBlockBase>::can_place_at(self, args.world, args.position);
        double_plant_neighbor_state(&args, support)
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        let mut props = TallSeagrassLikeProperties::default(args.block);
        props.half = DoubleBlockHalf::Upper;
        args.world.set_block_state(
            &args.position.up(),
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
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
            if other_block == args.block {
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
