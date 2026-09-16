//! Persistent portal/forced tickets and their distinct simulation footprints.
use crate::world::World;
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::vector2::Vector2;
use pumpkin_world::chunk_system::chunk_loading::ChunkLoading;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

type TicketKey = (Vector2<i32>, i32);

#[derive(Default)]
pub struct PortalTickets {
    deadlines: FxHashMap<TicketKey, i64>,
    installed: FxHashSet<TicketKey>,
    installed_forced: FxHashSet<Vector2<i32>>,
    retained: Vec<NbtTag>,
}

impl PortalTickets {
    fn refresh(&mut self, pos: Vector2<i32>, now: i64) {
        self.deadlines.insert((pos, 30), now.saturating_add(300));
    }

    fn expire(&mut self, now: i64) {
        self.deadlines.retain(|_, deadline| now <= *deadline);
    }

    fn chunks(&self, forced: &FxHashSet<Vector2<i32>>, threshold: i32) -> FxHashSet<Vector2<i32>> {
        let mut chunks = FxHashSet::default();
        for (center, level) in self
            .deadlines
            .keys()
            .copied()
            .chain(forced.iter().map(|pos| (*pos, 31)))
        {
            let radius = threshold - level;
            if radius < 0 {
                continue;
            }
            for x in -radius..=radius {
                for z in -radius..=radius {
                    chunks.insert(Vector2::new(
                        center.x.wrapping_add(x),
                        center.y.wrapping_add(z),
                    ));
                }
            }
        }
        chunks
    }

    pub fn read(data: &NbtCompound, now: i64) -> (Self, FxHashSet<Vector2<i32>>) {
        let mut tickets = Self::default();
        let mut forced = FxHashSet::default();
        if let Some(entries) = data.get_list("tickets") {
            for tag in entries {
                let NbtTag::Compound(entry) = tag else {
                    tickets.retained.push(tag.clone());
                    continue;
                };
                let parsed = entry.get("chunk_pos").and_then(codec_chunk_pos).zip(
                    entry
                        .get("level")
                        .and_then(codec_int)
                        .filter(|level| *level >= 0),
                );
                let Some((pos, level)) = parsed else {
                    tickets.retained.push(tag.clone());
                    continue;
                };
                let ticks_left = match entry.get("ticks_left") {
                    None => 0,
                    Some(tag) => match codec_long(tag) {
                        Some(value) => value,
                        None => {
                            tickets.retained.push(NbtTag::Compound(entry.clone()));
                            continue;
                        }
                    },
                };
                match entry
                    .get_string("type")
                    .map(|s| s.strip_prefix("minecraft:").unwrap_or(s))
                {
                    Some("portal") => {
                        // TicketStorage activation merges equal type/level tickets;
                        // a duplicate refreshes the first ticket to the type timeout.
                        tickets
                            .deadlines
                            .entry((pos, level))
                            .and_modify(|deadline| *deadline = now.saturating_add(300))
                            .or_insert_with(|| now.saturating_add(ticks_left));
                    }
                    Some("forced") if level == 31 => {
                        forced.insert(pos);
                    }
                    _ => tickets.retained.push(tag.clone()),
                }
            }
        }
        (tickets, forced)
    }

    fn entry(pos: Vector2<i32>, kind: &str, level: i32, ticks_left: i64) -> NbtTag {
        let mut tag = NbtCompound::new();
        tag.put("chunk_pos", NbtTag::IntArray(vec![pos.x, pos.y]));
        tag.put_string("type", format!("minecraft:{kind}"));
        tag.put_int("level", level);
        if ticks_left != 0 {
            tag.put_long("ticks_left", ticks_left);
        }
        NbtTag::Compound(tag)
    }

