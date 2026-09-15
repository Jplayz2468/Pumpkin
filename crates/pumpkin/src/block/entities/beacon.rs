use pumpkin_data::Block;
use pumpkin_data::data_component_impl::IDSetContent;
use pumpkin_data::dye_color::DyeColor;
use pumpkin_data::tag::Taggable;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use pumpkin_data::effect::StatusEffect;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::chunk::ChunkHeightmapType;

use crate::block::entities::BlockEntity;
use crate::world::World;
use pumpkin_inventory::window_property::PropertyDelegate;

/// How many new blocks of the beam column are scanned per tick.
/// Vanilla: `BeaconBlockEntity.BLOCKS_CHECK_PER_TICK` (`BeaconBlockEntity.java:65`).
const BLOCKS_CHECK_PER_TICK: i32 = 10;

/// A single coloured segment of the beacon beam.
/// Vanilla: `BeaconBeamOwner.Section` (`BeaconBeamOwner.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeaconBeamSection {
    /// Opaque ARGB colour (alpha is always `0xFF`).
    pub color: u32,
    pub height: i32,
}

impl BeaconBeamSection {
    #[must_use]
    pub const fn new(color: u32) -> Self {
        Self { color, height: 1 }
    }

    pub fn increase_height(&mut self) {
        self.height += 1;
    }
}

