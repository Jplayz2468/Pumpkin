//! Runtime catalyst spreading, from vanilla 26.2 SculkSpreader/SculkBehaviour,
//! SculkBlock, and SculkVeinBlock. World-generation spreaders use a separate cache.
use std::sync::Arc;

use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockState,
    block_properties::{GlowLichenLikeProperties, SculkShriekerLikeProperties},
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    world::WorldEvent,
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::{
    math::{position::BlockPos, vector3::Vector3},
    random::RandomImpl,
};
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rustc_hash::FxHashMap;

use crate::world::World;

const FACE_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

#[derive(Default)]
pub struct SculkSpreader {
    cursors: Vec<ChargeCursor>,
}

struct ChargeCursor {
    pos: BlockPos,
    charge: i32,
    decay_delay: i32,
    update_delay: i32,
    // None (never on sculk) differs from Some(0) (last on a solid sculk block).
    facings: Option<u8>,
}

impl SculkSpreader {
    pub fn is_empty(&self) -> bool {
        self.cursors.is_empty()
    }

    pub fn add_cursors(&mut self, pos: BlockPos, mut charge: u32) {
        while charge > 0 && self.cursors.len() < 32 {
            let current = charge.min(1000);
            self.cursors.push(ChargeCursor {
                pos,
                charge: current as i32,
                decay_delay: 1,
                update_delay: 0,
                facings: None,
            });
            charge -= current;
        }
    }

    pub fn from_nbt(nbt: &NbtCompound) -> Self {
        let cursors = nbt
            .get_list("cursors")
            .filter(|list| list.len() <= 32)
            .and_then(|list| {
                list.iter()
                    .map(ChargeCursor::from_nbt)
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        Self { cursors }
    }

    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_list(
            "cursors",
            self.cursors.iter().map(ChargeCursor::to_nbt).collect(),
        );
    }

    pub fn tick(&mut self, world: &Arc<World>, origin: BlockPos) {
        let ticking = world
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&origin.chunk_and_chunk_relative_position().0);
        let mut processed: Vec<ChargeCursor> = Vec::new();
        let mut mergeable: FxHashMap<BlockPos, usize> = FxHashMap::default();
        let mut charges: FxHashMap<BlockPos, i32> = FxHashMap::default();
        for mut cursor in self.cursors.drain(..) {
            let distance = (i64::from(cursor.pos.0.x) - i64::from(origin.0.x))
                .abs()
                .max((i64::from(cursor.pos.0.y) - i64::from(origin.0.y)).abs())
                .max((i64::from(cursor.pos.0.z) - i64::from(origin.0.z)).abs());
            if distance > 1024 {
                continue;
            }
            if ticking {
                cursor.update(world, origin);
            }
            if cursor.charge <= 0 {
                world.sync_world_event(WorldEvent::ParticlesSculkCharge, cursor.pos, 0);
                continue;
            }
            *charges.entry(cursor.pos).or_default() += cursor.charge;
            if let Some(&index) = mergeable.get(&cursor.pos) {
                let existing = &mut processed[index];
                if existing.charge + cursor.charge <= 1000 {
                    existing.charge += cursor.charge;
                    existing.update_delay = existing.update_delay.min(cursor.update_delay);
                    continue;
                }
                if cursor.charge < existing.charge {
                    mergeable.insert(cursor.pos, processed.len());
                }
            } else {
                mergeable.insert(cursor.pos, processed.len());
            }
            processed.push(cursor);
        }
        for (pos, charge) in charges {
            if let Some(faces) = processed[mergeable[&pos]].facings {
                let particles = ((f64::from(charge).ln_1p() / f64::from(2.3_f32)) as i32) + 1;
                world.sync_world_event(
                    WorldEvent::ParticlesSculkCharge,
                    pos,
                    (particles << 6) + i32::from(faces),
                );
            }
        }
        self.cursors = processed;
    }
}

