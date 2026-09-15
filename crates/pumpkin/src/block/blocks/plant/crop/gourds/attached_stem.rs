use pumpkin_data::{
    Block, BlockId, BlockStateId,
    block_properties::{WallTorchLikeProperties, WheatLikeProperties},
};

use crate::block::{BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs};

type AttachedStemProperties = WallTorchLikeProperties;

type StemProperties = WheatLikeProperties;
pub struct AttachedStemBlock;

impl BlockMetadata for AttachedStemBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::ATTACHED_PUMPKIN_STEM, BlockId::ATTACHED_MELON_STEM].into()
    }
}

impl AttachedStemBlock {
    fn get_stem(block: &Block) -> &Block {
        match block.id {
            id if id == Block::ATTACHED_PUMPKIN_STEM.id => &Block::PUMPKIN_STEM,
            id if id == Block::ATTACHED_MELON_STEM.id => &Block::MELON_STEM,
            _ => &Block::MELON_STEM, // Should never happen
        }
    }

    fn get_gourd(block: &Block) -> &Block {
        match block.id {
            id if id == Block::ATTACHED_PUMPKIN_STEM.id => &Block::PUMPKIN,
            id if id == Block::ATTACHED_MELON_STEM.id => &Block::MELON,
            _ => &Block::MELON, // Should never happen
        }
    }
}

impl BlockBehaviour for AttachedStemBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        super::stem::stem_supports(
            args.block,
            args.block_accessor.get_block(&args.position.down()),
        )
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = AttachedStemProperties::from_state_id(args.state_id);
        if args.direction.to_horizontal_facing() == Some(props.facing)
            && args.neighbor_state_id.to_block() != Self::get_gourd(args.block)
        {
            let mut props = StemProperties::default(Self::get_stem(args.block));
            props.age = 7;
            return props.to_state_id(Self::get_stem(args.block));
        }
        if super::stem::stem_supports(args.block, args.world.get_block(&args.position.down())) {
            args.state_id
        } else {
            Block::AIR.default_state.id
        }
    }
}