/// Component-wise average of two opaque ARGB colours (integer/floor division per channel).
///
/// Vanilla: `ARGB.average` (`ARGB.java:227-229`), used by `BeaconBlockEntity.tick`
/// (`BeaconBlockEntity.java:150`) to *blend* a beam segment's colour with the next,
/// differently-coloured beam block above it, rather than replacing it outright.
#[must_use]
pub const fn average_argb(lhs: u32, rhs: u32) -> u32 {
    let alpha = ((lhs >> 24) + (rhs >> 24)) / 2;
    let red = (((lhs >> 16) & 0xFF) + ((rhs >> 16) & 0xFF)) / 2;
    let green = (((lhs >> 8) & 0xFF) + ((rhs >> 8) & 0xFF)) / 2;
    let blue = ((lhs & 0xFF) + (rhs & 0xFF)) / 2;
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

/// Maps a stained-glass(-pane) name prefix (e.g. `"light_blue"`) to its `DyeColor`.
/// Mirrors the `DyeColor` naming vanilla uses for `StainedGlassBlock`/`StainedGlassPaneBlock`
/// registration (`StainedGlassBlock.java:19`, `StainedGlassPaneBlock.java:19`).
fn dye_color_from_prefix(prefix: &str) -> Option<DyeColor> {
    Some(match prefix {
        "white" => DyeColor::White,
        "orange" => DyeColor::Orange,
        "magenta" => DyeColor::Magenta,
        "light_blue" => DyeColor::LightBlue,
        "yellow" => DyeColor::Yellow,
        "lime" => DyeColor::Lime,
        "pink" => DyeColor::Pink,
        "gray" => DyeColor::Gray,
        "light_gray" => DyeColor::LightGray,
        "cyan" => DyeColor::Cyan,
        "purple" => DyeColor::Purple,
        "blue" => DyeColor::Blue,
        "brown" => DyeColor::Brown,
        "green" => DyeColor::Green,
        "red" => DyeColor::Red,
        "black" => DyeColor::Black,
        _ => return None,
    })
}

/// The beam colour of a block, if it is a beacon-beam block.
///
/// Vanilla: `instanceof BeaconBeamBlock` (`BeaconBlockEntity.java:141`). The blocks
/// implementing `BeaconBeamBlock` (`BeaconBeamBlock.java`) are the beacon itself (always
/// white, `BeaconBlock.java:33-34`), stained glass (`StainedGlassBlock.java:19,25`) and
/// stained glass panes (`StainedGlassPaneBlock.java:19,28`). Plain glass and tinted glass
/// do **not** implement it, so they behave like any other transparent block: they extend
/// the current segment instead of starting a new coloured one.
fn beacon_beam_block_color(block: &'static Block) -> Option<u32> {
    let dye_color = if block == &Block::BEACON {
        DyeColor::White
    } else if let Some(prefix) = block.name.strip_suffix("_stained_glass_pane") {
        dye_color_from_prefix(prefix)?
    } else if let Some(prefix) = block.name.strip_suffix("_stained_glass") {
        dye_color_from_prefix(prefix)?
    } else {
        return None;
    };

    // Vanilla forces the stored value opaque in the `DyeColor` constructor via
    // `ARGB.opaque` (`DyeColor.java:78`); `texture_diffuse_color()` here returns the raw,
    // non-opaque 24-bit RGB constant, so the alpha byte is applied here instead.
    Some(dye_color.texture_diffuse_color() | 0xFF00_0000)
}

pub struct BeaconBlockEntity {
    pub position: BlockPos,
    pub primary_effect: AtomicI32,
    pub secondary_effect: AtomicI32,
    pub levels: AtomicI32,
    pub dirty: AtomicBool,

    // Vanilla Parity Fields
    pub custom_name: Mutex<Option<String>>,
    pub lock_key: Mutex<Option<String>>,
    pub last_check_y: AtomicI32,

    /// The beam's colour segments as of the most recently *completed* column scan.
    /// Vanilla: `BeaconBlockEntity.beamSections` (`BeaconBlockEntity.java:70`).
    pub beam_sections: Mutex<Vec<BeaconBeamSection>>,
    /// The beam segments currently being (re)built, `BLOCKS_CHECK_PER_TICK` blocks per tick.
    /// Vanilla: `BeaconBlockEntity.checkingBeamSections` (`BeaconBlockEntity.java:71`).
    pub checking_beam_sections: Mutex<Vec<BeaconBeamSection>>,
}

impl BeaconBlockEntity {
    pub const ID: &'static str = "minecraft:beacon";

    // ContainerData Property Constants
    pub const DATA_LEVELS: usize = 0;
    pub const DATA_PRIMARY: usize = 1;
    pub const DATA_SECONDARY: usize = 2;
    pub const NUM_DATA_VALUES: usize = 3;

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            primary_effect: AtomicI32::new(-1),
            secondary_effect: AtomicI32::new(-1),
            levels: AtomicI32::new(0),
            dirty: AtomicBool::new(false),
            custom_name: Mutex::new(None),
            lock_key: Mutex::new(None),
            // Vanilla: `lastCheckY` has no explicit initializer, defaulting to `0`
            // (`BeaconBlockEntity.java:73`); it is never persisted to NBT.
            last_check_y: AtomicI32::new(0),
            beam_sections: Mutex::new(Vec::new()),
            checking_beam_sections: Mutex::new(Vec::new()),
        }
    }

    /// Replicates the Java `ContainerData` used to sync values to the `BeaconMenu`
    pub fn get_data(&self, id: usize) -> i32 {
        match id {
            Self::DATA_LEVELS => self.levels.load(Ordering::Relaxed),
            Self::DATA_PRIMARY => self.primary_effect.load(Ordering::Relaxed) + 1,
            Self::DATA_SECONDARY => self.secondary_effect.load(Ordering::Relaxed) + 1,
            _ => 0,
        }
    }

    pub fn set_data(&self, id: usize, value: i32) {
        match id {
            Self::DATA_LEVELS => self.levels.store(value, Ordering::Relaxed),
            Self::DATA_PRIMARY => self
                .primary_effect
                .store(Self::filter_effect(value - 1), Ordering::Relaxed),
            Self::DATA_SECONDARY => self
                .secondary_effect
                .store(Self::filter_effect(value - 1), Ordering::Relaxed),
            _ => {}
        }
        self.mark_dirty();
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    fn required_levels(effect: Option<i32>) -> i32 {
        match effect {
            None => 0,
            Some(id)
                if id == StatusEffect::SPEED.id as i32 || id == StatusEffect::HASTE.id as i32 =>
            {
                1
            }
            Some(id)
                if id == StatusEffect::RESISTANCE.id as i32
                    || id == StatusEffect::JUMP_BOOST.id as i32 =>
            {
                2
            }
            Some(id) if id == StatusEffect::STRENGTH.id as i32 => 3,
            Some(id) if id == StatusEffect::REGENERATION.id as i32 => 4,
            _ => i32::MAX,
        }
    }
    fn filter_effect(id: i32) -> i32 {
        if Self::required_levels(Some(id)) == i32::MAX {
            -1
        } else {
            id
        }
    }
    pub fn is_valid_primary_effect(effect_id: i32, levels: i32) -> bool {
        let required = Self::required_levels(Some(effect_id));
        required < 4 && required <= levels
    }
    pub fn is_valid_secondary_effect(primary_id: i32, secondary_id: i32, levels: i32) -> bool {
        Self::validate_effects(
            (primary_id >= 0).then_some(primary_id),
            (secondary_id >= 0).then_some(secondary_id),
            levels,
        )
    }
    pub fn validate_effects(primary: Option<i32>, secondary: Option<i32>, levels: i32) -> bool {
        if secondary.is_some() && levels < 4 {
            return false;
        }
        let primary_level = Self::required_levels(primary);
        let secondary_level = Self::required_levels(secondary);
        primary_level <= levels
            && secondary_level <= levels
            && primary_level < 4
            && (secondary_level == 0 || secondary_level >= 4 || primary == secondary)
    }

    /// The beam sections to render, gated on the beacon actually being powered.
    ///
    /// Vanilla: `BeaconBlockEntity.getBeamSections` (`BeaconBlockEntity.java:296-299`)
    /// returns an empty beam once `levels` is `0`, even though `beam_sections` itself may
    /// still hold the last-scanned segments.
    ///
    /// Note this is **not** sent to the client over the network. Vanilla's block-entity
    /// sync (`saveAdditional`/`loadAdditional`/`getUpdateTag`, `BeaconBlockEntity.java:305-333`)
    /// only ever writes `primary_effect`/`secondary_effect`/`Levels`/`CustomName`/`Lock` —
    /// no beam colour or section data. The vanilla client instead re-runs this exact
    /// column scan itself, against its own (already block-synced) copy of the world,
    /// because `BeaconBlockEntity::tick` runs identically on both logical sides
    /// (`BeaconBlockEntity.java:124-193` has no `level.isClientSide()` guard around the
    /// scanning loop). So as long as the glass blocks above the beacon and `Levels` are
    /// synced normally, the vanilla client renders the correct mixed colour beam without
    /// Pumpkin needing to put any beam/colour data on the wire.
    #[must_use]
    pub fn get_beam_sections(&self) -> Vec<BeaconBeamSection> {
        if self.levels.load(Ordering::Relaxed) == 0 {
            Vec::new()
        } else {
            self.beam_sections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        }
    }

    /// Scans up to `BLOCKS_CHECK_PER_TICK` new blocks of the column above the beacon each
    /// tick, building `checking_beam_sections`; once a full pass reaches the world-surface
    /// heightmap, publishes the result into `beam_sections` and starts over.
    ///
    /// Ports `BeaconBlockEntity.tick`'s beam-scanning half
    /// (`BeaconBlockEntity.java:120-165`).
    fn tick_beam(&self, world: &Arc<World>) -> bool {
        let x = self.position.0.x;
        let y = self.position.0.y;
        let z = self.position.0.z;

        let mut checking = self
            .checking_beam_sections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let last_check_y = self.last_check_y.load(Ordering::Relaxed);
        let mut check_pos = if last_check_y < y {
            // BeaconBlockEntity.java:127-131: (re)start the scan from the beacon block
            // itself, which is white (`BeaconBlock.java:33-34`).
            checking.clear();
            self.last_check_y.store(y - 1, Ordering::Relaxed);
            BlockPos::new(x, y, z)
        } else {
            BlockPos::new(x, last_check_y + 1, z)
        };

        let last_set_block = world.get_heightmap_height(ChunkHeightmapType::WorldSurface, x, z);

        for _ in 0..BLOCKS_CHECK_PER_TICK {
            if check_pos.0.y > last_set_block {
                break;
            }

            let state = world.get_block_state(&check_pos);
            let block = world.get_block(&check_pos);

            if let Some(color) = beacon_beam_block_color(block) {
                // BeaconBlockEntity.java:142-152: the first two beam-coloured blocks
                // (typically the beacon itself, then the first glass block) always start
                // their own section rather than merging — only the third-and-later blocks
                // can trigger a colour *mix* against the running last section.
                if checking.len() <= 1 {
                    checking.push(BeaconBeamSection::new(color));
                } else if let Some(last) = checking.last_mut() {
                    if color == last.color {
                        last.increase_height();
                    } else {
                        // BeaconBlockEntity.java:150 — mix, don't replace.
                        let mixed_color = average_argb(last.color, color);
                        checking.push(BeaconBeamSection::new(mixed_color));
                    }
                }
            } else {
                // BeaconBlockEntity.java:154-161: any other block only extends the beam if
                // it doesn't (fully) dampen light — bedrock is special-cased so bedrock
                // roofs/floors never block the beam.
                let blocked =
                    checking.is_empty() || (state.opacity >= 15 && block != &Block::BEDROCK);
                if blocked {
                    checking.clear();
                    self.last_check_y.store(last_set_block, Ordering::Relaxed);
                    break;
                }
                if let Some(last) = checking.last_mut() {
                    last.increase_height();
                }
            }

            check_pos = BlockPos::new(x, check_pos.0.y + 1, z);
            self.last_check_y.fetch_add(1, Ordering::Relaxed);
        }

        self.last_check_y.load(Ordering::Relaxed) >= last_set_block
    }

    pub fn update_base(&self, world: &Arc<World>) -> i32 {
        let x = self.position.0.x;
        let y = self.position.0.y;
        let z = self.position.0.z;

        let mut current_level = 0;

        for level in 1..=4 {
            let layer_y = y - level;
            if layer_y < world.dimension.min_y {
                break;
            }

            let mut layer_valid = true;
            for dx in -level..=level {
                for dz in -level..=level {
                    let block_pos = BlockPos::new(x + dx, layer_y, z + dz);
                    let state = world.get_block_state(&block_pos);
                    let block = world.get_block(&block_pos);

                    if !block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_BEACON_BASE_BLOCKS) {
                        layer_valid = false;
                        break;
                    }

                    // Optional: Stricter block type validations can happen here
                    let _ = state;
                }
                if !layer_valid {
                    break;
                }
            }

            if layer_valid {
                current_level = level;
            } else {
                break;
            }
        }

        current_level
    }

    pub fn apply_effects(&self, world: &Arc<World>, levels: i32) {
        let primary_id = self.primary_effect.load(Ordering::Relaxed);
        let secondary_id = self.secondary_effect.load(Ordering::Relaxed);

        if primary_id < 0 {
            return;
        }

        let primary_effect = StatusEffect::from_id(primary_id as u16);
        let secondary_effect = StatusEffect::from_id(secondary_id as u16);

        // Vanilla duration: (9 + levels * 2) * 20 ticks
        let duration_ticks = (9 + levels * 2) * 20;

        // Base amplifier: primary gets amp 1 (Level II) if secondary matches primary
        let base_amp = i32::from(levels >= 4 && primary_id == secondary_id);

        // Vanilla Range is level * 10 + 10 blocks in each horizontal direction
        let range = f64::from(levels * 10 + 10);
        let pos = self.position.0.to_f64();
        let box_min = [pos.x - range, pos.y - range, pos.z - range];
        let box_max = [
            pos.x + range + 1.0,
            pos.y + range + 1.0 + world.dimension.height as f64,
            pos.z + range + 1.0,
        ];
        let bounds = BoundingBox::new_array(box_min, box_max);

        // Apply effect to all players in range
        let players = world.players.load();
        for player in players.iter() {
            if !bounds.intersects(&player.living_entity.entity.bounding_box.load()) {
                continue;
            }

            if let Some(effect) = primary_effect {
                player.add_effect(pumpkin_data::potion::Effect {
                    effect_type: effect,
                    duration: duration_ticks,
                    amplifier: base_amp as u8,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }

            if levels >= 4
                && primary_id != secondary_id
                && let Some(effect) = secondary_effect
            {
                player.add_effect(pumpkin_data::potion::Effect {
                    effect_type: effect,
                    duration: duration_ticks,
                    amplifier: 0,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }
}

impl BlockEntity for BeaconBlockEntity {
    fn to_property_delegate(self: Arc<Self>) -> Option<Arc<dyn PropertyDelegate>> {
        Some(self)
    }
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let primary = nbt
            .get_string("primary_effect")
            .and_then(|s| {
                StatusEffect::from_minecraft_name(s)
                    .or_else(|| StatusEffect::from_name(s))
                    .map(|e| e.id as i32)
            })
            .or_else(|| nbt.get_int("primary_effect"))
            .unwrap_or(-1);
        let secondary = nbt
            .get_string("secondary_effect")
            .and_then(|s| {
                StatusEffect::from_minecraft_name(s)
                    .or_else(|| StatusEffect::from_name(s))
                    .map(|e| e.id as i32)
            })
            .or_else(|| nbt.get_int("secondary_effect"))
            .unwrap_or(-1);
        let levels = nbt.get_int("Levels").unwrap_or(0);
        let custom_name = nbt
            .get_string("CustomName")
            .or_else(|| nbt.get_string("custom_name"))
            .map(std::string::ToString::to_string);
        let lock_key = nbt.get_string("Lock").map(std::string::ToString::to_string);

        Self {
            position,
            primary_effect: AtomicI32::new(primary),
            secondary_effect: AtomicI32::new(secondary),
            levels: AtomicI32::new(levels),
            dirty: AtomicBool::new(false),
            custom_name: Mutex::new(custom_name),
            lock_key: Mutex::new(lock_key),
            // See the comment in `new` — `lastCheckY` is not persisted in vanilla either.
            last_check_y: AtomicI32::new(0),
            beam_sections: Mutex::new(Vec::new()),
            checking_beam_sections: Mutex::new(Vec::new()),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let primary = self.primary_effect.load(Ordering::Relaxed);
        if primary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(primary as u16) {
                nbt.put_string("primary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("primary_effect", primary);
            }
        }
        let secondary = self.secondary_effect.load(Ordering::Relaxed);
        if secondary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(secondary as u16) {
                nbt.put_string("secondary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("secondary_effect", secondary);
            }
        }
        nbt.put_int("Levels", self.levels.load(Ordering::Relaxed));

        if let Some(name) = &*self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Some(lock) = &*self
            .lock_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_string("Lock", lock.clone());
        }
    }

    fn tick(&self, world: &Arc<World>) {
        use pumpkin_data::{
            advancement::Advancement,
            sound::{Sound, SoundCategory},
        };
        let completed = self.tick_beam(world);
        let previous_levels = self.levels.load(Ordering::Relaxed);
        // The previous completed beam remains in use until after this tick's effects.
        if world.get_world_age() % 80 == 0 {
            let unobstructed = !self
                .beam_sections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty();
            if unobstructed {
                self.levels
                    .store(self.update_base(world), Ordering::Relaxed);
            }
            let levels = self.levels.load(Ordering::Relaxed);
            if levels > 0 && unobstructed {
                self.apply_effects(world, levels);
                world.play_sound(
                    Sound::BlockBeaconAmbient,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
        }
        if completed {
            self.last_check_y
                .store(world.dimension.min_y - 1, Ordering::Relaxed);
            *self
                .beam_sections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = self
                .checking_beam_sections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let levels = self.levels.load(Ordering::Relaxed);
            if previous_levels <= 0 && levels > 0 {
                world.play_sound(
                    Sound::BlockBeaconActivate,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
                let p = self.position.to_f64();
                let bounds = BoundingBox::new(
                    p - pumpkin_util::math::vector3::Vector3::new(10.0, 9.0, 10.0),
                    p + pumpkin_util::math::vector3::Vector3::new(10.0, 5.0, 10.0),
                );
                for player in world.players.load().iter() {
                    if bounds.intersects(&player.living_entity.entity.bounding_box.load()) {
                        player.trigger_advancement_criterion(
                            Advancement::NETHER_CREATE_BEACON,
                            "beacon",
                        );
                        if levels == 4 {
                            player.trigger_advancement_criterion(
                                Advancement::NETHER_CREATE_FULL_BEACON,
                                "beacon",
                            );
                        }
                    }
                }
            } else if previous_levels > 0 && levels <= 0 {
                world.play_sound(
                    Sound::BlockBeaconDeactivate,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
        }
    }

    fn on_block_replaced(self: Arc<Self>, world: &Arc<World>, position: &BlockPos) {
        world.play_sound(
            pumpkin_data::sound::Sound::BlockBeaconDeactivate,
            pumpkin_data::sound::SoundCategory::Blocks,
            &position.to_centered_f64(),
        );
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        let primary = self.primary_effect.load(Ordering::Relaxed);
        if primary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(primary as u16) {
                nbt.put_string("primary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("primary_effect", primary);
            }
        }
        let secondary = self.secondary_effect.load(Ordering::Relaxed);
        if secondary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(secondary as u16) {
                nbt.put_string("secondary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("secondary_effect", secondary);
            }
        }
        nbt.put_int("Levels", self.levels.load(Ordering::Relaxed));
        if let Ok(name) = self.custom_name.try_lock()
            && let Some(ref name) = *name
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Ok(lock) = self.lock_key.try_lock()
            && let Some(ref lock) = *lock
        {
            nbt.put_string("Lock", lock.clone());
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PropertyDelegate for BeaconBlockEntity {
    fn get_property(&self, index: i32) -> i32 {
        self.get_data(index as usize)
    }
    fn set_property(&self, index: i32, value: i32) {
        self.set_data(index as usize, value);
    }
    fn get_properties_size(&self) -> i32 {
        3
    }
}

#[cfg(test)]
mod beam_color_tests {
    use pumpkin_data::Block;

    use super::{average_argb, beacon_beam_block_color};

    /// Pins `ARGB.average` (`ARGB.java:227-229`), the exact mixing rule
    /// `BeaconBlockEntity.tick` applies at `BeaconBlockEntity.java:150` when the beam
    /// crosses into a differently-coloured beam block. The expected values below are
    /// worked out by hand from vanilla's own opaque `DyeColor.textureDiffuseColor`
    /// constants (`DyeColor.java`: WHITE=16383998, RED=11546150, LIGHT_BLUE=3847130,
    /// each OR'd with alpha `0xFF000000` per `ARGB.opaque`, `DyeColor.java:78`) — this
    /// test does not reimplement the averaging formula itself, only calls it.
    #[test]
    fn average_argb_matches_vanilla_argb_average() {
        let white = 0xFFF9_FFFEu32; // opaque(16383998)
        let red = 0xFFB0_2E26u32; // opaque(11546150)
        let light_blue = 0xFF3A_B3DAu32; // opaque(3847130)

        // (0xFF+0xFF)/2=0xFF, (0xF9+0xB0)/2=0xD4, (0xFF+0x2E)/2=0x96, (0xFE+0x26)/2=0x92
        assert_eq!(average_argb(white, red), 0xFFD4_9692);
        // Averaging is symmetric.
        assert_eq!(average_argb(red, white), 0xFFD4_9692);
        // Chained mix, as happens when a third differently-coloured segment follows:
        // average(average(white, red), light_blue).
        assert_eq!(average_argb(0xFFD4_9692, light_blue), 0xFF87_A4B6);
        // Alpha stays fully opaque through repeated averaging (255+255)/2 == 255.
        assert_eq!(average_argb(white, red) >> 24, 0xFF);
    }

    /// Pins which blocks vanilla's beam scan treats as `BeaconBeamBlock`s, and what
    /// colour each contributes (`BeaconBlockEntity.java:141`, `BeaconBeamBlock.java`,
    /// `BeaconBlock.java:33-34`, `StainedGlassBlock.java:19,25`,
    /// `StainedGlassPaneBlock.java:19,28`). Tinted glass and plain stone must NOT be
    /// treated as coloured beam blocks.
    #[test]
    fn beacon_beam_block_color_matches_vanilla_beacon_beam_block() {
        assert_eq!(
            beacon_beam_block_color(&Block::BEACON),
            Some(0xFFF9_FFFE) // opaque(DyeColor.WHITE.textureDiffuseColor)
        );
        assert_eq!(
            beacon_beam_block_color(&Block::RED_STAINED_GLASS),
            Some(0xFFB0_2E26) // opaque(DyeColor.RED.textureDiffuseColor)
        );
        assert_eq!(
            beacon_beam_block_color(&Block::LIGHT_BLUE_STAINED_GLASS_PANE),
            Some(0xFF3A_B3DA) // opaque(DyeColor.LIGHT_BLUE.textureDiffuseColor)
        );
        assert_eq!(beacon_beam_block_color(&Block::TINTED_GLASS), None);
        assert_eq!(beacon_beam_block_color(&Block::GLASS), None);
        assert_eq!(beacon_beam_block_color(&Block::STONE), None);
    }
}