impl ChargeCursor {
    fn from_nbt(tag: &NbtTag) -> Option<Self> {
        let NbtTag::Compound(nbt) = tag else {
            return None;
        };
        let [x, y, z] = nbt.get_int_array("pos")? else {
            return None;
        };
        // Optional codec fields default only when absent, not when malformed.
        let integer = |key, default| {
            if nbt.get(key).is_some() {
                nbt.get_int(key)
            } else {
                Some(default)
            }
        };
        let charge = integer("charge", 0)?;
        let decay_delay = integer("decay_delay", 1)?;
        let update_delay = integer("update_delay", 0)?;
        if !(0..=1000).contains(&charge) || !(0..=1).contains(&decay_delay) || update_delay < 0 {
            return None;
        }
        let facings = nbt.get_list("facings").and_then(|list| {
            list.iter().try_fold(0_u8, |faces, entry| {
                let NbtTag::String(name) = entry else {
                    return None;
                };
                FACE_NAMES
                    .iter()
                    .position(|candidate| *candidate == name.as_ref())
                    .map(|index| faces | (1 << index))
            })
        });
        Some(Self {
            pos: BlockPos::new(*x, *y, *z),
            charge,
            decay_delay,
            update_delay,
            facings,
        })
    }

    fn to_nbt(&self) -> NbtTag {
        let mut nbt = NbtCompound::new();
        nbt.put(
            "pos",
            NbtTag::IntArray(vec![self.pos.0.x, self.pos.0.y, self.pos.0.z]),
        );
        nbt.put_int("charge", self.charge);
        nbt.put_int("decay_delay", self.decay_delay);
        nbt.put_int("update_delay", self.update_delay);
        if let Some(faces) = self.facings {
            nbt.put_list(
                "facings",
                FACE_NAMES
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| faces & (1 << index) != 0)
                    .map(|(_, name)| NbtTag::String((*name).into()))
                    .collect(),
            );
        }
        NbtTag::Compound(nbt)
    }

    fn update(&mut self, world: &Arc<World>, origin: BlockPos) {
        if self.charge <= 0 {
            return;
        }
        if self.update_delay > 0 {
            self.update_delay -= 1;
            return;
        }
        let mut state = world.get_block_state(&self.pos);
        let mut block = state.id.to_block().id;
        let spread = if is_sculk(block) {
            spread_all(world, state, self.pos, false)
        } else {
            match self.facings {
                None => spread_all(world, state, self.pos, true),
                Some(0) => spread_all(world, state, self.pos, false),
                Some(faces) => {
                    (state.is_air() || water_source(state)) && regrow(world, state, self.pos, faces)
                }
            }
        };
        if spread {
            if block != BlockId::SCULK {
                state = world.get_block_state(&self.pos);
                block = state.id.to_block().id;
            }
            spread_sound(world, self.pos);
        }
        self.charge = match block {
            BlockId::SCULK => use_sculk_charge(world, self.pos, origin, self.charge),
            BlockId::SCULK_VEIN => {
                if place_sculk(world, self.pos) {
                    self.charge - 1
                } else if next_int(world, 10) == 0 {
                    self.charge / 2
                } else {
                    self.charge
                }
            }
            _ => {
                if self.decay_delay > 0 {
                    self.charge
                } else {
                    0
                }
            }
        };
        if self.charge <= 0 {
            discharge(world, state, self.pos);
            return;
        }
        if let Some(next) = movement_pos(world, self.pos) {
            discharge(world, state, self.pos);
            self.pos = next;
            state = world.get_block_state(&next);
        }
        if is_sculk(state.id.to_block().id) {
            self.facings = Some(faces(state));
        }
        // Delays belong to the behavior that just used charge, before movement.
        self.decay_delay = if is_sculk(block) {
            1
        } else {
            (self.decay_delay - 1).max(0)
        };
        self.update_delay = 1;
    }
}

