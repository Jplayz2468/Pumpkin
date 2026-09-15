use pumpkin_data::{BlockId, BlockStateId, tag};

use crate::block::BlockBehaviour;
use crate::block::BlockMetadata;
use crate::block::OnPlaceArgs;

type LogProperties = pumpkin_data::block_properties::PaleOakWoodLikeProperties;

/// Handles every vanilla `RotatedPillarBlock` (RotatedPillarBlock.java): axis is set
/// from the clicked face on placement (`getStateForPlacement`) and nothing else, so one
/// struct covers all of them.
///
/// Most members are the wood/stem families reachable through the `minecraft:logs` tag
/// (`logs_that_burn` + `crimson_stems` + `warped_stems`). The remaining vanilla
/// `RotatedPillarBlock` instances (basalt/polished_basalt, bone_block, deepslate,
/// muddy_mangrove_roots, purpur_pillar, quartz_pillar, bamboo_block/stripped_bamboo_block,
/// the froglights) have no tag of their own, so they are listed explicitly.
pub struct LogBlock;

impl BlockMetadata for LogBlock {
    fn ids() -> Box<[BlockId]> {
        let tagged = tag::get_tag_ids(tag::RegistryKey::Block, "minecraft:logs")
            .unwrap_or_else(|| panic!("Failed to get tag IDs for: minecraft:logs"))
            .iter()
            .copied()
            .map(BlockId::new_or_air);

        let untagged = [
            BlockId::BASALT,
            BlockId::POLISHED_BASALT,
            BlockId::BONE_BLOCK,
            BlockId::DEEPSLATE,
            BlockId::MUDDY_MANGROVE_ROOTS,
            BlockId::PURPUR_PILLAR,
            BlockId::QUARTZ_PILLAR,
            BlockId::BAMBOO_BLOCK,
            BlockId::STRIPPED_BAMBOO_BLOCK,
            BlockId::OCHRE_FROGLIGHT,
            BlockId::PEARLESCENT_FROGLIGHT,
            BlockId::VERDANT_FROGLIGHT,
        ];

        tagged.chain(untagged).collect()
    }
}

impl BlockBehaviour for LogBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut log_props = LogProperties::default(args.block);
        log_props.axis = args.direction.to_axis();

        log_props.to_state_id(args.block)
    }
}
