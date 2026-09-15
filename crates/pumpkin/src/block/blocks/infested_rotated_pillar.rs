use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;

use crate::block::BlockBehaviour;
use crate::block::OnPlaceArgs;

// InfestedRotatedPillarBlock.java extends InfestedBlock and additionally carries the
// vanilla RotatedPillarBlock AXIS property (see RotatedPillarBlock#getStateForPlacement /
// #rotate). infested_deepslate is the only member of this family.
type InfestedDeepslateProperties = pumpkin_data::block_properties::InfestedDeepslateProperties;

#[pumpkin_block("minecraft:infested_deepslate")]
pub struct InfestedRotatedPillarBlock;

impl BlockBehaviour for InfestedRotatedPillarBlock {
    // InfestedRotatedPillarBlock.java:39-41 getStateForPlacement sets AXIS to the clicked
    // face's axis, same as any other rotated-pillar block (see logs.rs LogBlock).
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = InfestedDeepslateProperties::default(args.block);
        props.axis = args.direction.to_axis();

        props.to_state_id(args.block)
    }

    fn spawn_after_break(&self, args: crate::block::SpawnAfterBreakArgs<'_>) {
        super::infested::InfestedBlock.spawn_after_break(args);
    }
}
