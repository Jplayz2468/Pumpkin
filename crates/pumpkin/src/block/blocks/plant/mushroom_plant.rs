use std::sync::Arc;

use crate::world::generation_cache::WorldGenerationCache;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId, tag};
use pumpkin_data::{
    configured_feature::ConfiguredFeature as FeatureKey, placed_feature::PlacedFeature,
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::generation::feature::configured_features::{
    CONFIGURED_FEATURES, ConfiguredFeature,
};
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    RandomTickArgs, blocks::plant::PlantBlockBase,
};
use crate::plugin::api::events::world::structure_grow::{StructureGrowEvent, TreeType};
use crate::world::World;

pub struct MushroomPlantBlock;

impl BlockMetadata for MushroomPlantBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BROWN_MUSHROOM, BlockId::RED_MUSHROOM].into()
    }
}

impl MushroomPlantBlock {
    #[must_use]
    pub const fn may_place_on(state: &BlockState) -> bool {
        state.is_solid_render()
    }

    pub fn can_survive(
        block_accessor: &dyn BlockAccessor,
        world: Option<&World>,
        pos: &BlockPos,
    ) -> bool {
        let below_pos = pos.down();
        let below_block = block_accessor.get_block(&below_pos);
        if below_block.has_tag(&tag::Block::MINECRAFT_OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT) {
            return true;
        }

        let is_dark_enough = world.is_none_or(|world| world.get_raw_brightness(pos, 0) < 13);

        is_dark_enough && Self::may_place_on(block_accessor.get_block_state(&below_pos))
    }

    pub fn grow_mushroom(world: &Arc<World>, pos: &BlockPos, block: &Block) -> bool {
        let species = if block == &Block::BROWN_MUSHROOM {
            TreeType::BrownMushroom
        } else if block == &Block::RED_MUSHROOM {
            TreeType::RedMushroom
        } else {
            TreeType::Custom
        };

        let mut event = StructureGrowEvent::new(*pos, species, true);
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        let key = if block == &Block::BROWN_MUSHROOM {
            FeatureKey::HugeBrownMushroom
        } else {
            FeatureKey::HugeRedMushroom
        };
        let Some(feature) = CONFIGURED_FEATURES.get(&key) else {
            return false;
        };
        let original = world.get_block_state_id(pos);
        // Vanilla removes the small mushroom before trying the feature and restores
        // it on failure, including the corresponding neighbor notifications.
        world.set_block_state(pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
        let mut cache = WorldGenerationCache::new(world.clone(), pos);
        let generated = cache.generate_with_level_random(|cache, random| match feature {
            ConfiguredFeature::HugeBrownMushroom(feature) => feature.generate(
                cache,
                world.dimension.min_y as i8,
                world.dimension.height as u16,
                PlacedFeature::BrownMushroomNormal,
                random,
                *pos,
            ),
            ConfiguredFeature::HugeRedMushroom(feature) => feature.generate(
                cache,
                world.dimension.min_y as i8,
                world.dimension.height as u16,
                PlacedFeature::RedMushroomNormal,
                random,
                *pos,
            ),
            _ => false,
        });
        if generated {
            cache.apply();
        } else {
            world.set_block_state(pos, original, BlockFlags::NOTIFY_ALL);
        }
        generated
    }
}

impl BlockBehaviour for MushroomPlantBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(args.block_accessor, args.world, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !Self::can_survive(args.world, Some(args.world), args.position) {
            return Block::AIR.default_state.id;
        }
        args.state_id
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        if args.rand_bounded_i32(25) != 0 {
            return;
        }
        let pos = *args.position;
        let world = args.world;
        let this_block = args.block;
        let state_id = world.get_block_state_id(&pos);

        let mut max = 5;
        for dz in -4..=4 {
            for dy in -1..=1 {
                for dx in -4..=4 {
                    let check_pos = pos.add(dx, dy, dz);
                    if world.get_block(&check_pos) == this_block {
                        max -= 1;
                        if max <= 0 {
                            return;
                        }
                    }
                }
            }
        }

        let mut current_pos = pos;
        let mut offset = current_pos.add(
            args.rand_bounded_i32(3) - 1,
            args.rand_bounded_i32(2) - args.rand_bounded_i32(2),
            args.rand_bounded_i32(3) - 1,
        );

        for _ in 0..4 {
            if world.get_block_state(&offset).is_air()
                && Self::can_survive(world.as_ref(), Some(world.as_ref()), &offset)
            {
                current_pos = offset;
            }
            offset = current_pos.add(
                args.rand_bounded_i32(3) - 1,
                args.rand_bounded_i32(2) - args.rand_bounded_i32(2),
                args.rand_bounded_i32(3) - 1,
            );
        }

        if world.get_block_state(&offset).is_air()
            && Self::can_survive(world.as_ref(), Some(world.as_ref()), &offset)
        {
            world.set_block_state(&offset, state_id, BlockFlags::NOTIFY_LISTENERS);
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let foliage_radius = if args.block == &Block::BROWN_MUSHROOM {
            3
        } else {
            2
        };
        let min_height = 4 + foliage_radius;
        args.world
            .is_in_height_limit(args.position.0.y + min_height)
    }

    fn is_bonemeal_success(&self, args: BonemealArgs<'_>) -> bool {
        args.world.rand_f32() < 0.4
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        Self::grow_mushroom(args.world, args.position, args.block);
    }
}

impl PlantBlockBase for MushroomPlantBlock {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let state = block_accessor.get_block_state(pos);
        Self::may_place_on(state)
    }

    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        Self::can_survive(block_accessor, None, block_pos)
    }
}
