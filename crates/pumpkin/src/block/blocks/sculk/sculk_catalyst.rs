use crate::block::{
    BlockBehaviour, BlockMetadata, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
    entities::sculk_catalyst::SculkCatalystBlockEntity,
};
use pumpkin_data::{BlockId, BlockStateId, block_properties::SculkCatalystLikeProperties};
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

pub struct SculkCatalystBlock;

impl BlockMetadata for SculkCatalystBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_CATALYST].into()
    }
}

impl BlockBehaviour for SculkCatalystBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        if args.world.get_block_entity(args.position).is_none() {
            args.world
                .add_block_entity(Arc::new(SculkCatalystBlockEntity::new(*args.position)));
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let mut props = SculkCatalystLikeProperties::from_state_id(
            args.world.get_block_state_id(args.position),
        );
        if props.bloom {
            props.bloom = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SculkCatalystLikeProperties::default(args.block);
        props.bloom = false;
        props.to_state_id(args.block)
    }
}
