use std::sync::Arc;

use pumpkin_data::{
    Block, BlockState, BlockStateId,
    biome::Biome,
    fluid::{Fluid, FluidState},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::{
    HeightMap,
    math::{position::BlockPos, vector2::Vector2, vector3::Vector3},
};
use pumpkin_world::{
    ProtoChunk,
    chunk::ChunkHeightmapType,
    generation::{
        blender::blending_data::BlendingData, height_limit::HeightLimitView,
        proto_chunk::GenerationCache,
    },
    world::{BlockAccessor, BlockFlags},
};
use rustc_hash::FxHashMap;

use crate::block::entities::block_entity_from_nbt;
use crate::world::World;

enum PendingBlockChange {
    Set(BlockPos, BlockStateId, BlockFlags),
    Destroy(BlockPos),
    Schedule(BlockPos, &'static Block, u32),
}

pub struct WorldGenerationCache {
    world: Arc<World>,
    center: ProtoChunk,
    pending: Vec<PendingBlockChange>,
    overlay: FxHashMap<BlockPos, BlockStateId>,
    block_entities: Vec<NbtCompound>,
}

impl WorldGenerationCache {
    #[must_use]
    pub fn new(world: Arc<World>, pos: &BlockPos) -> Self {
        let generator = world.level.world_gen.load();
        let center = ProtoChunk::new(pos.0.x >> 4, pos.0.z >> 4, &generator);
        Self {
            world,
            center,
            pending: Vec::new(),
            overlay: FxHashMap::default(),
            block_entities: Vec::new(),
        }
    }

    pub fn place_configured_feature(
        world: &Arc<World>,
        key: pumpkin_data::configured_feature::ConfiguredFeature,
        placement: pumpkin_data::placed_feature::PlacedFeature,
        pos: BlockPos,
    ) -> bool {
        let Some(feature) =
            pumpkin_world::generation::feature::configured_features::CONFIGURED_FEATURES.get(&key)
        else {
            return false;
        };
        let portal = world.level.world_portal.load_full();
        let Some(portal) = portal.as_ref() else {
            return false;
        };
        let mut cache = Self::new(world.clone(), &pos);
        let result = cache.generate_with_level_random(|cache, random| {
            feature.generate(
                cache,
                &**portal,
                world.dimension.min_y as i8,
                world.dimension.height as u16,
                placement,
                random,
                pos,
            )
        });
        cache.apply();
        result
    }

    pub fn place_placed_feature(
        world: &Arc<World>,
        key: pumpkin_data::placed_feature::PlacedFeature,
        pos: BlockPos,
    ) -> bool {
        let Some(feature) =
            pumpkin_world::generation::feature::placed_features::PLACED_FEATURES.get(&key)
        else {
            return false;
        };
        let portal = world.level.world_portal.load_full();
        let Some(portal) = portal.as_ref() else {
            return false;
        };
        let mut cache = Self::new(world.clone(), &pos);
        let result = cache.generate_with_level_random(|cache, random| {
            feature.generate(
                cache,
                &**portal,
                world.dimension.min_y as i8,
                world.dimension.height as u16,
                key,
                random,
                pos,
            )
        });
        cache.apply();
        result
    }

    fn read(&self, pos: &BlockPos) -> BlockStateId {
        self.overlay
            .get(pos)
            .copied()
            .unwrap_or_else(|| self.world.get_block_state_id(pos))
    }

    fn heightmap(&self, heightmap: ChunkHeightmapType, x: i32, z: i32) -> i32 {
        let min_y = self.world.dimension.min_y;
        self.world
            .level
            .loaded_chunks
            .get(&Vector2::new(x >> 4, z >> 4))
            .map_or(min_y, |chunk| {
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(heightmap, x & 15, z & 15, min_y)
                    + 1
            })
    }

    /// Run the cached feature against the level's existing Java random stream.
    /// This closure may only write to the cache; apply it after this method returns
    /// so neighbor callbacks can draw from the same random source without deadlock.
    pub fn generate_with_level_random<T>(
        &mut self,
        generate: impl FnOnce(&mut Self, &mut pumpkin_util::random::RandomGenerator) -> T,
    ) -> T {
        use pumpkin_util::random::RandomGenerator;
        let world = self.world.clone();
        let mut source = world
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut random = RandomGenerator::Legacy(source.clone());
        let result = generate(self, &mut random);
        let RandomGenerator::Legacy(updated) = random else {
            unreachable!("a configured feature must preserve its random source");
        };
        *source = updated;
        result
    }

    pub fn apply(self) {
        for change in self.pending {
            match change {
                PendingBlockChange::Set(pos, state_id, flags) => {
                    self.world.set_block_state(&pos, state_id, flags);
                }
                PendingBlockChange::Destroy(pos) => {
                    self.world.break_block(&pos, None, BlockFlags::NOTIFY_ALL);
                }
                PendingBlockChange::Schedule(pos, block, delay) => {
                    self.world.schedule_block_tick(
                        block,
                        pos,
                        delay,
                        pumpkin_world::tick::TickPriority::Normal,
                    );
                }
            }
        }
        for nbt in self.block_entities {
            if let Some(block_entity) = block_entity_from_nbt(&nbt) {
                self.world.add_block_entity(block_entity);
            }
        }
    }
}

impl HeightLimitView for WorldGenerationCache {
    fn height(&self) -> u16 {
        self.world.dimension.height as u16
    }

    fn bottom_y(&self) -> i8 {
        self.world.dimension.min_y as i8
    }
}

impl BlockAccessor for WorldGenerationCache {
    fn get_block(&self, position: &BlockPos) -> &'static Block {
        Block::from_state_id(self.read(position))
    }

    fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
        BlockState::from_id(self.read(position))
    }

    fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
        self.read(position)
    }

    fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState) {
        BlockState::from_id_with_block(self.read(position))
    }
}

