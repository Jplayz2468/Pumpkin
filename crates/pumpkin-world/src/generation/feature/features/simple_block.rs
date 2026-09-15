use pumpkin_data::{Block, BlockId, tag, tag::Taggable};
use pumpkin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

use crate::generation::proto_chunk::GenerationCache;
use crate::{
    generation::block_state_provider::BlockStateProvider,
    world::{BlockAccessor, WorldPortalExt},
};

pub struct SimpleBlockFeature {
    pub to_place: BlockStateProvider,
    pub schedule_tick: Option<bool>,
}

impl SimpleBlockFeature {
    pub fn generate<T: GenerationCache>(
        &self,
        block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let Some(state) = self
            .to_place
            .get_optional(block_registry, chunk, random, pos)
        else {
            return false;
        };
        let block = Block::from_state_id(state.id);
        let block_accessor: &dyn BlockAccessor = chunk;
        if !block_registry.can_place_at(block, state, block_accessor, &pos) {
            return false;
        }

        let flags = crate::world::BlockFlags::NOTIFY_LISTENERS;
        let properties = block.properties(state.id).map(|p| p.to_props());
        if block.has_tag(&tag::Block::MINECRAFT_TALL_FLOWERS)
            || matches!(
                block.id,
                BlockId::TALL_GRASS
                    | BlockId::LARGE_FERN
                    | BlockId::TALL_SEAGRASS
                    | BlockId::SMALL_DRIPLEAF
                    | BlockId::PITCHER_CROP
            )
        {
            if !chunk.is_air(&pos.up().0) {
                return false;
            }
            for (target, half) in [(pos, "lower"), (pos.up(), "upper")] {
                let mut props = properties.clone().unwrap_or_default();
                for (key, value) in &mut props {
                    if *key == "half" {
                        *value = half;
                    }
                    if *key == "waterlogged" {
                        *value = if chunk
                            .get_fluid_and_fluid_state(&target.0)
                            .0
                            .matches_type(&pumpkin_data::fluid::Fluid::WATER)
                        {
                            "true"
                        } else {
                            "false"
                        };
                    }
                }
                let placed = block.from_properties(&props).to_state_id(block);
                chunk.set_block_state_with_flags(&target.0, placed.to_state(), flags);
            }
        } else if block == &Block::PALE_MOSS_CARPET {
            use crate::block::mossy_carpet;
            let base = mossy_carpet::updated_state(chunk, &pos, block.default_state.id, true);
            chunk.set_block_state_with_flags(&pos.0, base.to_state(), flags);
            let top = mossy_carpet::create_topper(chunk, &pos, || random.next_bool());
            if top != pumpkin_data::BlockStateId::AIR {
                chunk.set_block_state_with_flags(&pos.up().0, top.to_state(), flags);
                let updated = mossy_carpet::updated_state(chunk, &pos, base, true);
                chunk.set_block_state_with_flags(&pos.0, updated.to_state(), flags);
            }
        } else {
            chunk.set_block_state_with_flags(&pos.0, state, flags);
        }
        if self.schedule_tick.unwrap_or(false) {
            let block = GenerationCache::get_block_state(chunk, &pos.0).to_block();
            chunk.schedule_block_tick(pos, block, 1);
        }
        true
    }
}
