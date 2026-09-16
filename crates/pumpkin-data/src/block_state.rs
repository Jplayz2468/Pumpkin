#[path = "generated/collision_step_coords.rs"]
mod collision_step_coords;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::block_properties::{COLLISION_SHAPES, NoteblockInstrument};
use crate::{Block, BlockDirection, BlockId};

/// Represents a specific state of a block, including its properties and physical behaviors.
///
/// A single `Block` (like a Hopper) can have multiple `BlockState`s (e.g., pointing North,
/// South, or being powered). This struct is optimized for high-speed lookups during
/// physics and lighting calculations.
#[derive(Debug)]
pub struct BlockState {
    /// The global palette ID used for network serialization and chunk storage.
    pub id: BlockStateId,
    /// Bit-flags representing boolean or enum properties (e.g., `waterlogged`, `lit`, `facing`).
    pub state_flags: u16,
    /// Cached flags for each of the 6 sides to speed up ambient occlusion and face culling.
    pub side_flags: u8,
    /// The note block instrument produced when this block is placed underneath one.
    pub instrument: NoteblockInstrument,
    /// The light level emitted by this block, ranging from 0 to 15.
    pub luminance: u8,
    /// Defines how the block reacts to being pushed or pulled by a piston.
    pub piston_behavior: PistonBehavior,
    /// Overrides the base block hardness for this specific state if necessary.
    pub hardness: f32,
    /// Indices into a global voxel-shape registry for physical entity collisions.
    pub collision_shapes: &'static [u16],
    /// Indices into a global voxel-shape registry for the selection highlight box.
    pub outline_shapes: &'static [u16],
    /// How much light is subtracted as it passes through this block (0 for transparent, 15 for opaque).
    pub opacity: u8,
    /// The ID of the block entity associated with this state.
    /// Set to `u16::MAX` if the block does not hold NBT data.
    pub block_entity_type: u16,
}

/// Helper struct to ensure the validity of `BlockStateIds` parsed from external sources.
/// Every [`BlockStateId`] is guaranteed to correspond to a valid [`BlockState`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BlockStateId(u16);

#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PistonBehavior {
    Normal,
    Destroy,
    Block,
    Ignore,
    PushOnly,
}

impl PartialEq<BlockStateId> for BlockState {
    fn eq(&self, other: &BlockStateId) -> bool {
        self.id == *other
    }
}

impl PartialEq<BlockState> for BlockStateId {
    fn eq(&self, other: &BlockState) -> bool {
        *self == other.id
    }
}

impl PartialEq for BlockState {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for BlockState {}

impl BlockState {
    #[must_use]
    pub const fn is_air(&self) -> bool {
        self.state_flags & IS_AIR != 0
    }

    #[must_use]
    pub const fn burnable(&self) -> bool {
        self.state_flags & BURNABLE != 0
    }

    #[must_use]
    pub const fn tool_required(&self) -> bool {
        self.state_flags & TOOL_REQUIRED != 0
    }

    #[must_use]
    pub const fn sided_transparency(&self) -> bool {
        self.state_flags & SIDED_TRANSPARENCY != 0
    }

    #[must_use]
    pub const fn replaceable(&self) -> bool {
        self.state_flags & REPLACEABLE != 0
    }

    #[must_use]
    pub const fn is_liquid(&self) -> bool {
        self.state_flags & IS_LIQUID != 0
    }

    /// Returns the legacy value for whether a block is solid.
    #[must_use]
    pub const fn is_solid(&self) -> bool {
        self.state_flags & IS_SOLID != 0
    }

    #[must_use]
    pub const fn is_full_cube(&self) -> bool {
        self.state_flags & IS_FULL_CUBE != 0
    }

    /// Returns whether the block is solid.
    /// Solid blocks conduct redstone and block redstone wire.
    /// Non-solid blocks don't allow redstone wire on top to propagate their signal downwards in java.
    #[must_use]
    pub const fn is_solid_block(&self) -> bool {
        self.state_flags & IS_SOLID_BLOCK != 0
    }

    #[must_use]
    pub const fn has_random_ticks(&self) -> bool {
        self.state_flags & HAS_RANDOM_TICKS != 0
    }