    pub fn saved_data(&self, now: i64, forced: &FxHashSet<Vector2<i32>>) -> NbtCompound {
        let mut data = NbtCompound::new();
        let mut entries = self.retained.clone();
        let mut portals: Vec<_> = self.deadlines.iter().collect();
        portals.sort_unstable_by_key(|((pos, level), _)| (pos.x, pos.y, *level));
        for ((pos, level), deadline) in portals {
            entries.push(Self::entry(
                *pos,
                "portal",
                *level,
                deadline.saturating_sub(now),
            ));
        }
        let mut forced: Vec<_> = forced.iter().copied().collect();
        forced.sort_unstable_by_key(|pos| (pos.x, pos.y));
        for pos in forced {
            entries.push(Self::entry(pos, "forced", 31, 0));
        }
        if !entries.is_empty() {
            data.put("tickets", NbtTag::List(entries));
        }
        data
    }

    pub fn load(folder: &Path, now: i64) -> (Self, FxHashSet<Vector2<i32>>) {
        let path = pumpkin_world::world_info::data_files::minecraft_data_dir(folder)
            .join("chunk_tickets.dat");
        if !path.exists() {
            return (Self::default(), FxHashSet::default());
        }
        match std::fs::File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|file| {
                pumpkin_nbt::nbt_compress::read_gzip_compound_tag(file).map_err(|e| e.to_string())
            }) {
            Ok(root) => root
                .get_compound("data")
                .map(|data| Self::read(data, now))
                .unwrap_or_default(),
            Err(error) => {
                tracing::warn!("Failed to read {}: {error}", path.display());
                (Self::default(), FxHashSet::default())
            }
        }
    }

    pub fn save(
        &self,
        folder: &Path,
        now: i64,
        forced: &FxHashSet<Vector2<i32>>,
    ) -> Result<(), String> {
        let dir = pumpkin_world::world_info::data_files::minecraft_data_dir(folder);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", 4903);
        root.put_compound("data", self.saved_data(now, forced));
        let bytes = pumpkin_nbt::nbt_compress::write_gzip_compound_tag_to_bytes(root)
            .map_err(|e| e.to_string())?;
        let temporary = dir.join("chunk_tickets.dat.tmp");
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(temporary, dir.join("chunk_tickets.dat")).map_err(|e| e.to_string())
    }

    fn sync_loading(&mut self, world: &World, forced: &FxHashSet<Vector2<i32>>) {
        let wanted: FxHashSet<_> = self.deadlines.keys().copied().collect();
        let mut loading = world
            .level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mapped = |level: i32| {
            i8::try_from(level.saturating_add(i32::from(ChunkLoading::FULL_CHUNK_LEVEL) - 33))
                .ok()
                .filter(|level| *level < ChunkLoading::MAX_LEVEL)
        };
        for (pos, level) in self.installed.difference(&wanted) {
            if let Some(level) = mapped(*level) {
                loading.remove_ticket(*pos, level);
            }
        }
        for (pos, level) in wanted.difference(&self.installed) {
            if let Some(level) = mapped(*level) {
                loading.add_ticket(*pos, level);
            }
        }
        for pos in self.installed_forced.difference(forced) {
            loading.remove_ticket(*pos, ChunkLoading::FULL_CHUNK_LEVEL - 2);
        }
        for pos in forced.difference(&self.installed_forced) {
            loading.add_ticket(*pos, ChunkLoading::FULL_CHUNK_LEVEL - 2);
        }
        loading.send_change();
        self.installed = wanted;
        self.installed_forced.clone_from(forced);
    }
}

// Codecs consume NbtOps' boxed Number values, whose floating casts truncate.
// This deliberately differs from NumericTag.intValue (which floors).
fn codec_int(tag: &NbtTag) -> Option<i32> {
    Some(match tag {
        NbtTag::Byte(v) => i32::from(*v),
        NbtTag::Short(v) => i32::from(*v),
        NbtTag::Int(v) => *v,
        NbtTag::Long(v) => *v as i32,
        NbtTag::Float(v) => *v as i32,
        NbtTag::Double(v) => *v as i32,
        _ => return None,
    })
}

