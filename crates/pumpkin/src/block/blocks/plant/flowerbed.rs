use pumpkin_data::BlockStateId;
use pumpkin_data::{BlockId, item::Item, item_stack::ItemStack};
use pumpkin_world::world::BlockFlags;

use crate::block::blocks::plant::PlantBlockBase;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, CanUpdateAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs,
};

use super::segmented::Segmented;

type FlowerbedProperties = pumpkin_data::block_properties::PinkPetalsLikeProperties;

pub struct FlowerbedBlock;

impl BlockMetadata for FlowerbedBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::PINK_PETALS, BlockId::WILDFLOWERS].into()
    }
}

impl BlockBehaviour for FlowerbedBlock {
    fn is_valid_bonemeal_target(&self, _args: crate::block::BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        let mut props = FlowerbedProperties::from_state_id(args.state_id);
        if props.flower_amount < 4 {
            props.flower_amount += 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        } else if let Some(item) = Item::from_id(args.block.item_id) {
            args.world
                .drop_stack(args.position, ItemStack::new(1, item));
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        Segmented::can_update_at(self, args)
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        Segmented::on_place(self, args)
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
}

impl PlantBlockBase for FlowerbedBlock {}

impl Segmented for FlowerbedBlock {
    type Properties = FlowerbedProperties;
}
