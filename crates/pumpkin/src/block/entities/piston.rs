use std::sync::Arc;
use std::sync::atomic::Ordering;

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::world::{BlockFlags, World};

use super::BlockEntity;

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

    /// Ports vanilla `PistonBlockEntity.pushEntities`: pushes entities whose
    /// bounding box intersects the moving block's swept volume during this tick.
    fn push_entities(&self, world: &Arc<World>, new_progress: f32) {
        let last = self.last_progress.load();
        let delta = f64::from(new_progress - last);
        if delta <= 0.0 {
            return;
        }

        let motion_dir = self.movement_direction();
        let amount = f64::from(self.amount_extended(last));

        // Block AABB at the current visual position (start of this tick).
        // For source=true (the piston head BE), vanilla uses the piston-head
        // collision shape — a 4-pixel-thick slab at the front face — rather than
        // the full cube. Without this, the head "sweeps" too much volume and
        // drags entities (e.g. end crystals) back with it during retraction.
        let raw_block_aabb = BoundingBox::from_block(&self.position);
        let block_aabb = if self.source {
            Self::head_shape_aabb(raw_block_aabb, self.facing)
        } else {
            raw_block_aabb
        }
        .shift(Self::dir_vec(self.facing, amount));

        // Stretch by motion to get the swept volume (one tick of animation).
        let motion = Self::dir_vec(motion_dir, delta);
        let swept = block_aabb.stretch(motion);

        for entity in world.get_entities_at_box(&swept) {
            let e = entity.get_entity();
            if e.no_physics.load(Ordering::Relaxed) {
                continue;
            }
            // Player movement is client-authoritative; vanilla still nudges them
            // via Entity.move(PISTON), but we skip them here to avoid teleport jank.
            if entity.get_player().is_some() {
                continue;
            }

            let entity_aabb = e.bounding_box.load();
            let intersection = Self::intersection_size(swept, motion_dir, entity_aabb);
            if intersection <= 0.0 {
                continue;
            }
            let push_amount = intersection.min(delta) + 0.01;
            Self::move_entity(e, motion_dir, push_amount);

            // For a retracting head, vanilla also shoves the entity OUT of the
            // piston body cube. Without this, entities get pulled into the
            // piston and "stick" to it (looks like a sticky-piston drag).
            // For a retract-head BE, `self.position` already IS the piston
            // block position (it replaces the piston during animation).
            if !self.extending && self.source {
                Self::push_out_of_piston_body(e, &self.position, motion_dir, delta);
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

    fn move_entity(entity: &crate::entity::Entity, dir: BlockDirection, distance: f64) {
        let new_pos = entity.pos.load() + Self::dir_vec(dir, distance);
        entity.set_pos(new_pos);
        entity.send_pos();
    }

    /// Vanilla `push`: when a piston head retracts, shove entities that ended up
    /// inside the piston-body cube back out the opposite direction (slightly past
    /// the move they just got, so the net motion is essentially zero).
    fn push_out_of_piston_body(
        entity: &crate::entity::Entity,
        piston_pos: &BlockPos,
        motion_dir: BlockDirection,
        amount: f64,
    ) {
        let body_aabb = BoundingBox::from_block(piston_pos);
        let entity_aabb = entity.bounding_box.load();
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
            Self::move_entity(entity, back, distance);
        }
    }

    /// Approximation of vanilla `PistonHeadBlock` collision shape: a 4-pixel
    /// (0.25 block) slab at the front face of the block in the facing direction.
    fn head_shape_aabb(block_aabb: BoundingBox, facing: BlockDirection) -> BoundingBox {
        const HEAD_THICKNESS: f64 = 0.25;
        let mut min = block_aabb.min;
        let mut max = block_aabb.max;
        match facing {
            BlockDirection::East => min.x = max.x - HEAD_THICKNESS,
            BlockDirection::West => max.x = min.x + HEAD_THICKNESS,
            BlockDirection::Up => min.y = max.y - HEAD_THICKNESS,
            BlockDirection::Down => max.y = min.y + HEAD_THICKNESS,
            BlockDirection::South => min.z = max.z - HEAD_THICKNESS,
            BlockDirection::North => max.z = min.z + HEAD_THICKNESS,
        }
        BoundingBox::new(min, max)
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

    pub fn finish(&self, world: &Arc<World>) {
        if self.last_progress.load() < 1.0 {
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let state = if self.source {
                    Block::AIR.default_state.id
                } else {
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos)
                };
                world.set_block_state(&pos, state, BlockFlags::NOTIFY_ALL);
                world.update_neighbors(&pos, None);
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
                if self.pushed_block_state.is_air() {
                    world.set_block_state(
                        &pos,
                        self.pushed_block_state.id,
                        BlockFlags::FORCE_STATE | BlockFlags::MOVED,
                    );
                } else {
                    let updated_state =
                        world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos);
                    world.set_block_state(
                        &pos,
                        updated_state,
                        BlockFlags::NOTIFY_ALL | BlockFlags::MOVED,
                    );
                    world.update_neighbors(&pos, None);
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

    /// A piston saved mid-push must restore the exact state it was moving. Vanilla stores
    /// it under "blockState" via `BlockState.CODEC` (`PistonMovingBlockEntity.java:359`);
    /// this previously wrote nothing and loaded air, silently deleting the block.
    #[test]
    fn round_trips_states_with_and_without_properties() {
        // No properties -- Name only, no Properties compound.
        let plain = Block::STONE.default_state;
        let encoded = block_state_to_nbt(plain);
        assert_eq!(encoded.get_string(BLOCK_STATE_NAME), Some("minecraft:stone"));
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
        assert_eq!(
            block_state_from_nbt(&bogus).id,
            Block::AIR.default_state.id
        );
    }
}