fn codec_long(tag: &NbtTag) -> Option<i64> {
    Some(match tag {
        NbtTag::Byte(v) => i64::from(*v),
        NbtTag::Short(v) => i64::from(*v),
        NbtTag::Int(v) => i64::from(*v),
        NbtTag::Long(v) => *v,
        NbtTag::Float(v) => *v as i64,
        NbtTag::Double(v) => *v as i64,
        _ => return None,
    })
}

fn codec_chunk_pos(tag: &NbtTag) -> Option<Vector2<i32>> {
    let coordinates: Vec<_> = match tag {
        NbtTag::IntArray(v) => v.clone(),
        NbtTag::LongArray(v) => v.iter().map(|v| *v as i32).collect(),
        NbtTag::ByteArray(v) => v.iter().map(|v| i32::from(*v as i8)).collect(),
        NbtTag::List(v) => v.iter().map(codec_int).collect::<Option<_>>()?,
        _ => return None,
    };
    (coordinates.len() == 2).then(|| Vector2::new(coordinates[0], coordinates[1]))
}

impl World {
    pub(crate) fn place_portal_ticket(&self, pos: Vector2<i32>) {
        let forced = self
            .forced_chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let now = self.get_world_age();
        let mut tickets = self
            .portal_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tickets.refresh(pos, now);
        tickets.sync_loading(self, &forced);
    }

    pub(crate) fn ticket_ticking_chunks(
        &self,
        forced: &FxHashSet<Vector2<i32>>,
    ) -> (FxHashSet<Vector2<i32>>, FxHashSet<Vector2<i32>>) {
        let now = self.get_world_age();
        let mut tickets = self
            .portal_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tickets.expire(now);
        tickets.sync_loading(self, forced);
        (tickets.chunks(forced, 31), tickets.chunks(forced, 32))
    }

