use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, OnLandedUponArgs};

#[pumpkin_block("minecraft:hay_block")]
pub struct HayBlock;

impl BlockBehaviour for HayBlock {
    // HayBlock inherits RotatedPillarBlock.getStateForPlacement; it additionally
    // keeps its own fall-damage multiplier below (HayBlock.java:24).
    fn on_place(&self, args: crate::block::OnPlaceArgs<'_>) -> pumpkin_data::BlockStateId {
        super::logs::LogBlock.on_place(args)
    }

    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity() {
            living.handle_fall_damage(args.entity, args.fall_distance, 0.2);
        }
    }
}