fn is_sculk(block: BlockId) -> bool {
    matches!(block, BlockId::SCULK | BlockId::SCULK_VEIN)
}
fn has_water(state: &BlockState) -> bool {
    World::fluid_state_from_block_state(state.id)
        .0
        .matches_type(&pumpkin_data::fluid::Fluid::WATER)
}
fn has_fluid(state: &BlockState) -> bool {
    !World::fluid_state_from_block_state(state.id).1.is_empty
}
fn water_source(state: &BlockState) -> bool {
    has_water(state) && World::fluid_state_from_block_state(state.id).1.is_source
}
fn relative(pos: BlockPos, dir: BlockDirection) -> BlockPos {
    pos.offset(dir.to_offset())
}
fn next_int(world: &World, bound: i32) -> i32 {
    world
        .random
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .next_bounded_i32(bound)
}
fn shuffle<T>(world: &World, values: &mut [T]) {
    // Util.shuffle / Direction.allShuffled (descending Fisher-Yates).
    for size in (2..=values.len()).rev() {
        values.swap(size - 1, next_int(world, size as i32) as usize);
    }
}
fn spread_sound(world: &World, pos: BlockPos) {
    world.play_sound(
        Sound::BlockSculkSpread,
        SoundCategory::Blocks,
        &pos.to_centered_f64(),
    );
}
fn faces(state: &BlockState) -> u8 {
    if !matches!(
        state.id.to_block().id,
        BlockId::SCULK_VEIN | BlockId::GLOW_LICHEN | BlockId::RESIN_CLUMP
    ) {
        return 0;
    }
    let p = GlowLichenLikeProperties::from_state_id(state.id);
    [p.down, p.up, p.north, p.south, p.west, p.east]
        .into_iter()
        .enumerate()
        .fold(0, |mask, (index, value)| mask | (u8::from(value) << index))
}
fn with_faces(mask: u8, waterlogged: bool) -> GlowLichenLikeProperties {
    with_faces_for(&Block::SCULK_VEIN, mask, waterlogged)
}
fn with_faces_for(block: &Block, mask: u8, waterlogged: bool) -> GlowLichenLikeProperties {
    let mut p = GlowLichenLikeProperties::default(block);
    p.down = mask & 1 != 0;
    p.up = mask & 2 != 0;
    p.north = mask & 4 != 0;
    p.south = mask & 8 != 0;
    p.west = mask & 16 != 0;
    p.east = mask & 32 != 0;
    p.waterlogged = waterlogged;
    p
}

/// MultifaceBlock.canAttachTo: either support or collision face may be full.
pub(super) fn can_attach(world: &dyn BlockAccessor, pos: BlockPos, dir: BlockDirection) -> bool {
    let state = world.get_block_state(&relative(pos, dir));
    let side = dir.opposite();
    if state.is_side_solid(side) {
        return true;
    }
    crate::block::shape::collision_face_covers(
        state,
        relative(pos, dir),
        side,
        [0.0, 1.0, 0.0, 1.0],
    )
}

fn spread_all(world: &Arc<World>, state: &BlockState, pos: BlockPos, same_space: bool) -> bool {
    let other_source = state.id.to_block().id != BlockId::SCULK_VEIN;
    let mask = faces(state);
    let mut spread = false;
    for from in BlockDirection::all() {
        if !other_source && mask & (1 << from as u8) == 0 {
            continue;
        }
        for toward in BlockDirection::all() {
            if from.to_axis() == toward.to_axis()
                || (!other_source && mask & (1 << toward as u8) != 0)
            {
                continue;
            }
            let targets = [
                (pos, toward),
                (relative(pos, toward), from),
                (relative(relative(pos, toward), from), toward.opposite()),
            ];
            for &(target, face) in &targets[..if same_space { 1 } else { 3 }] {
                if spread_to(world, pos, target, face) {
                    spread = true;
                    break;
                }
            }
        }
    }
    spread
}
fn spread_to(world: &Arc<World>, source: BlockPos, pos: BlockPos, face: BlockDirection) -> bool {
    let Some(state) = spread_state(world, source, pos, face, &Block::SCULK_VEIN) else {
        return false;
    };
    if !world.is_in_build_limit(pos) {
        return false;
    }
    world.set_block_state(&pos, state, BlockFlags::NOTIFY_LISTENERS);
    true
}

fn spread_state(
    world: &World,
    source: BlockPos,
    pos: BlockPos,
    face: BlockDirection,
    target: &Block,
) -> Option<pumpkin_data::BlockStateId> {
    let state = world.get_block_state(&pos);
    let block = state.id.to_block();
    let sculk = target.id == BlockId::SCULK_VEIN;
    if sculk {
        let support = world.get_block(&relative(pos, face)).id;
        if matches!(
            support,
            BlockId::SCULK | BlockId::SCULK_CATALYST | BlockId::MOVING_PISTON
        ) {
            return None;
        }
        let delta = pos.0 - source.0;
        if delta.x.abs() + delta.y.abs() + delta.z.abs() == 2
            && world
                .get_block_state(&relative(source, face.opposite()))
                .is_side_solid(face)
        {
            return None;
        }
        if (has_fluid(state) && !water_source(state)) || block.has_tag(&tag::Block::MINECRAFT_FIRE)
        {
            return None;
        }
    }
    if !(state.is_air()
        || block.id == target.id
        || (block.id == BlockId::WATER && water_source(state))
        || (sculk && state.replaceable()))
    {
        return None;
    }
    let mask = if block.id == target.id {
        faces(state)
    } else {
        0
    };
    let bit = 1 << face as u8;
    if mask & bit != 0 || !can_attach(world, pos, face) {
        return None;
    }
    let waterlogged = if block.id == target.id {
        state.is_waterlogged()
    } else {
        water_source(state)
    };
    Some(with_faces_for(target, mask | bit, waterlogged).to_state_id(target))
}