    pub(crate) fn save_chunk_tickets(&self) {
        let forced = self
            .forced_chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let now = self.get_world_age();
        if let Err(error) = self
            .portal_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save(&self.level.level_folder.dim_folder, now, &forced)
        {
            tracing::warn!("Failed to save chunk tickets: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_ticket_codec_and_duplicate_activation() {
        let cases: Vec<(String, String)> =
            serde_json::from_str(include_str!("chunk_ticket_cases.json")).unwrap();
        let parse = |text: String| {
            let mut reader = crate::command::string_reader::StringReader::new(text);
            let NbtTag::Compound(data) =
                crate::command::snbt::SnbtParser::parse_for_commands(&mut reader).unwrap()
            else {
                panic!("compound required")
            };
            data
        };
        for (index, (input, expected)) in cases.into_iter().enumerate() {
            let (tickets, forced) = PortalTickets::read(&parse(input), 100);
            assert_eq!(
                tickets.saved_data(100, &forced),
                parse(expected),
                "case {index}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loaded_world_uses_block_ring_defers_events_and_autosaves_tickets() {
        use crate::block::registry::BlockRegistry;
        use arc_swap::ArcSwap;
        use pumpkin_data::{Block, dimension::Dimension};
        use pumpkin_util::{math::position::BlockPos, world_seed::Seed};
        use pumpkin_world::{
            chunk::ChunkData, level::Level, tick::TickPriority, world_info::LevelData,
        };
        use std::sync::{Arc, Weak};

        let dir = tempfile::tempdir().unwrap();
        let level = Level::from_root_folder(
            &pumpkin_config::world::LevelConfig {
                autosave_ticks: 1,
                ..Default::default()
            },
            dir.path().to_path_buf(),
            262,
            Dimension::OVERWORLD,
        );
        for x in -3..=3 {
            for z in -3..=3 {
                level
                    .loaded_chunks
                    .insert(Vector2::new(x, z), ChunkData::empty_sync(x, z));
            }
        }
        let mut saved = PortalTickets::default();
        saved.refresh(Vector2::new(0, 0), 0);
        saved
            .save(&level.level_folder.dim_folder, 0, &FxHashSet::default())
            .unwrap();
        let world = World::load(
            level,
            Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(262)))),
            Dimension::OVERWORLD,
            Arc::new(BlockRegistry::default()),
            Weak::new(),
        );
        world.update_active_chunks();
        assert_eq!(world.active_chunks.read().unwrap().len(), 9);
        assert_eq!(world.block_ticking_chunks.read().unwrap().len(), 25);
        let block_ring = BlockPos::new(32, 64, 0);
        let outside = BlockPos::new(48, 64, 0);
        for pos in [block_ring, outside] {
            world
                .level
                .schedule_block_tick(&Block::STONE, pos, 0, TickPriority::Normal);
        }
        let (due, _) = world
            .level
            .get_scheduled_ticks_if(|pos| world.block_ticking_chunks.read().unwrap().contains(pos));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].position, block_ring);
        assert!(world.level.is_block_tick_scheduled(&outside, &Block::STONE));
        world.add_synced_block_event(outside, 0, 0);
        world.add_synced_block_event(outside, 0, 0);
        world.flush_synced_block_events();
        let event = world
            .synced_block_event_queue
            .lock()
            .unwrap()
            .pop_front()
            .unwrap();
        assert_eq!(event.pos, outside);
        assert!(
            world
                .synced_block_event_queue
                .lock()
                .unwrap()
                .pop_front()
                .is_none()
        );
        world
            .synced_block_event_queue
            .lock()
            .unwrap()
            .push_back(event);
        world
            .forced_chunks
            .lock()
            .unwrap()
            .insert(outside.chunk_position());
        world.update_active_chunks();
        world.flush_synced_block_events();
        assert!(
            world
                .synced_block_event_queue
                .lock()
                .unwrap()
                .pop_front()
                .is_none()
        );
        let (due, _) = world
            .level
            .get_scheduled_ticks_if(|pos| world.block_ticking_chunks.read().unwrap().contains(pos));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].position, outside);
        let center = Vector2::new(0, 0);
        world.level.get_entity_chunk(center).await;
        assert!(!world.level.clean_memory().contains(&center));
        world.level.clean_entity_chunks([center]);
        assert!(world.level.get_entity_chunk_sync(&center).is_some());
        // This calls the actual periodic save path, after releasing the time lock.
        world.tick_environment();
        let (restored, forced) = PortalTickets::load(&world.level.level_folder.dim_folder, 0);
        assert_eq!(restored.deadlines[&(Vector2::new(0, 0), 30)], 299);
        assert!(forced.contains(&outside.chunk_position()));
        world.level_time.lock().unwrap().world_age = 302;
        world.update_active_chunks();
        assert!(!world.level.should_retain_entity_chunk(&center));
        assert!(world.level.clean_memory().contains(&center));
        world.level.shutdown().await;
    }

    #[test]
    fn remaining_lifetime_survives_restart_and_refresh_without_extending_expired_tickets() {
        let dir = tempfile::tempdir().unwrap();
        let mut tickets = PortalTickets::default();
        let a = Vector2::new(-4, 7);
        let b = Vector2::new(10, 10);
        let forced = FxHashSet::from_iter([b]);
        tickets.refresh(a, 10);
        tickets.save(dir.path(), 110, &forced).unwrap();
        let (mut restored, restored_forced) = PortalTickets::load(dir.path(), 0);
        assert_eq!(restored_forced, forced);
        assert_eq!(restored.chunks(&forced, 31).len(), 10);
        assert_eq!(restored.chunks(&forced, 32).len(), 34);
        restored.expire(200);
        assert_eq!(restored.deadlines.len(), 1);
        restored.expire(201);
        assert!(restored.deadlines.is_empty());
        restored.refresh(a, 202);
        restored.refresh(a, 300);
        restored.expire(503);
        assert_eq!(restored.deadlines.len(), 1);
        restored.expire(601);
        assert!(restored.deadlines.is_empty());
        assert_eq!(restored.chunks(&forced, 31), forced);
    }
}
