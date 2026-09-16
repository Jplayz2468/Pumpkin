use crate::entity::EntityBase;
use pumpkin_data::block_properties::{PistonHeadLikeProperties, StickyPistonLikeProperties};
use std::sync::atomic::Ordering;
use std::{cell::Cell, sync::Arc};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::world::{BlockFlags, World};

use super::BlockEntity;

thread_local! {
    static NOCLIP: Cell<Option<BlockDirection>> = const { Cell::new(None) };
}

/// Restore the thread-local collision context even if an entity callback unwinds.
pub(crate) fn with_piston_noclip<T>(direction: BlockDirection, action: impl FnOnce() -> T) -> T {
    struct Reset(Option<BlockDirection>);
    impl Drop for Reset {
        fn drop(&mut self) {
            NOCLIP.set(self.0);
        }
    }
    let _reset = Reset(NOCLIP.replace(Some(direction)));
    action()
}

pub struct PistonBlockEntity {
    pub position: BlockPos,
    pub pushed_block_state: &'static BlockState,
    pub facing: BlockDirection,
    pub current_progress: AtomicCell<f32>,
    pub last_progress: AtomicCell<f32>,
    pub extending: bool,
    pub source: bool,
    /// Vanilla `PistonMovingBlockEntity.lastTicked` (`PistonMovingBlockEntity.java:47`),
    /// set each tick and read by `PistonBaseBlock.checkIfExtend` to decide whether a
    /// retraction drags the block it is pulling.
    pub last_ticked: AtomicCell<i64>,
}

impl PistonBlockEntity {
    pub const ID: &'static str = "minecraft:piston";

    const fn movement_direction(&self) -> BlockDirection {
        if self.extending {
            self.facing
        } else {
            self.facing.opposite()
        }
    }

    /// Vanilla's `getAmountExtended`: how far back from the block's final position
    /// the visual is at a given animation progress. Negative for extending.
    fn amount_extended(&self, progress: f32) -> f32 {
        if self.extending {
            progress - 1.0
        } else {
            1.0 - progress
        }
    }

    fn dir_vec(dir: BlockDirection, scale: f64) -> Vector3<f64> {
        let off = dir.to_offset();
        Vector3::new(
            f64::from(off.x) * scale,
            f64::from(off.y) * scale,
            f64::from(off.z) * scale,
        )
    }