    /// Returns whether this block state is a full opaque cube (`isSolidRender()` in Java).
    #[must_use]
    pub const fn is_solid_render(&self) -> bool {
        self.state_flags & IS_SOLID_RENDER != 0
    }

    /// Returns whether this block state can occlude other blocks (`canOcclude` in Java).
    #[must_use]
    pub const fn can_occlude(&self) -> bool {
        self.state_flags & CAN_OCCLUDE != 0
    }

    /// Returns whether this block produces an analog output signal for comparators (`hasAnalogOutputSignal()` in Java).
    #[must_use]
    pub const fn has_analog_output_signal(&self) -> bool {
        self.state_flags & HAS_ANALOG_OUTPUT_SIGNAL != 0
    }

    ///`isFaceSturdy()` in Java!
    #[must_use]
    pub const fn is_side_solid(&self, side: BlockDirection) -> bool {
        match side {
            BlockDirection::Down => self.side_flags & DOWN_SIDE_SOLID != 0,
            BlockDirection::Up => self.side_flags & UP_SIDE_SOLID != 0,
            BlockDirection::North => self.side_flags & NORTH_SIDE_SOLID != 0,
            BlockDirection::South => self.side_flags & SOUTH_SIDE_SOLID != 0,
            BlockDirection::West => self.side_flags & WEST_SIDE_SOLID != 0,
            BlockDirection::East => self.side_flags & EAST_SIDE_SOLID != 0,
        }
    }

    ///isSideSolid(..., Direction.UP, SideShapeType.CENTER) in Java!
    ///Only valid for UP and DOWN sides
    #[must_use]
    pub const fn is_center_solid(&self, side: BlockDirection) -> bool {
        match side {
            BlockDirection::Down => self.side_flags & DOWN_CENTER_SOLID != 0,
            BlockDirection::Up => self.side_flags & UP_CENTER_SOLID != 0,
            _ => false,
        }
    }

    #[must_use]
    pub fn is_waterlogged(&self) -> bool {
        self.id.is_waterlogged()
    }

    /// Produce a new state identical to `self` except the waterlogged property
    /// is set to `value`. If the block type does not support waterlogging or
    /// the state already had waterlogged set to `value`, `None` is returned.
    #[must_use]
    pub fn set_waterlogged(&self, value: bool) -> Option<&'static BlockState> {
        self.id
            .to_block()
            .set_waterlogged(self.id, value)
            .map(BlockStateId::to_state)
    }

    /// Full vanilla voxel grid, including internal coordinates that affect step-up choice.
    pub fn collision_coordinates(&self) -> [&'static [f64]; 3] {
        collision_step_coords::GRIDS[collision_step_coords::STATE_GRID[self.id.0 as usize] as usize]
    }

    pub fn collision_coordinates_at(&self, pos: &BlockPos) -> [Vec<f64>; 3] {
        let offset = Block::from_state_id(self.id).shape_offset_delta(pos);
        let offset = [offset.x, offset.y, offset.z];
        let coordinates = self.collision_coordinates();
        std::array::from_fn(|axis| {
            coordinates[axis]
                .iter()
                .map(|value| value + offset[axis])
                .collect()
        })
    }