fn lichen_target(
    world: &World,
    pos: BlockPos,
    state: &BlockState,
    from: BlockDirection,
    toward: BlockDirection,
) -> Option<(BlockPos, pumpkin_data::BlockStateId)> {
    let mask = faces(state);
    if from.to_axis() == toward.to_axis()
        || mask & (1 << from as u8) == 0
        || mask & (1 << toward as u8) != 0
    {
        return None;
    }
    [
        (pos, toward),
        (relative(pos, toward), from),
        (relative(relative(pos, toward), from), toward.opposite()),
    ]
    .into_iter()
    .find_map(|(target, face)| {
        spread_state(world, pos, target, face, &Block::GLOW_LICHEN).map(|state| (target, state))
    })
}

pub(super) fn can_bonemeal_lichen(world: &World, pos: BlockPos, state: &BlockState) -> bool {
    BlockDirection::all().into_iter().any(|from| {
        BlockDirection::all()
            .into_iter()
            .any(|toward| lichen_target(world, pos, state, from, toward).is_some())
    })
}

pub(super) fn bonemeal_lichen(world: &Arc<World>, pos: BlockPos, state: &BlockState) {
    let mut from_faces = BlockDirection::all();
    shuffle(world, &mut from_faces);
    for from in from_faces {
        if faces(state) & (1 << from as u8) == 0 {
            continue;
        }
        let mut directions = BlockDirection::all();
        shuffle(world, &mut directions);
        for toward in directions {
            if let Some((target, state)) = lichen_target(world, pos, state, from, toward)
                && world.is_in_build_limit(target)
            {
                world.set_block_state(&target, state, BlockFlags::NOTIFY_LISTENERS);
                return;
            }
        }
    }
}

