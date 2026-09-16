use pumpkin_data::block_properties::SlabType;
use pumpkin_data::{BlockDirection, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block_from_tag;

use crate::block::{
    BlockBehaviour, BlockIsReplacing, CanUpdateAtArgs, OnPlaceArgs, PathComputationType,
};

type SlabProperties = pumpkin_data::block_properties::ResinBrickSlabLikeProperties;

#[pumpkin_block_from_tag("minecraft:slabs")]
pub struct SlabBlock;

impl BlockBehaviour for SlabBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let mut slab_props = SlabProperties::from_state_id(state_id);
            slab_props.r#type = SlabType::Double;
            slab_props.waterlogged = false;
            return slab_props.to_state_id(args.block);
        }

        let mut slab_props = SlabProperties::default(args.block);
        let (fluid, fluid_state) = crate::world::World::fluid_state_from_block_state(
            args.world.get_block_state_id(args.position),
        );
        slab_props.waterlogged =
            fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) && fluid_state.is_source;
        slab_props.r#type = match args.direction {
            BlockDirection::Up => SlabType::Top,
            BlockDirection::Down => SlabType::Bottom,
            _ => match args.use_item_on.cursor_pos.y {
                y if y <= 0.5 => SlabType::Bottom,
                _ => SlabType::Top,
            },
        };

        slab_props.to_state_id(args.block)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        let slab_props = SlabProperties::from_state_id(args.state_id);

        if slab_props.r#type == SlabType::Double {
            return false;
        }
        if !args.replacing_clicked {
            return true;
        }
        let above = args.cursor_pos.y > 0.5;
        match slab_props.r#type {
            SlabType::Bottom => {
                args.direction == BlockDirection::Up || (above && args.direction.is_horizontal())
            }
            SlabType::Top => {
                args.direction == BlockDirection::Down || (!above && args.direction.is_horizontal())
            }
            SlabType::Double => false,
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        match computation_type {
            PathComputationType::Water => state.is_waterlogged(),
            PathComputationType::Land | PathComputationType::Air => false,
        }
    }
}