    fn head_state(&self, short: bool) -> &'static BlockState {
        let mut props = PistonHeadLikeProperties::default(&Block::PISTON_HEAD);
        props.facing = self.facing.to_facing();
        props.short = short;
        props.to_state_id(&Block::PISTON_HEAD).to_state()
    }

    /// Collision boxes relative to the moving-piston cell, including its stationary
    /// base during retraction. Only the moving piece is suppressed while it pushes.
    pub(crate) fn collision_boxes(&self) -> Vec<BoundingBox> {
        let progress = self.current_progress.load();
        let mut boxes = Vec::new();
        if !self.extending && self.source {
            let block = self.pushed_block_state.id.to_block();
            if block == &Block::PISTON || block == &Block::STICKY_PISTON {
                let mut props =
                    StickyPistonLikeProperties::from_state_id(self.pushed_block_state.id);
                props.extended = true;
                boxes.extend(
                    props
                        .to_state_id(block)
                        .to_state()
                        .get_block_collision_shapes_at(&self.position),
                );
            }
        }
        if progress < 1.0 && NOCLIP.get() == Some(self.movement_direction()) {
            return boxes;
        }
        let state = if self.source {
            self.head_state(self.extending != (1.0 - progress < 0.25))
        } else {
            self.pushed_block_state
        };
        let shift = Self::dir_vec(self.facing, f64::from(self.amount_extended(progress)));
        boxes.extend(
            state
                .get_block_collision_shapes_at(&self.position)
                .map(|b| b.shift(shift)),
        );
        boxes
    }

    fn ignores_piston(entity: &dyn EntityBase) -> bool {
        entity.get_entity().is_removed()
            || entity
                .cast_any()
                .downcast_ref::<crate::entity::decoration::armor_stand::ArmorStandEntity>()
                .is_some_and(|stand| stand.is_marker())
    }

    /// Swept front face (PistonMath.getMovementArea), excluding the old volume.
    fn movement_area(mut area: BoundingBox, direction: BlockDirection, delta: f64) -> BoundingBox {
        match direction {
            BlockDirection::East => {
                area.min.x = area.max.x;
                area.max.x += delta;
            }
            BlockDirection::West => {
                area.max.x = area.min.x;
                area.min.x -= delta;
            }
            BlockDirection::Up => {
                area.min.y = area.max.y;
                area.max.y += delta;
            }
            BlockDirection::Down => {
                area.max.y = area.min.y;
                area.min.y -= delta;
            }
            BlockDirection::South => {
                area.min.z = area.max.z;
                area.max.z += delta;
            }
            BlockDirection::North => {
                area.max.z = area.min.z;
                area.min.z -= delta;
            }
        }
        area
    }

    fn push_entities(&self, world: &Arc<World>, new_progress: f32) {
        let last = self.current_progress.load();
        let delta = f64::from(new_progress - last);
        if delta <= 0.0 {
            return;
        }
        let direction = self.movement_direction();
        let shift = self.position.to_f64()
            + Self::dir_vec(self.facing, f64::from(self.amount_extended(last)));
        let state = if !self.extending && self.source {
            self.head_state(last > 0.25)
        } else {
            self.pushed_block_state
        };
        let boxes: Vec<_> = state
            .get_block_collision_shapes_at(&self.position)
            .map(|b| b.shift(shift))
            .collect();
        if let Some(first) = boxes.first().copied() {
            let bounds = boxes.iter().skip(1).fold(first, |a, b| {
                BoundingBox::new(
                    Vector3::new(
                        a.min.x.min(b.min.x),
                        a.min.y.min(b.min.y),
                        a.min.z.min(b.min.z),
                    ),
                    Vector3::new(
                        a.max.x.max(b.max.x),
                        a.max.y.max(b.max.y),
                        a.max.z.max(b.max.z),
                    ),
                )
            });
            for entity in world.get_all_at_box(&bounds.stretch(Self::dir_vec(direction, delta))) {
                if Self::ignores_piston(entity.as_ref()) {
                    continue;
                }
                let e = entity.get_entity();
                if self.pushed_block_state.id.to_block() == &Block::SLIME_BLOCK {
                    // Server players receive the block animation and simulate bounce locally.
                    if entity.get_player().is_some() {
                        continue;
                    }
                    let mut velocity = e.velocity.load();
                    let axis = direction.to_axis().into();
                    velocity.set_axis(axis, if direction.positive() { 1.0 } else { -1.0 });
                    e.velocity.store(velocity);
                    e.velocity_dirty.store(true, Ordering::Relaxed);
                }
                let entity_box = e.bounding_box.load();
                let overlap = boxes
                    .iter()
                    .map(|b| Self::movement_area(*b, direction, delta))
                    .filter(|b| b.intersects(&entity_box))
                    .map(|b| Self::intersection_size(b, direction, entity_box))
                    .fold(0.0_f64, f64::max);
                if overlap <= 0.0 {
                    continue;
                }
                Self::move_entity(
                    entity.as_ref(),
                    direction,
                    overlap.min(delta) + 0.01,
                    direction,
                );
                if !self.extending && self.source {
                    Self::push_out_of_piston_body(
                        entity.as_ref(),
                        &self.position,
                        direction,
                        delta,
                    );
                }
            }
        }
        if self.pushed_block_state.id.to_block() == &Block::HONEY_BLOCK
            && direction != BlockDirection::Up
            && direction != BlockDirection::Down
        {
            let top = self
                .pushed_block_state
                .get_block_collision_shapes_at(&self.position)
                .map(|b| b.max.y)
                .fold(0.0_f64, f64::max);
            let area = BoundingBox::new(
                Vector3::new(0.0, top, 0.0),
                Vector3::new(1.0, 1.5000010000000001, 1.0),
            )
            .shift(shift);
            for entity in world.get_all_at_box(&area) {
                let e = entity.get_entity();
                let pos = e.pos.load();
                if !Self::ignores_piston(entity.as_ref())
                    && e.on_ground.load(Ordering::Relaxed)
                    && (e.get_supporting_block_pos() == Some(self.position)
                        || (pos.x >= area.min.x
                            && pos.x <= area.max.x
                            && pos.z >= area.min.z
                            && pos.z <= area.max.z))
                {
                    Self::move_entity(entity.as_ref(), direction, delta, direction);
                }
            }
        }
    }

    /// Vanilla `getIntersectionSize`: how much `entity` overlaps `swept` along
    /// `motion_dir`. Positive means the entity is in the path of the moving block.
    fn intersection_size(
        swept: BoundingBox,
        motion_dir: BlockDirection,
        entity: BoundingBox,
    ) -> f64 {
        match motion_dir {
            BlockDirection::East => swept.max.x - entity.min.x,
            BlockDirection::West => entity.max.x - swept.min.x,
            BlockDirection::Up => swept.max.y - entity.min.y,
            BlockDirection::Down => entity.max.y - swept.min.y,
            BlockDirection::South => swept.max.z - entity.min.z,
            BlockDirection::North => entity.max.z - swept.min.z,
        }
    }

    fn move_entity(
        entity: &dyn EntityBase,
        dir: BlockDirection,
        distance: f64,
        piston_direction: BlockDirection,
    ) {
        with_piston_noclip(piston_direction, || {
            entity
                .get_entity()
                .move_by_piston(entity, Self::dir_vec(dir, distance))
        });
    }

    /// Vanilla `push`: when a piston head retracts, shove entities that ended up
    /// inside the piston-body cube back out the opposite direction (slightly past
    /// the move they just got, so the net motion is essentially zero).
    fn push_out_of_piston_body(
        entity: &dyn EntityBase,
        piston_pos: &BlockPos,
        motion_dir: BlockDirection,
        amount: f64,
    ) {
        let body_aabb = BoundingBox::from_block(piston_pos);
        let entity_aabb = entity.get_entity().bounding_box.load();
        if !body_aabb.intersects(&entity_aabb) {
            return;
        }
        let back = motion_dir.opposite();
        let e = Self::intersection_size(body_aabb, back, entity_aabb) + 0.01;
        let f = Self::intersection_size(
            body_aabb,
            back,
            Self::aabb_intersection(body_aabb, entity_aabb),
        ) + 0.01;
        if (e - f).abs() < 0.01 {
            let distance = e.min(amount) + 0.01;
            Self::move_entity(entity, back, distance, motion_dir);
        }
    }

    const fn aabb_intersection(a: BoundingBox, b: BoundingBox) -> BoundingBox {
        BoundingBox::new(
            Vector3::new(
                a.min.x.max(b.min.x),
                a.min.y.max(b.min.y),
                a.min.z.max(b.min.z),
            ),
            Vector3::new(
                a.max.x.min(b.max.x),
                a.max.y.min(b.max.y),
                a.max.z.min(b.max.z),
            ),
        )
    }

    /// Vanilla `PistonMovingBlockEntity.finalTick` (`PistonMovingBlockEntity.java:278`).
    pub fn finish(&self, world: &Arc<World>) {
        if self.last_progress.load() < 1.0 {
            // Vanilla sets `progress` and `progressO` to 1.0 *before* removing the entity
            // (`PistonMovingBlockEntity.java:280`). The block-entity tick loop iterates a
            // snapshot taken at the start of the tick, so this entity is still ticked after
            // it is removed here; without pinning the progress, that tick would restart the
            // animation and shove entities for a cycle that has already ended.
            self.current_progress.store(1.0);
            self.last_progress.store(1.0);
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let state = if self.source {
                    Block::AIR.default_state.id
                } else {
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos)
                };
                world.set_block_state(&pos, state, BlockFlags::NOTIFY_ALL);
                world.update_neighbor(&pos, state.to_block());
            }
        }
    }
}