    pub fn get_block_collision_shapes(&self) -> impl Iterator<Item = BoundingBox> + '_ {
        self.collision_shapes
            .iter()
            .map(|&id| COLLISION_SHAPES[id as usize])
    }

    /// Returns block-local collision shapes with vanilla's coordinate-derived offset applied.
    pub fn get_block_collision_shapes_at(
        &self,
        pos: &BlockPos,
    ) -> impl Iterator<Item = BoundingBox> + '_ {
        let offset = Block::from_state_id(self.id).shape_offset_delta(pos);
        self.get_block_collision_shapes()
            .map(move |shape| shape.shift(offset))
    }

    /// Whether the union of a collision face covers the given rectangle, in local
    /// coordinates. Region axes are X/Z on horizontal faces, X/Y on Z, and Y/Z on X.
    /// This implements !Shapes.joinIsNotEmpty(test, faceShape, ONLY_FIRST).
    pub fn collision_face_covers(
        &self,
        pos: BlockPos,
        side: BlockDirection,
        region: [f64; 4],
    ) -> bool {
        // Check the union, not just individual boxes (stairs can tile a full face).
        let rectangles: Vec<_> = self
            .get_block_collision_shapes_at(&pos)
            .filter_map(|shape| {
                let (min, max, a, b, c, d) = match side {
                    BlockDirection::Down | BlockDirection::Up => (
                        shape.min.y,
                        shape.max.y,
                        shape.min.x,
                        shape.max.x,
                        shape.min.z,
                        shape.max.z,
                    ),
                    BlockDirection::North | BlockDirection::South => (
                        shape.min.z,
                        shape.max.z,
                        shape.min.x,
                        shape.max.x,
                        shape.min.y,
                        shape.max.y,
                    ),
                    BlockDirection::West | BlockDirection::East => (
                        shape.min.x,
                        shape.max.x,
                        shape.min.y,
                        shape.max.y,
                        shape.min.z,
                        shape.max.z,
                    ),
                };
                let boundary = if side.positive() { 1.0 } else { 0.0 };
                (min <= boundary + 1.0e-7
                    && max >= boundary - 1.0e-7
                    && b > region[0]
                    && a < region[1]
                    && d > region[2]
                    && c < region[3])
                    .then_some((
                        a.max(region[0]),
                        b.min(region[1]),
                        c.max(region[2]),
                        d.min(region[3]),
                    ))
            })
            .collect();
        let mut edges = vec![region[0], region[1]];
        for &(a, b, _, _) in &rectangles {
            edges.extend([a, b]);
        }
        edges.sort_by(f64::total_cmp);
        edges.dedup();
        edges.windows(2).all(|interval| {
            let mut strips: Vec<_> = rectangles
                .iter()
                .filter(|&&(a, b, _, _)| a <= interval[0] && b >= interval[1])
                .map(|&(_, _, c, d)| (c, d))
                .collect();
            strips.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut end = region[2];
            for (start, stop) in strips {
                if start > end {
                    return false;
                }
                end = end.max(stop);
            }
            end >= region[3]
        })
    }

    pub fn get_block_outline_shapes(&self) -> impl Iterator<Item = BoundingBox> + '_ {
        let base_shapes = self
            .outline_shapes
            .iter()
            .map(|&id| COLLISION_SHAPES[id as usize]);

        let water_shape = self
            .is_waterlogged()
            .then(|| BoundingBox::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.875, 1.0)));

        base_shapes.chain(water_shape)
    }

    /// Returns block-local outline shapes with vanilla's coordinate-derived offset applied.
    pub fn get_block_outline_shapes_at(
        &self,
        pos: &BlockPos,
    ) -> impl Iterator<Item = BoundingBox> + '_ {
        let offset = Block::from_state_id(self.id).shape_offset_delta(pos);
        let base_shapes = self
            .outline_shapes
            .iter()
            .map(move |&id| COLLISION_SHAPES[id as usize].shift(offset));

        let water_shape = self
            .is_waterlogged()
            .then(|| BoundingBox::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.875, 1.0)));

        base_shapes.chain(water_shape)
    }

    #[must_use]
    pub fn rotate(&self, rotation: crate::block_rotation::Rotation) -> &'static Self {
        Block::from_state_id(self.id).rotate(self.id, rotation)
    }

    #[must_use]
    pub fn mirror(&self, mirror: crate::block_rotation::Mirror) -> &'static Self {
        Block::from_state_id(self.id).mirror(self.id, mirror)
    }
}

impl BlockStateId {
    // depends on generated impl:
    // pub(crate) const STATE_COUNT: u16;

    /// The total count of all registered block states.
    pub const COUNT: u16 = Self::STATE_COUNT;

    // SAFETY: There must never be a BlockStateId where self.0 >= BlockStateId::STATE_COUNT

    #[inline]
    #[must_use]
    pub const fn new(inner: u16) -> Option<Self> {
        if inner < Self::STATE_COUNT {
            return Some(Self(inner));
        }
        None
    }