impl GenerationCache for WorldGenerationCache {
    fn get_center_chunk_mut(&mut self) -> &mut ProtoChunk {
        &mut self.center
    }

    fn get_center_chunk(&self) -> &ProtoChunk {
        &self.center
    }

    fn get_chunk_mut(&mut self, _chunk_x: i32, _chunk_z: i32) -> Option<&mut ProtoChunk> {
        None
    }

    fn get_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> Option<&ProtoChunk> {
        None
    }

    fn try_get_proto_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> Option<&ProtoChunk> {
        None
    }

    fn get_block_state(&self, pos: &Vector3<i32>) -> BlockStateId {
        self.read(&BlockPos(*pos))
    }

    fn get_fluid_and_fluid_state(&self, position: &Vector3<i32>) -> (Fluid, FluidState) {
        let (fluid, state) = World::fluid_state_from_block_state(self.read(&BlockPos(*position)));
        (fluid.clone(), state)
    }

    fn set_block_state(&mut self, pos: &Vector3<i32>, block_state: &BlockState) {
        self.set_block_state_with_flags(pos, block_state, BlockFlags::NOTIFY_ALL);
    }

    fn set_block_state_with_flags(
        &mut self,
        pos: &Vector3<i32>,
        state: &BlockState,
        flags: BlockFlags,
    ) {
        let position = BlockPos(*pos);
        if !self.world.is_in_height_limit(pos.y) {
            return;
        }
        self.overlay.insert(position, state.id);
        self.pending
            .push(PendingBlockChange::Set(position, state.id, flags));
    }

    fn destroy_block(&mut self, pos: &Vector3<i32>) {
        let position = BlockPos(*pos);
        let state = self.read(&position);
        if state.to_state().is_air() {
            return;
        }
        let (_, fluid) = World::fluid_state_from_block_state(state);
        self.overlay.insert(position, fluid.block_state_id);
        self.pending.push(PendingBlockChange::Destroy(position));
    }

    fn schedule_block_tick(&mut self, pos: BlockPos, block: &'static Block, delay: u32) {
        self.pending
            .push(PendingBlockChange::Schedule(pos, block, delay));
    }

    fn add_block_entity(&mut self, pos: &Vector3<i32>, mut nbt: NbtCompound) {
        nbt.put_int("x", pos.x);
        nbt.put_int("y", pos.y);
        nbt.put_int("z", pos.z);
        self.block_entities.push(nbt);
    }

    fn top_motion_blocking_block_height_exclusive(&self, x: i32, z: i32) -> i32 {
        self.heightmap(ChunkHeightmapType::MotionBlocking, x, z)
    }

    fn top_motion_blocking_block_no_leaves_height_exclusive(&self, x: i32, z: i32) -> i32 {
        self.heightmap(ChunkHeightmapType::MotionBlockingNoLeaves, x, z)
    }

    fn get_top_y(&self, heightmap: &HeightMap, x: i32, z: i32) -> i32 {
        match heightmap {
            HeightMap::WorldSurfaceWg
            | HeightMap::WorldSurface
            | HeightMap::OceanFloorWg
            | HeightMap::OceanFloor => self.top_block_height_exclusive(x, z),
            HeightMap::MotionBlocking => self.top_motion_blocking_block_height_exclusive(x, z),
            HeightMap::MotionBlockingNoLeaves => {
                self.top_motion_blocking_block_no_leaves_height_exclusive(x, z)
            }
        }
    }

    fn top_block_height_exclusive(&self, x: i32, z: i32) -> i32 {
        self.heightmap(ChunkHeightmapType::WorldSurface, x, z)
    }

    fn ocean_floor_height_exclusive(&self, x: i32, z: i32) -> i32 {
        self.heightmap(ChunkHeightmapType::WorldSurface, x, z)
    }

    fn is_air(&self, local_pos: &Vector3<i32>) -> bool {
        BlockState::from_id(self.read(&BlockPos(*local_pos))).is_air()
    }

    fn get_biome_for_terrain_gen(&self, x: i32, y: i32, z: i32) -> &'static Biome {
        self.world.get_biome(&BlockPos::new(x, y, z))
    }

    fn get_blending_data(&self, _chunk_x: i32, _chunk_z: i32) -> Option<&BlendingData> {
        None
    }
}