/// Serialize a block state as vanilla's `BlockState.CODEC` does -- a compound holding the
/// namespaced `Name` and, when the block has any, a `Properties` compound of string
/// key/value pairs. Mirrors the palette encoding already used for chunk sections.
fn block_state_to_nbt(state: &'static BlockState) -> NbtCompound {
    let block = Block::from_state_id(state.id);
    let mut compound = NbtCompound::new();
    let name = if block.name.starts_with("minecraft:") {
        block.name.to_string()
    } else {
        format!("minecraft:{}", block.name)
    };
    compound.put_string(BLOCK_STATE_NAME, name);

    if let Some(props) = block.properties(state.id) {
        let pairs = props.to_props();
        if !pairs.is_empty() {
            let mut props_compound = NbtCompound::new();
            for (key, value) in pairs {
                props_compound.put_string(key, value.to_string());
            }
            compound.put_compound(BLOCK_STATE_PROPERTIES, props_compound);
        }
    }
    compound
}

/// Inverse of [`block_state_to_nbt`]. Falls back to air when the entry is missing or names
/// a block that no longer exists, which is what vanilla's `orElse(DEFAULT_BLOCK_STATE)`
/// does on a failed decode.
fn block_state_from_nbt(compound: &NbtCompound) -> &'static BlockState {
    let Some(name) = compound.get_string(BLOCK_STATE_NAME) else {
        return Block::AIR.default_state;
    };
    let Some(block) = Block::from_name(name) else {
        return Block::AIR.default_state;
    };

    let Some(props_compound) = compound.get_compound(BLOCK_STATE_PROPERTIES) else {
        return block.default_state;
    };

    let owned: Vec<(String, String)> = props_compound
        .child_tags
        .iter()
        .filter_map(|(key, tag)| {
            tag.extract_string()
                .map(|value| (key.to_string(), value.to_string()))
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = owned
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let state_id = block.from_properties(&borrowed).to_state_id(block);
    BlockState::from_id(state_id)
}

const BLOCK_STATE_NAME: &str = "Name";
const BLOCK_STATE_PROPERTIES: &str = "Properties";

const BLOCK_STATE: &str = "blockState";
const FACING: &str = "facing";
const LAST_PROGRESS: &str = "progress";
const EXTENDING: &str = "extending";
const SOURCE: &str = "source";

impl BlockEntity for PistonBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick(&self, world: &Arc<World>) {
        // Vanilla `PistonMovingBlockEntity.tick` (`PistonMovingBlockEntity.java:311`).
        self.last_ticked.store(world.get_world_age());
        let current_progress = self.current_progress.load();
        self.last_progress.store(current_progress);
        if current_progress >= 1.0 {
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let updated_state =
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos);
                if updated_state.to_state().is_air() {
                    // Restore the moved block before destruction so unsupported
                    // blocks produce their own drops, rather than moving-piston loot.
                    world.set_block_state(
                        &pos,
                        self.pushed_block_state.id,
                        BlockFlags::MOVED
                            | BlockFlags::SKIP_SHAPE_UPDATES
                            | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
                    );
                    world.break_block(&pos, None, BlockFlags::NOTIFY_ALL);
                } else {
                    let updated_state = updated_state
                        .to_block()
                        .set_waterlogged(updated_state, false)
                        .unwrap_or(updated_state);
                    world.set_block_state(
                        &pos,
                        updated_state,
                        BlockFlags::NOTIFY_ALL | BlockFlags::MOVED,
                    );
                    world.update_neighbor(&pos, updated_state.to_block());
                }
            }
            return;
        }
        let new_progress = (current_progress + 0.5).min(1.0);
        self.push_entities(world, new_progress);
        self.current_progress.store(new_progress);
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        // Vanilla `PistonMovingBlockEntity.loadAdditional` (`:348`) reads "blockState"
        // through BlockState.CODEC, defaulting to air. Without this a world saved mid-push
        // lost the block being moved.
        let pushed_block_state = nbt
            .get_compound(BLOCK_STATE)
            .map_or_else(|| Block::AIR.default_state, block_state_from_nbt);
        let facing = nbt.get_byte(FACING).unwrap_or(0);
        let last_progress = nbt.get_float(LAST_PROGRESS).unwrap_or(0.0);
        let extending = nbt.get_bool(EXTENDING).unwrap_or(false);
        let source = nbt.get_bool(SOURCE).unwrap_or(false);
        Self {
            pushed_block_state,
            position,
            facing: BlockDirection::from_index(facing as u8).unwrap_or(BlockDirection::Down),
            current_progress: last_progress.into(),
            last_progress: last_progress.into(),
            extending,
            source,
            // Not persisted in vanilla either; it is re-established on the next tick.
            last_ticked: AtomicCell::new(0),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_compound(BLOCK_STATE, block_state_to_nbt(self.pushed_block_state));
        nbt.put_byte(FACING, self.facing.to_index() as i8);
        nbt.put_float(LAST_PROGRESS, self.last_progress.load());
        nbt.put_bool(EXTENDING, self.extending);
        nbt.put_bool(SOURCE, self.source);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_compound(BLOCK_STATE, block_state_to_nbt(self.pushed_block_state));
        nbt.put_byte(FACING, self.facing.to_index() as i8);
        nbt.put_float(LAST_PROGRESS, self.last_progress.load());
        nbt.put_bool(EXTENDING, self.extending);
        nbt.put_bool(SOURCE, self.source);
        // TODO: duplicated code because of async :c
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod block_state_nbt_tests {
    use super::*;

    #[test]
    fn moving_slab_preserves_empty_space_and_noclip_is_scoped() {
        let slab = Block::STONE_SLAB
            .from_properties(&[("type", "bottom"), ("waterlogged", "false")])
            .to_state_id(&Block::STONE_SLAB)
            .to_state();
        let piston = PistonBlockEntity {
            position: BlockPos::new(0, 0, 0),
            pushed_block_state: slab,
            facing: BlockDirection::East,
            current_progress: 0.5.into(),
            last_progress: 0.0.into(),
            extending: true,
            source: false,
            last_ticked: 0.into(),
        };
        let boxes = piston.collision_boxes();
        assert!(!boxes.is_empty());
        assert!(
            boxes
                .iter()
                .all(|b| b.max.y == 0.5 && b.min.x == -0.5 && b.max.x == 0.5)
        );
        with_piston_noclip(BlockDirection::East, || {
            assert!(piston.collision_boxes().is_empty());
            with_piston_noclip(BlockDirection::West, || {
                assert!(!piston.collision_boxes().is_empty())
            });
            assert!(piston.collision_boxes().is_empty());
        });
        assert!(!piston.collision_boxes().is_empty());
    }

    #[test]
    fn swept_front_excludes_entities_behind_the_moving_face() {
        let body = BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]);
        let swept = PistonBlockEntity::movement_area(body, BlockDirection::East, 0.5);
        assert!(!swept.intersects(&BoundingBox::new_array([0.2, 0.1, 0.2], [0.7, 0.4, 0.7])));
        assert!(swept.intersects(&BoundingBox::new_array([1.2, 0.1, 0.2], [1.7, 0.4, 0.7])));
        assert!(!swept.intersects(&BoundingBox::new_array([1.2, 0.6, 0.2], [1.7, 0.9, 0.7])));
    }

    /// A piston saved mid-push must restore the exact state it was moving. Vanilla stores
    /// it under "blockState" via `BlockState.CODEC` (`PistonMovingBlockEntity.java:359`);
    /// this previously wrote nothing and loaded air, silently deleting the block.
    #[test]
    fn round_trips_states_with_and_without_properties() {
        // No properties -- Name only, no Properties compound.
        let plain = Block::STONE.default_state;
        let encoded = block_state_to_nbt(plain);
        assert_eq!(
            encoded.get_string(BLOCK_STATE_NAME),
            Some("minecraft:stone")
        );
        assert!(encoded.get_compound(BLOCK_STATE_PROPERTIES).is_none());
        assert_eq!(block_state_from_nbt(&encoded).id, plain.id);

        // With properties -- a non-default state must survive, not collapse to default.
        let repeater = &Block::REPEATER;
        let non_default = BlockState::from_id(
            repeater
                .from_properties(&[
                    ("delay", "3"),
                    ("facing", "west"),
                    ("locked", "true"),
                    ("powered", "true"),
                ])
                .to_state_id(repeater),
        );
        assert_ne!(
            non_default.id, repeater.default_state.id,
            "test needs a genuinely non-default state"
        );
        let encoded = block_state_to_nbt(non_default);
        assert!(encoded.get_compound(BLOCK_STATE_PROPERTIES).is_some());
        assert_eq!(
            block_state_from_nbt(&encoded).id,
            non_default.id,
            "a state carrying properties must round-trip exactly"
        );
    }

    /// Vanilla's decode is `orElse(DEFAULT_BLOCK_STATE)`, so malformed or unknown entries
    /// fall back to air rather than failing the chunk load.
    #[test]
    fn unknown_or_missing_entries_fall_back_to_air() {
        assert_eq!(
            block_state_from_nbt(&NbtCompound::new()).id,
            Block::AIR.default_state.id
        );

        let mut bogus = NbtCompound::new();
        bogus.put_string(BLOCK_STATE_NAME, "minecraft:not_a_real_block".to_string());
        assert_eq!(block_state_from_nbt(&bogus).id, Block::AIR.default_state.id);
    }
}
