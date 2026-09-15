use std::sync::Arc;

use pumpkin_data::{
    Block, BlockStateId, configured_feature::ConfiguredFeature as FeatureKey, tag, tag::Taggable,
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::{
    generation::feature::configured_features::{CONFIGURED_FEATURES, ConfiguredFeature},
    world::BlockFlags,
};

use crate::world::{World, generation_cache::WorldGenerationCache};

struct TwoByTwoSaplingPos {
    offset_x: i32,
    offset_z: i32,
    saplings: Vec<(BlockPos, BlockStateId)>,
}

pub struct TreeGrower {
    trees: &'static [FeatureKey],
    mega_trees: &'static [FeatureKey],
    flower_trees: &'static [FeatureKey],
    shortest: Option<FeatureKey>,
    secondary_chance: f32,
}

impl TreeGrower {
    pub const OAK: Self = Self {
        trees: &[FeatureKey::Oak, FeatureKey::FancyOak],
        mega_trees: &[],
        flower_trees: &[FeatureKey::OakBees005, FeatureKey::FancyOakBees005],
        shortest: Some(FeatureKey::Oak),
        secondary_chance: 0.1,
    };
    pub const SPRUCE: Self = Self {
        trees: &[FeatureKey::Spruce],
        mega_trees: &[FeatureKey::MegaSpruce, FeatureKey::MegaPine],
        flower_trees: &[],
        shortest: Some(FeatureKey::Spruce),
        secondary_chance: 0.5,
    };
    pub const MANGROVE: Self = Self {
        trees: &[FeatureKey::Mangrove, FeatureKey::TallMangrove],
        mega_trees: &[],
        flower_trees: &[],
        shortest: Some(FeatureKey::Mangrove),
        secondary_chance: 0.85,
    };
    pub const AZALEA: Self = Self {
        trees: &[FeatureKey::AzaleaTree],
        mega_trees: &[],
        flower_trees: &[],
        shortest: Some(FeatureKey::AzaleaTree),
        secondary_chance: 0.0,
    };
    pub const BIRCH: Self = Self {
        trees: &[FeatureKey::Birch],
        mega_trees: &[],
        flower_trees: &[FeatureKey::BirchBees005],
        shortest: Some(FeatureKey::Birch),
        secondary_chance: 0.0,
    };
    pub const JUNGLE: Self = Self {
        trees: &[FeatureKey::JungleTreeNoVine],
        mega_trees: &[FeatureKey::MegaJungleTree],
        flower_trees: &[],
        shortest: Some(FeatureKey::JungleTreeNoVine),
        secondary_chance: 0.0,
    };
    pub const ACACIA: Self = Self {
        trees: &[FeatureKey::Acacia],
        mega_trees: &[],
        flower_trees: &[],
        shortest: Some(FeatureKey::Acacia),
        secondary_chance: 0.0,
    };
    pub const CHERRY: Self = Self {
        trees: &[FeatureKey::Cherry],
        mega_trees: &[],
        flower_trees: &[FeatureKey::CherryBees005],
        shortest: Some(FeatureKey::Cherry),
        secondary_chance: 0.0,
    };
    pub const DARK_OAK: Self = Self {
        trees: &[],
        mega_trees: &[FeatureKey::DarkOak],
        flower_trees: &[],
        shortest: None,
        secondary_chance: 0.0,
    };
    pub const PALE_OAK: Self = Self {
        trees: &[],
        mega_trees: &[FeatureKey::PaleOakBonemeal],
        flower_trees: &[],
        shortest: None,
        secondary_chance: 0.0,
    };

    #[must_use]
    pub fn for_block(block: &Block) -> Option<&'static Self> {
        match block.name {
            "oak_sapling" => Some(&Self::OAK),
            "spruce_sapling" => Some(&Self::SPRUCE),
            "birch_sapling" => Some(&Self::BIRCH),
            "jungle_sapling" => Some(&Self::JUNGLE),
            "acacia_sapling" => Some(&Self::ACACIA),
            "dark_oak_sapling" => Some(&Self::DARK_OAK),
            "pale_oak_sapling" => Some(&Self::PALE_OAK),
            "cherry_sapling" => Some(&Self::CHERRY),
            "azalea" | "flowering_azalea" => Some(&Self::AZALEA),
            "mangrove_propagule" => Some(&Self::MANGROVE),
            _ => None,
        }
    }

    fn pick_tree(&self, world: &World, has_flowers: bool) -> Option<FeatureKey> {
        // Vanilla always draws here, including growers with no secondary tree.
        if world.rand_f32() < self.secondary_chance {
            if has_flowers && let Some(key) = self.flower_trees.get(1) {
                return Some(*key);
            }
            if let Some(key) = self.trees.get(1) {
                return Some(*key);
            }
        }
        if has_flowers && let Some(key) = self.flower_trees.first() {
            return Some(*key);
        }
        self.trees.first().copied()
    }

    fn pick_mega(&self, world: &World) -> Option<FeatureKey> {
        if let Some(key) = self.mega_trees.get(1)
            && world.rand_f32() < self.secondary_chance
        {
            return Some(*key);
        }
        self.mega_trees.first().copied()
    }

    fn has_flowers(world: &World, pos: &BlockPos) -> bool {
        for z in -2..=2 {
            for y in -1..=1 {
                for x in -2..=2 {
                    let block = world.get_block(&pos.offset(Vector3::new(x, y, z)));
                    if block.has_tag(&tag::Block::MINECRAFT_FLOWERS) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn surrounding(
        world: &World,
        pos: &BlockPos,
        dx: i32,
        dz: i32,
    ) -> Vec<(BlockPos, BlockStateId)> {
        [(dx, dz), (dx + 1, dz), (dx, dz + 1), (dx + 1, dz + 1)]
            .into_iter()
            .map(|(x, z)| {
                let position = pos.offset(Vector3::new(x, 0, z));
                (position, world.get_block_state_id(&position))
            })
            .collect()
    }

    fn find_two_by_two(world: &World, block: &Block, pos: &BlockPos) -> Option<TwoByTwoSaplingPos> {
        for dx in [0, -1] {
            for dz in [0, -1] {
                let saplings = Self::surrounding(world, pos, dx, dz);
                if saplings
                    .iter()
                    .all(|(_, state_id)| Block::from_state_id(*state_id) == block)
                {
                    return Some(TwoByTwoSaplingPos {
                        offset_x: dx,
                        offset_z: dz,
                        saplings,
                    });
                }
            }
        }
        None
    }

    fn remove_saplings(world: &Arc<World>, saplings: &[(BlockPos, BlockStateId)]) {
        for (pos, _) in saplings {
            world.set_block_state(
                pos,
                Block::AIR.default_state.id,
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
        }
    }

    fn reset_saplings(world: &Arc<World>, saplings: &[(BlockPos, BlockStateId)]) {
        for (pos, state_id) in saplings {
            world.set_block_state(
                pos,
                *state_id,
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
        }
    }

    fn place(world: &Arc<World>, key: FeatureKey, pos: BlockPos) -> bool {
        let Some(ConfiguredFeature::Tree(tree)) = CONFIGURED_FEATURES.get(&key) else {
            return false;
        };
        let portal = world.level.world_portal.load_full();
        let Some(portal) = portal.as_ref() else {
            return false;
        };
        let mut cache = WorldGenerationCache::new(world.clone(), &pos);
        if !cache.generate_with_level_random(|cache, random| {
            tree.generate(&**portal, cache, random, pos)
        }) {
            return false;
        }
        cache.apply();
        true
    }

    #[must_use]
    pub fn min_height(&self) -> i32 {
        self.shortest
            .and_then(|key| match CONFIGURED_FEATURES.get(&key) {
                Some(ConfiguredFeature::Tree(tree)) => {
                    Some(i32::from(tree.trunk_placer.base_height))
                }
                _ => None,
            })
            .unwrap_or(0)
    }

    pub fn grow_tree(
        &self,
        world: &Arc<World>,
        pos: &BlockPos,
        block: &Block,
        state_id: BlockStateId,
    ) -> bool {
        if let Some(mega) = self.pick_mega(world)
            && CONFIGURED_FEATURES.contains_key(&mega)
            && let Some(two_by_two) = Self::find_two_by_two(world, block, pos)
        {
            Self::remove_saplings(world, &two_by_two.saplings);
            let origin = pos.offset(Vector3::new(two_by_two.offset_x, 0, two_by_two.offset_z));
            if Self::place(world, mega, origin) {
                return true;
            }
            // Failed mega growth restores the clicked sapling state in all four cells.
            let restore: Vec<_> = two_by_two
                .saplings
                .iter()
                .map(|(pos, _)| (*pos, state_id))
                .collect();
            Self::reset_saplings(world, &restore);
            return false;
        }

        let Some(key) = self.pick_tree(world, Self::has_flowers(world, pos)) else {
            return false;
        };
        if !CONFIGURED_FEATURES.contains_key(&key) {
            return false;
        }
        let empty = World::fluid_state_from_block_state(state_id)
            .1
            .block_state_id;
        world.set_block_state(pos, empty, BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK);
        if Self::place(world, key, *pos) {
            if world.get_block_state_id(pos) == empty {
                world.queue_block_updates(&[(*pos, empty)]);
            }
            return true;
        }
        Self::reset_saplings(world, &[(*pos, state_id)]);
        false
    }
}