fn regrow(world: &Arc<World>, state: &BlockState, pos: BlockPos, mask: u8) -> bool {
    let mut valid = 0;
    for dir in BlockDirection::all() {
        let bit = 1 << dir as u8;
        if mask & bit != 0 && can_attach(world.as_ref(), pos, dir) {
            valid |= bit;
        }
    }
    if valid == 0 {
        return false;
    }
    world.set_block_state(
        &pos,
        with_faces(valid, has_fluid(state)).to_state_id(&Block::SCULK_VEIN),
        BlockFlags::NOTIFY_ALL,
    );
    true
}
fn discharge(world: &Arc<World>, state: &BlockState, pos: BlockPos) {
    if state.id.to_block().id != BlockId::SCULK_VEIN {
        return;
    }
    let mut mask = faces(state);
    for dir in BlockDirection::all() {
        if world.get_block(&relative(pos, dir)).id == BlockId::SCULK {
            mask &= !(1 << dir as u8);
        }
    }
    let replacement = if mask == 0 {
        if has_fluid(world.get_block_state(&pos)) {
            Block::WATER
        } else {
            Block::AIR
        }
        .default_state
        .id
    } else {
        with_faces(mask, state.is_waterlogged()).to_state_id(&Block::SCULK_VEIN)
    };
    world.set_block_state(&pos, replacement, BlockFlags::NOTIFY_ALL);
}
fn place_sculk(world: &Arc<World>, pos: BlockPos) -> bool {
    let mask = faces(world.get_block_state(&pos));
    let mut directions = BlockDirection::all();
    shuffle(world, &mut directions);
    for dir in directions {
        let support = relative(pos, dir);
        if mask & (1 << dir as u8) == 0
            || !world
                .get_block(&support)
                .has_tag(&tag::Block::MINECRAFT_SCULK_REPLACEABLE)
        {
            continue;
        }
        let old_state = world.get_block_state(&support);
        world.set_block_state(
            &support,
            Block::SCULK.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        crate::block::shape::push_entities_up(
            world,
            support,
            old_state,
            Block::SCULK.default_state,
        );
        spread_sound(world, support);
        spread_all(world, Block::SCULK.default_state, support, false);
        for neighbor in BlockDirection::all() {
            if neighbor != dir.opposite() {
                let neighbor_pos = relative(support, neighbor);
                discharge(world, world.get_block_state(&neighbor_pos), neighbor_pos);
            }
        }
        return true;
    }
    false
}
fn movement_pos(world: &World, pos: BlockPos) -> Option<BlockPos> {
    let mut offsets = pumpkin_world::generation::feature::features::sculk::NON_CORNER_NEIGHBOURS;
    shuffle(world, &mut offsets);
    let mut found = None;
    for offset in offsets {
        let next = pos.offset(offset);
        let state = world.get_block_state(&next);
        if !is_sculk(state.id.to_block().id) {
            continue;
        }
        if offset.x.abs() + offset.y.abs() + offset.z.abs() != 1 {
            let directions = [
                (offset.x, BlockDirection::West, BlockDirection::East),
                (offset.y, BlockDirection::Down, BlockDirection::Up),
                (offset.z, BlockDirection::North, BlockDirection::South),
            ];
            if !directions
                .into_iter()
                .filter(|(delta, _, _)| *delta != 0)
                .any(|(delta, negative, positive)| {
                    let dir = if delta < 0 { negative } else { positive };
                    !world
                        .get_block_state(&relative(pos, dir))
                        .is_side_solid(dir.opposite())
                })
            {
                continue;
            }
        }
        found = Some(next);
        let mask = faces(state);
        if BlockDirection::all().into_iter().any(|dir| {
            mask & (1 << dir as u8) != 0
                && world
                    .get_block(&relative(next, dir))
                    .has_tag(&tag::Block::MINECRAFT_SCULK_REPLACEABLE)
        }) {
            break;
        }
    }
    found
}
fn use_sculk_charge(world: &Arc<World>, pos: BlockPos, origin: BlockPos, charge: i32) -> i32 {
    if charge == 0 || next_int(world, 10) != 0 {
        return charge;
    }
    let delta = pos.0 - origin.0;
    let distance_squared =
        f64::from(delta.x).powi(2) + f64::from(delta.y).powi(2) + f64::from(delta.z).powi(2);
    let close = distance_squared < 16.0;
    if !close && can_place_growth(world, pos) {
        if next_int(world, 10) < charge {
            let above = pos.up();
            let shrieker = next_int(world, 11) == 0;
            let block = if shrieker {
                &Block::SCULK_SHRIEKER
            } else {
                &Block::SCULK_SENSOR
            };
            let mut state = block.default_state.id;
            if shrieker {
                let mut props = SculkShriekerLikeProperties::from_state_id(state);
                props.can_summon = false;
                state = props.to_state_id(block);
            }
            if has_fluid(world.get_block_state(&above)) {
                state = block.set_waterlogged(state, true).unwrap_or(state);
            }
            world.set_block_state(&above, state, BlockFlags::NOTIFY_ALL);
            world.play_sound(
                if shrieker {
                    Sound::BlockSculkShriekerPlace
                } else {
                    Sound::BlockSculkSensorPlace
                },
                SoundCategory::Blocks,
                &pos.to_centered_f64(),
            );
        }
        (charge - 10).max(0)
    } else if next_int(world, 5) != 0 {
        charge
    } else {
        let penalty = if close {
            1
        } else {
            let outer_squared = (distance_squared.sqrt() as f32 - 4.0).powi(2);
            ((charge as f32 * (outer_squared / 400.0).min(1.0) * 0.5) as i32).max(1)
        };
        charge - penalty
    }
}
fn can_place_growth(world: &World, pos: BlockPos) -> bool {
    let state = world.get_block_state(&pos.up());
    if !state.is_air() && !(state.id.to_block().id == BlockId::WATER && water_source(state)) {
        return false;
    }
    let mut count = 0;
    for z in -4..=4 {
        for y in 0..=2 {
            for x in -4..=4 {
                if matches!(
                    world.get_block(&pos.offset(Vector3::new(x, y, z))).id,
                    BlockId::SCULK_SENSOR | BlockId::SCULK_SHRIEKER
                ) {
                    count += 1;
                }
                if count > 2 {
                    return false;
                }
            }
        }
    }
    true
}
