use std::sync::Arc;

use pumpkin_data::BlockStateId;
use pumpkin_data::entity::EntityType;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::GameMode;

use crate::block::BlockBehaviour;
use crate::block::BrokenArgs;
use crate::block::OnPlaceArgs;
use crate::entity::Entity;

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

    // InfestedBlock.java:60-66 spawnAfterBreak spawns a silverfish when the block is
    // broken (unless drops are disabled / prevented by enchantment); mirrors InfestedBlock
    // in infested.rs, which this family inherits from in vanilla.
    fn broken(&self, args: BrokenArgs<'_>) {
        {
            // TODO: ugly fix, use onStacksDropped
            if args.player.gamemode.load() == GameMode::Creative {
                return;
            }
            let entity = Entity::new(
                args.world.clone(),
                args.position.0.to_f64(),
                &EntityType::SILVERFISH,
            );

            args.world.spawn_entity(Arc::new(entity));
        }
    }
}