    #[inline]
    #[must_use]
    pub const fn new_or_air(inner: u16) -> Self {
        if inner < Self::STATE_COUNT {
            return Self(inner);
        }
        Self::AIR
    }

    #[inline(always)]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn to_state(self) -> &'static BlockState {
        BlockState::from_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn to_block_id(self) -> BlockId {
        BlockId::from_state_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn to_block(self) -> &'static Block {
        Block::from_state_id(self)
    }

    #[inline]
    #[must_use]
    pub const fn is_solid_render(self) -> bool {
        self.to_state().is_solid_render()
    }

    #[inline]
    #[must_use]
    pub const fn can_occlude(self) -> bool {
        self.to_state().can_occlude()
    }

    #[inline]
    #[must_use]
    pub const fn has_analog_output_signal(self) -> bool {
        self.to_state().has_analog_output_signal()
    }

    #[inline]
    #[must_use]
    pub fn is_waterlogged(self) -> bool {
        self.to_block().is_waterlogged(self)
    }

    #[inline]
    #[must_use]
    pub fn rotate(self, rotation: crate::block_rotation::Rotation) -> &'static BlockState {
        Block::from_state_id(self).rotate(self, rotation)
    }

    #[inline]
    #[must_use]
    pub fn mirror(self, mirror: crate::block_rotation::Mirror) -> &'static BlockState {
        Block::from_state_id(self).mirror(self, mirror)
    }
}

impl Default for BlockStateId {
    #[inline]
    fn default() -> Self {
        Self::AIR
    }
}

impl std::fmt::Display for BlockStateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BlockStateId({} = \"{}\")",
            self.0,
            Block::from_state_id(*self).name
        )
    }
}

//This is the Layout of state_props in the right order
// state_flags
const IS_AIR: u16 = 1 << 0;
const BURNABLE: u16 = 1 << 1;
const TOOL_REQUIRED: u16 = 1 << 2;
const SIDED_TRANSPARENCY: u16 = 1 << 3;
const REPLACEABLE: u16 = 1 << 4;
const IS_LIQUID: u16 = 1 << 5;
const IS_SOLID: u16 = 1 << 6;
const IS_FULL_CUBE: u16 = 1 << 7;
const IS_SOLID_BLOCK: u16 = 1 << 8;
const HAS_RANDOM_TICKS: u16 = 1 << 9;
const IS_SOLID_RENDER: u16 = 1 << 10;
const CAN_OCCLUDE: u16 = 1 << 11;
const HAS_ANALOG_OUTPUT_SIGNAL: u16 = 1 << 12;

// side_flags
const DOWN_SIDE_SOLID: u8 = 1 << 0;
const UP_SIDE_SOLID: u8 = 1 << 1;
const NORTH_SIDE_SOLID: u8 = 1 << 2;
const SOUTH_SIDE_SOLID: u8 = 1 << 3;
const WEST_SIDE_SOLID: u8 = 1 << 4;
const EAST_SIDE_SOLID: u8 = 1 << 5;
const DOWN_CENTER_SOLID: u8 = 1 << 6;
const UP_CENTER_SOLID: u8 = 1 << 7;

#[cfg(test)]
mod tests {
    use crate::{Block, BlockStateId, block_state_remap::remap_block_state_for_version};
    use pumpkin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
    }

    #[test]
    fn exported_voxel_grids_cover_every_collision_box() {
        assert_eq!(
            super::collision_step_coords::STATE_GRID.len(),
            usize::from(BlockStateId::COUNT)
        );
        for id in 0..BlockStateId::COUNT {
            let state = BlockStateId::new(id).unwrap().to_state();
            let coordinates = state.collision_coordinates();
            for shape in state.get_block_collision_shapes() {
                for (axis, (min, max)) in [
                    (shape.min.x, shape.max.x),
                    (shape.min.y, shape.max.y),
                    (shape.min.z, shape.max.z),
                ]
                .into_iter()
                .enumerate()
                {
                    assert!(
                        coordinates[axis]
                            .iter()
                            .any(|value| (value - min).abs() < 1.0e-7)
                            && coordinates[axis]
                                .iter()
                                .any(|value| (value - max).abs() < 1.0e-7),
                        "state {id}, axis {axis}, box {min}..{max}, grid {:?}",
                        coordinates[axis]
                    );
                }
            }
        }
    }

    #[test]
    fn bamboo_collision_shape_uses_its_world_position() {
        let state = Block::BAMBOO.default_state;
        let origin_shape = state
            .get_block_collision_shapes_at(&BlockPos::new(0, 64, 0))
            .next()
            .unwrap();
        let shifted_shape = state
            .get_block_collision_shapes_at(&BlockPos::new(-18, 64, -7))
            .next()
            .unwrap();

        assert_close(origin_shape.min.x, 0.15625);
        assert_close(origin_shape.max.x, 0.34375);
        assert_close(shifted_shape.min.x, 0.65625);
        assert_close(shifted_shape.max.x, 0.84375);
        assert_close(shifted_shape.min.z, 0.65625);
        assert_close(shifted_shape.max.z, 0.84375);
    }

    #[test]
    fn supported_client_versions_keep_offset_collisions_mapped() {
        let versions = [
            JavaMinecraftVersion::V_1_20_5,
            JavaMinecraftVersion::V_1_21,
            JavaMinecraftVersion::V_1_21_2,
            JavaMinecraftVersion::V_1_21_4,
            JavaMinecraftVersion::V_1_21_5,
            JavaMinecraftVersion::V_1_21_6,
            JavaMinecraftVersion::V_1_21_7,
            JavaMinecraftVersion::V_1_21_9,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_1,
            JavaMinecraftVersion::V_26_2,
        ];

        for version in versions {
            for block in [Block::BAMBOO, Block::POINTED_DRIPSTONE] {
                assert_ne!(
                    remap_block_state_for_version(block.default_state.id.as_u16(), version),
                    BlockStateId::AIR.as_u16(),
                    "{} mapped to air for {version}",
                    block.name
                );
            }
        }
    }
}

#[cfg(test)]
mod redstone_conductor_parity {
    use super::*;
    use crate::Block;

    /// `is_solid_block` is what the redstone wire code uses where vanilla calls
    /// `BlockState.isRedstoneConductor` -- the predicate deciding whether dust may run
    /// up or down over a neighbour. Vanilla defaults it to `isCollisionShapeFullBlock`
    /// and then overrides it on 26 blocks (`Blocks.java`), which is where the
    /// counter-intuitive cases live: glass and glowstone look like full cubes but do not
    /// conduct, while soul sand and mud conduct despite not being full cubes.
    ///
    /// Pinning the ones redstone builders actually rely on, so a data regeneration that
    /// changed this flag's meaning would fail here rather than silently letting dust
    /// climb glass.
    #[test]
    fn matches_vanilla_is_redstone_conductor() {
        // `.isRedstoneConductor(Blocks::never)` in vanilla despite being full cubes.
        for (name, block) in [
            ("glass", &Block::GLASS),
            ("tinted_glass", &Block::TINTED_GLASS),
            ("glowstone", &Block::GLOWSTONE),
            ("sea_lantern", &Block::SEA_LANTERN),
            ("ice", &Block::ICE),
            ("frosted_ice", &Block::FROSTED_ICE),
            ("redstone_block", &Block::REDSTONE_BLOCK),
            ("observer", &Block::OBSERVER),
            ("tnt", &Block::TNT),
            ("beacon", &Block::BEACON),
        ] {
            assert!(
                !BlockState::from_id(block.default_state.id).is_solid_block(),
                "{name} must not conduct redstone"
            );
        }

        // `.isRedstoneConductor(Blocks::always)` in vanilla despite not being full cubes.
        for (name, block) in [("soul_sand", &Block::SOUL_SAND), ("mud", &Block::MUD)] {
            assert!(
                BlockState::from_id(block.default_state.id).is_solid_block(),
                "{name} must conduct redstone"
            );
        }

        // Ordinary full cube, for contrast.
        assert!(BlockState::from_id(Block::STONE.default_state.id).is_solid_block());
    }
}
