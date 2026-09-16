use crate::block::entities::{BlockEntity, block_entity_from_nbt};
use dashmap::DashMap;
use pumpkin_data::chunk::Biome;
use pumpkin_data::item::{BedrockItem, BedrockItemVersion};
use pumpkin_protocol::bedrock::client::item_registry::{CItemRegistry, ItemData};
use pumpkin_protocol::bedrock::client::level_event::{CLevelEvent, LevelEvent};
use pumpkin_protocol::bedrock::client::{CBiomeDefinitionList, block_actor_data::CBlockActorData};
use pumpkin_protocol::bedrock::network_item::{NetworkItemDescriptor, NetworkItemStackDescriptor};
use pumpkin_protocol::codec::data_component::data_to_proto_sound;
use pumpkin_world::generation::proto_chunk::GenerationCache;
use rayon::prelude::*;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, RwLock, Weak};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::Ordering,
};
use tracing::{debug, error, info, trace, warn};

mod active_chunks;
mod block_ray;
mod fluid_flow;
mod sound_delivery;
pub use block_ray::{RayFluidHandling, RayHit};
mod block_interaction_shapes;
pub mod chunker;
pub mod explosion;
pub mod generation_cache;
pub mod loot;
pub mod map;
pub mod neighbor_updater;
pub mod portal;
pub mod raid;
pub mod random_sequences;
mod scheduled_dispatch;
pub mod stopwatches;
pub mod time;
pub mod villager_poi;
pub mod warden_spawn;
pub mod warden_spawn_attempts;
pub mod warden_spawn_position;

use crate::block::RandomTickArgs;
use crate::world::chunker::is_within_chebyshev_distance;
use crate::{block::BlockEvent, entity::item::ItemEntity};
use crate::{
    block::{
        registry::BlockRegistry,
        {OnNeighborUpdateArgs, OnScheduledTickArgs},
    },
    command::client_suggestions,
    entity::{Entity, EntityBase, RemovalReason, player::Player, r#type::from_type},
    error::PumpkinError,
    net::{ClientPlatform, bedrock::BedrockClient, java::JavaClient},
    plugin::{
        block::block_break::BlockBreakEvent,
        player::{
            player_change_world::PlayerChangeWorldEvent, player_join::PlayerJoinEvent,
            player_leave::PlayerLeaveEvent, player_respawn::PlayerRespawnEvent,
        },
    },
    server::Server,
};
use active_chunks::{ActiveChunkTracker, ActivePlayerArea};
use arc_swap::ArcSwap;
use border::Worldborder;
use bytes::BufMut;
pub use explosion::{
    BlockInteraction, DefaultExplosionDamageCalculator, Explosion, ExplosionDamageCalculator,
    ExplosionInteraction, SimpleExplosionDamageCalculator,
};
use pumpkin_config::BasicConfiguration;
use pumpkin_data::block_properties::{blocks_movement, is_air};
use pumpkin_data::block_rotation::{Mirror, Rotation};
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::dimension::Dimension;
use pumpkin_data::entity::MobCategory;
use pumpkin_data::fluid::FluidState;
use pumpkin_data::game_rules::{GameRule, GameRuleValue};
use pumpkin_data::noise_settings::NoiseSettings;
use pumpkin_data::{
    Block, BlockId, BlockStateId,
    entity::{EntityStatus, EntityType},
    fluid::Fluid,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
    sound_id_remap::remap_sound_id_for_version,
    world::{RAW, WorldEvent},
};
use pumpkin_data::{
    BlockDirection, BlockState, HorizontalFacingExt,
    block_properties::{ChestLikeProperties, ChestType},
    tag::Taggable,
    translation,
};
use pumpkin_inventory::crafting::recipe_provider::RecipeProvider;
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_inventory::{Clearable, Inventory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::client::set_actor_data::{CSetActorData, PropertySyncData};
use pumpkin_protocol::bedrock::client::start_game::{CStartGame, ServerTelemetryData};
use pumpkin_protocol::java::client::play::{
    CBlockUpdate, CDisguisedChatMessage, CExplosion, CRespawn, CSetBlockDestroyStage, CWorldEvent,
    PlayerSpawnData,
};
use pumpkin_protocol::java::client::play::{
    CPlayerSpawnPosition, CRecipeBookAdd, CRecipeBookSettings, CSystemChatMessage,
};
use pumpkin_protocol::java::client::play::{CSetEntityMetadata, Metadata};
use pumpkin_protocol::{
    BClientPacket, ClientPacket, IdOr, SoundEvent,
    bedrock::{
        client::{
            add_player::CAddPlayer,
            block_event::CBlockEvent as CBedrockBlockEvent,
            common::BuildPlatform,
            creative_content::{
                CCreativeContent, CreativeCategory, CreativeGroupInfoPayload,
                CreativeItemEntryPayload,
            },
            level_sound_event::CLevelSoundEvent,
            player_list::{CPlayerList, PlayerListEntry, Skin},
            remove_actor::CRemoveActor,
            start_game::{Experiments, GamePublishSetting, LevelSettings},
            update_attributes::{AttributeData, CUpdateAttributes},
        },
        server::{
            actor_event::{ActorEventID, SActorEvent},
            text::SText,
        },
    },
    codec::{var_int::VarInt, var_long::VarLong, var_uint::VarUInt, var_ulong::VarULong},
    java::{
        self,
        client::play::{
            CBlockEntityData, CDamageEvent, CEntityStatus, CGameEvent, CLogin, CMultiBlockUpdate,
            CPlayerInfoUpdate, CRemoveEntities, CRemovePlayerInfo, CSetSelectedSlot, CSoundEffect,
            CSpawnEntity, GameEvent, InitChat, PlayerAction, PlayerInfoFlags,
        },
        server::play::SChatMessage,
    },
};
use pumpkin_protocol::{
    codec::item_stack_seralizer::ItemStackSerializer,
    java::client::play::{
        CBlockEvent, CParticle, CRemoveMobEffect, CSetEquipment, CUpdateMobEffect,
    },
};
use pumpkin_util::resource_location::ResourceLocation;
use pumpkin_util::text::{TextComponent, color::NamedColor};
use pumpkin_util::version::JavaMinecraftVersion;
use pumpkin_util::{
    Difficulty,
    math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3},
};
use pumpkin_util::{
    math::{get_section_cord, position::chunk_section_from_pos, vector2::Vector2},
    random::{RandomImpl, get_seed},
};
use pumpkin_world::world::{GetBlockError, WorldPortalExt};
use pumpkin_world::{
    CURRENT_BEDROCK_MC_VERSION, biome,
    chunk::{io::Dirtiable, palette::bedrock_water_state},
};
use pumpkin_world::{chunk::ChunkData, world::BlockAccessor};
use pumpkin_world::{level::Level, tick::TickPriority};
pub use pumpkin_world::{world::BlockFlags, world_info::LevelData};
use rand::{RngExt, rng};
use scoreboard::Scoreboard;
use time::LevelTime;

pub mod block_placer;
pub mod border;
pub mod bossbar;
pub mod custom_bossbar;
pub mod dragon_fight;
pub mod end_podium;
pub mod entity_tracker;
pub mod environment;
pub mod natural_spawner;
pub mod precipitation;
pub mod scoreboard;
pub mod weather;

pub use environment::EnvironmentAttributes;
pub use pumpkin_data::environment_attribute::{Activity, MoonPhase};

use crate::world::natural_spawner::{SpawnState, spawn_for_chunk};
use pumpkin_config::lighting::LightingEngineConfig;
use pumpkin_data::effect::StatusEffect;
use pumpkin_world::chunk::ChunkHeightmapType::{self, MotionBlocking};
use uuid::Uuid;
use weather::Weather;

const MAX_LIGHT_LEVEL: u8 = 15;

/// Pumpkin-side safety net with no vanilla equivalent: `ServerLevel.runBlockEvents`
/// drains without a bound and relies on block logic terminating. A bug that re-enqueued
/// forever would hang the server, so cap the chain and report it instead.
const MAX_SYNCED_BLOCK_EVENTS_PER_TICK: u32 = 1_000_000;

fn bedrock_chest_block_actor(state_id: BlockStateId, position: BlockPos) -> Option<NbtCompound> {
    let (block, _) = BlockState::from_id_with_block(state_id);
    if !block.has_tag(&pumpkin_data::tag::Block::C_CHESTS_WOODEN)
        && !block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS)
    {
        return None;
    }

    // Block actor tags describe the chest itself. Container contents are synchronized
    // through inventory packets and must not be exposed in chunk data.
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "Chest".to_string());
    nbt.put_int("x", position.0.x);
    nbt.put_int("y", position.0.y);
    nbt.put_int("z", position.0.z);
    nbt.put_bool("isMovable", true);

    let properties = ChestLikeProperties::from_state_id(state_id);
    if properties.r#type != ChestType::Single {
        let direction = if properties.r#type == ChestType::Left {
            properties.facing.rotate_clockwise()
        } else {
            properties.facing.rotate_counter_clockwise()
        };
        let pair = position.offset(direction.to_offset());
        nbt.put_int("pairx", pair.0.x);
        nbt.put_int("pairz", pair.0.z);
        if properties.r#type == ChestType::Right {
            nbt.put_bool("pairlead", true);
        }
    }

    Some(nbt)
}

use rustc_hash::{FxHashMap, FxHashSet};

impl PumpkinError for GetBlockError {
    fn is_kick(&self) -> bool {
        false
    }

    fn severity(&self) -> tracing::Level {
        tracing::Level::WARN
    }

    fn client_kick_reason(&self) -> Option<String> {
        None
    }
}

/// Represents a Minecraft world, containing entities, players, and the underlying level data.
///
/// Each dimension (Overworld, Nether, End) typically has its own `World`.
///
/// **Key Responsibilities:**
///
/// - Manages the `Level` instance for handling chunk-related operations.
/// - Stores and tracks active `Player` entities within the world.
/// - Provides a central hub for interacting with the world's entities and environment.
pub struct World {
    /// Weak ownership lets block-entity inventories perform synchronous world callbacks.
    self_reference: Weak<World>,
    /// Level owns a separate thread-safe legacy stream for sound packet seeds.
    sound_random: std::sync::Mutex<pumpkin_util::random::legacy_rand::LegacyRand>,
    /// Java-compatible Level random stream. Spawn finalization uses this owner;
    /// remaining world RNG consumers are migrated separately.
    pub random: std::sync::Mutex<pumpkin_util::random::legacy_rand::LegacyRand>,
    /// Represents the World's Unique Identifier
    pub uuid: Uuid,
    /// The underlying level, responsible for chunk management and terrain generation.
    pub level: Arc<Level>,
    pub level_info: Arc<ArcSwap<LevelData>>,
    /// A map of active players within the world, keyed by their unique UUID.
    pub players: ArcSwap<Vec<Arc<Player>>>,
    /// A map of active entities within the world, keyed by their unique UUID.
    /// This does not include players.
    pub entities: ArcSwap<Vec<Arc<dyn EntityBase>>>,
    /// The world's scoreboard, used for tracking scores, objectives, and display information.
    pub scoreboard: std::sync::Mutex<Scoreboard>,
    /// The world's worldborder, defining the playable area and controlling its expansion or contraction.
    pub worldborder: std::sync::Mutex<Worldborder>,
    /// Vanilla `ServerLevel.handlingTick` (`ServerLevel.java:217`): true from the start of
    /// the tick through `runBlockEvents`, false during entity and block-entity ticking.
    /// `PistonBaseBlock.checkIfExtend` reads it to decide whether a retracting sticky
    /// piston still drags its block.
    handling_tick: std::sync::atomic::AtomicBool,
    /// The world's time, including counting ticks for weather, time cycles, and statistics.
    pub level_time: std::sync::Mutex<LevelTime>,
    /// The type of dimension the world is in.
    pub dimension: Dimension,
    pub sea_level: i32,
    pub min_y: i32,
    /// The world's weather, including rain and thunder levels.
    pub weather: std::sync::Mutex<Weather>,
    /// Block Behaviour
    pub block_registry: Arc<BlockRegistry>,
    pub server: Weak<Server>,
    synced_block_event_queue: std::sync::Mutex<std::collections::VecDeque<BlockEvent>>,
    /// A map of unsent block changes, keyed by block position.
    unsent_block_changes: std::sync::Mutex<HashMap<BlockPos, BlockStateId>>,
    /// Persisted vanilla POI storage for portal and villager lookups.
    pub portal_poi: std::sync::Mutex<portal::PortalPoiStorage>,
    /// Villager job sites and their current owners.
    pub villager_poi: std::sync::Mutex<villager_poi::VillagerPoiStorage>,
    /// Active raids in this world.
    pub raids: std::sync::Mutex<raid::Raids>,
    /// End Dragon fight manager (only present in `THE_END` dimension).
    pub dragon_fight: Option<std::sync::Mutex<dragon_fight::DragonFight>>,
    pub spawn_state: ArcSwap<SpawnState>,
    pub active_chunks: RwLock<FxHashSet<Vector2<i32>>>,
    active_chunk_tracker: std::sync::Mutex<ActiveChunkTracker>,
    pub forced_chunks: std::sync::Mutex<FxHashSet<Vector2<i32>>>,
    pub(crate) portal_tickets: std::sync::Mutex<portal::tickets::PortalTickets>,
    /// Block entities indexed by chunk, so ticking only visits the currently
    /// active chunks instead of scanning every loaded block entity each tick.
    pub block_entities: DashMap<Vector2<i32>, FxHashMap<BlockPos, Arc<dyn BlockEntity>>>,
    pending_block_entity_migrations: crossbeam::queue::SegQueue<Vector2<i32>>,
    /// Persistent custom data for the world (matching Bukkit's `PersistentDataHolder`)
    pub custom_data: std::sync::Mutex<NbtCompound>,
    /// Persistent custom data for block entities at specific positions
    pub custom_block_entity_data: DashMap<BlockPos, NbtCompound>,
    /// Entity tracker responsible for tracking entity visibility and sending delta/status packets to watchers.
    pub entity_tracker: entity_tracker::EntityTracker,
    /// Vanilla `Level.neighborUpdater`: queues neighbor updates instead of recursing
    /// into them, which is what fixes the order chained redstone updates resolve in.
    pub neighbor_updater: neighbor_updater::CollectingNeighborUpdater,
}

#[derive(Clone, Copy)]
pub(crate) enum BlockBreakingProgress {
    Start { stage: i32, speed: f32 },
    Update { stage: i32, speed: Option<f32> },
    Stop,
}

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl Eq for World {}

impl World {
    pub async fn get_block_state_id_async(&self, position: &BlockPos) -> BlockStateId {
        if !self.is_in_build_limit(*position) {
            return Block::AIR.default_state.id;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        self.level
            .get_or_fetch_chunk(chunk_coordinate, |chunk| {
                chunk
                    .section
                    .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
                    .unwrap_or(Block::AIR.default_state.id)
            })
            .await
    }

    pub async fn get_block_state_async(&self, position: &BlockPos) -> &'static BlockState {
        let id = self.get_block_state_id_async(position).await;
        BlockState::from_id(id)
    }

    pub async fn get_heightmap_height_async(
        &self,
        height_map: ChunkHeightmapType,
        x: i32,
        z: i32,
    ) -> i32 {
        let chunk_pos = Vector2::new(x >> 4, z >> 4);
        self.level
            .get_or_fetch_chunk(chunk_pos, |chunk| {
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(height_map, x, z, self.min_y)
            })
            .await
    }

    #[must_use]
    pub fn load(
        level: Arc<Level>,
        level_info: Arc<ArcSwap<LevelData>>,
        dimension: Dimension,
        block_registry: Arc<BlockRegistry>,
        server: Weak<Server>,
    ) -> Arc<Self> {
        // TODO
        let generation_settings = NoiseSettings::from_dimension(&dimension);

        // Load portal POI from disk (PoiStorage::new automatically loads from disk if files exist)
        let portal_poi = portal::PortalPoiStorage::new(level.level_folder.poi_folder.clone());
        let dragon_fight = (dimension.minecraft_name == Dimension::THE_END.minecraft_name)
            .then(|| std::sync::Mutex::new(dragon_fight::DragonFight::new()));

        let custom_data_path = level
            .level_folder
            .root_folder
            .join("pumpkin_custom_data.nbt");
        let custom_data = if custom_data_path.exists()
            && let Ok(bytes) = std::fs::read(&custom_data_path)
            && let Ok(nbt) = pumpkin_nbt::Nbt::read_unnamed(
                &mut pumpkin_nbt::deserializer::NbtReadHelperJava::new(&mut std::io::Cursor::new(
                    bytes,
                )),
            ) {
            nbt.root_tag
        } else {
            NbtCompound::new()
        };

        let worldborder = Worldborder::load(&level.level_folder.dim_folder, &level_info.load());
        Arc::new_cyclic(|self_reference| Self {
            self_reference: self_reference.clone(),
            sound_random: std::sync::Mutex::new(
                pumpkin_util::random::legacy_rand::LegacyRand::from_seed(get_seed()),
            ),
            random: std::sync::Mutex::new(
                pumpkin_util::random::legacy_rand::LegacyRand::from_seed(get_seed()),
            ),
            uuid: Uuid::new_v4(),
            level,
            level_info,
            players: ArcSwap::new(Arc::new(Vec::new())),
            entities: ArcSwap::new(Arc::new(Vec::new())),
            scoreboard: std::sync::Mutex::new(Scoreboard::default()),
            handling_tick: std::sync::atomic::AtomicBool::new(false),
            worldborder: std::sync::Mutex::new(worldborder),
            level_time: std::sync::Mutex::new(LevelTime::new()),
            dimension,
            weather: std::sync::Mutex::new(Weather::new()),
            block_registry,
            sea_level: generation_settings.sea_level,
            min_y: i32::from(generation_settings.shape.min_y),
            synced_block_event_queue: std::sync::Mutex::new(std::collections::VecDeque::new()),
            unsent_block_changes: std::sync::Mutex::new(HashMap::new()),
            portal_poi: std::sync::Mutex::new(portal_poi),
            villager_poi: std::sync::Mutex::new(villager_poi::VillagerPoiStorage::default()),
            raids: std::sync::Mutex::new(raid::Raids::default()),
            dragon_fight,
            spawn_state: ArcSwap::new(Arc::new(SpawnState::empty())),
            active_chunks: RwLock::new(FxHashSet::default()),
            active_chunk_tracker: std::sync::Mutex::new(ActiveChunkTracker::default()),
            forced_chunks: std::sync::Mutex::new(FxHashSet::default()),
            portal_tickets: std::sync::Mutex::default(),
            server,
            block_entities: DashMap::new(),
            pending_block_entity_migrations: crossbeam::queue::SegQueue::new(),
            custom_data: std::sync::Mutex::new(custom_data),
            custom_block_entity_data: DashMap::new(),
            entity_tracker: entity_tracker::EntityTracker::new(),
            neighbor_updater: neighbor_updater::CollectingNeighborUpdater::default(),
        })
    }

    pub fn update_active_chunks(&self) {
        let sim_dist = self.server.upgrade().map_or(10, |s| {
            s.advanced_config.networking.java.simulation_distance.get()
        }) as i32;
        let players = self.players.load();
        let mut forced_chunks = self
            .forced_chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        forced_chunks.extend(self.portal_ticking_chunks());
        let mut tracker = self
            .active_chunk_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut active_chunks = self
            .active_chunks
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut newly_active = Vec::new();
        let mut current_players = FxHashSet::default();

        let spectators_generate_chunks =
            self.level_info.load().game_rules.spectators_generate_chunks;

        for player in players.iter() {
            if player.is_spectator() && !spectators_generate_chunks {
                continue;
            }
            let id = player.gameprofile.id;
            current_players.insert(id);
            tracker.update_player(
                id,
                ActivePlayerArea {
                    center: player.get_entity().chunk_pos.load(),
                    simulation_distance: sim_dist,
                },
                &mut active_chunks,
                &mut newly_active,
            );
        }
        let removed_players: Vec<_> = tracker
            .players
            .keys()
            .filter(|id| !current_players.contains(id))
            .copied()
            .collect();
        for id in removed_players {
            tracker.remove_player(id, &mut active_chunks);
        }
        tracker.sync_forced_chunks(&forced_chunks, &mut active_chunks, &mut newly_active);

        for pos in newly_active {
            if self.level.is_chunk_loaded(&pos) && tracker.loaded_active_chunks.insert(pos) {
                self.migrate_pending_block_entities(pos);
            }
        }
        for change in self.level.loaded_chunk_changes() {
            match change {
                pumpkin_world::level::LoadedChunkChange::Loaded(pos) => {
                    if active_chunks.contains(&pos)
                        && self.level.is_chunk_loaded(&pos)
                        && tracker.loaded_active_chunks.insert(pos)
                    {
                        self.migrate_pending_block_entities(pos);
                    }
                }
                pumpkin_world::level::LoadedChunkChange::Unloaded(pos) => {
                    if !self.level.is_chunk_loaded(&pos) {
                        tracker.loaded_active_chunks.remove(&pos);
                    }
                }
            }
        }
        let mut pending_migrations = FxHashSet::default();
        while let Some(pos) = self.pending_block_entity_migrations.pop() {
            pending_migrations.insert(pos);
        }
        for pos in pending_migrations {
            if active_chunks.contains(&pos) && self.level.is_chunk_loaded(&pos) {
                self.migrate_pending_block_entities(pos);
            }
        }
        let spawnable_chunks = natural_spawner::natural_spawn_chunk_count(
            self.players
                .load()
                .iter()
                .filter(|player| player.gamemode.load() != pumpkin_util::GameMode::Spectator)
                .map(|player| player.position().to_block_pos().chunk_position()),
        );
        drop(active_chunks);
        drop(tracker);

        self.spawn_state.store(Arc::new(SpawnState::new(
            spawnable_chunks,
            &self.entities,
            self,
        )));
    }

    pub fn get_lighting_config(&self) -> LightingEngineConfig {
        self.server
            .upgrade()
            .map(|s| s.advanced_config.world.lighting)
            .unwrap_or_default()
    }

    /// Get the world folder name (e.g., `world`, `world_nether`, `world_the_end`).
    /// Falls back to "world" if the name cannot be determined.
    pub fn get_world_name(&self) -> &str {
        self.level
            .level_folder
            .root_folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("world")
    }

    /// Returns the configured shared world spawn block position and rotation.
    #[must_use]
    pub fn get_spawn_location(&self) -> (BlockPos, f32, f32) {
        let level_info = self.level_info.load();
        (
            BlockPos::new(level_info.spawn_x, level_info.spawn_y, level_info.spawn_z),
            level_info.spawn_yaw,
            level_info.spawn_pitch,
        )
    }

    #[must_use]
    pub fn is_in_spawn_protection(&self, player: &Player, position: &BlockPos) -> bool {
        if player.permission_lvl.load() == pumpkin_util::permission::PermissionLvl::Four {
            return false;
        }

        let Some(server) = self.server.upgrade() else {
            return false;
        };

        let radius = server.basic_config.spawn_protection;
        if radius == 0 {
            return false;
        }

        let radius = i32::try_from(radius).unwrap_or(i32::MAX);
        let spawn = self.get_spawn_location().0;
        let dx = (spawn.0.x - position.0.x).abs();
        let dz = (spawn.0.z - position.0.z).abs();

        dx <= radius && dz <= radius
    }

    pub async fn shutdown(&self) {
        for entity in self.entities.load().iter() {
            self.save_entity(entity).await;
        }

        let chunks: Vec<Vector2<i32>> = self
            .block_entities
            .iter()
            .map(|chunk_block_entities| *chunk_block_entities.key())
            .collect();
        for chunk_pos in chunks {
            self.save_block_entities(chunk_pos);
        }

        // Save portal POI to disk
        let save_result = self
            .portal_poi
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_all();
        if let Err(e) = save_result {
            error!("Failed to save portal POI: {e}");
        }

        self.level.shutdown().await;
    }

    /// Serializes a live entity into its current chunk's entity data. The live
    /// entity list is the source of truth while a chunk is loaded (its saved NBT
    /// is consumed on load), so this simply appends the entity to the chunk it is
    /// currently in; the chunk is rewritten from scratch every unload cycle, so
    /// there is nothing stale to deduplicate.
    async fn save_entity(&self, entity: &Arc<dyn EntityBase>) {
        let base_entity = entity.get_entity();
        if base_entity.is_removed() {
            return;
        }
        let current_chunk = base_entity.block_pos.load().chunk_position();
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt);
        let chunk = self.level.get_entity_chunk(current_chunk).await;
        chunk
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(nbt);
        chunk.mark_dirty(true);
    }

    /// Serializes the live block entities of a chunk back into that chunk's block
    /// entity data. The live map is the source of truth while a chunk is loaded -
    /// `get_block_entity` takes the saved NBT out of the chunk when it wakes an
    /// entity up - so this has to run before the chunk is dropped, or everything
    /// the entity did since it was loaded is lost.
    fn save_block_entities(&self, chunk_pos: Vector2<i32>) {
        let Some(block_entities) = self
            .block_entities
            .get(&chunk_pos)
            .map(|chunk_block_entities| chunk_block_entities.values().cloned().collect::<Vec<_>>())
        else {
            return;
        };

        for block_entity in block_entities {
            let mut nbt = NbtCompound::new();
            block_entity.write_internal(&mut nbt);
            if let Some(custom_data) = self
                .custom_block_entity_data
                .get(&block_entity.get_position())
                && !custom_data.is_empty()
            {
                nbt.put_compound("PumpkinCustomData", custom_data.clone());
            }
            self.add_block_entity_nbt(block_entity.get_position(), &nbt);
        }
    }

    /// Broadcasts an entity status update / event to all players tracking the specified entity,
    /// and to the entity itself if it is a player.
    /// Matching Vanilla's `ServerLevel.broadcastEntityEvent(entity, event)`.
    pub fn broadcast_entity_event(
        &self,
        entity: &Entity,
        java_status: EntityStatus,
        bedrock_status: Option<ActorEventID>,
    ) {
        let je_packet = CEntityStatus::new(entity.entity_id, java_status as i8);
        if let Some(be_event) = bedrock_status {
            let be_packet = SActorEvent {
                target_runtime_id: VarULong(entity.entity_id as u64),
                event_id: be_event,
                data: VarInt(0),
                fire_at_position: None,
            };
            self.send_to_tracking_players_and_self_editioned(entity, &je_packet, &be_packet);
        } else {
            self.send_to_tracking_players_and_self(entity, &je_packet);
        }
    }

    /// Broadcasts a damage event to all players tracking the specified entity,
    /// and to the entity itself if it is a player.
    /// Matching Vanilla's `ServerLevel.broadcastDamageEvent(entity, source)`.
    pub fn broadcast_damage_event(
        &self,
        entity: &Entity,
        damage_type_id: i32,
        source_entity_id: Option<i32>,
        cause_entity_id: Option<i32>,
        position: Option<Vector3<f64>>,
    ) {
        let je_packet = CDamageEvent::new(
            entity.entity_id.into(),
            damage_type_id.into(),
            source_entity_id.map(Into::into),
            cause_entity_id.map(Into::into),
            position,
        );
        self.send_to_tracking_players_and_self(entity, &je_packet);
    }

    /// Sends an entity status update to all players tracking the specified entity.
    pub fn send_entity_status(
        &self,
        entity: &Entity,
        java_status: EntityStatus,
        bedrock_status: Option<ActorEventID>,
    ) {
        self.broadcast_entity_event(entity, java_status, bedrock_status);
    }

    pub fn send_remove_mob_effect(&self, entity: &Entity, effect_type: &'static StatusEffect) {
        let je_packet =
            CRemoveMobEffect::new(entity.entity_id.into(), VarInt(i32::from(effect_type.id)));

        let be_packet = pumpkin_protocol::bedrock::client::CMobEffect {
            target_runtime_id: VarULong(entity.entity_id as u64),
            event_id: pumpkin_protocol::bedrock::client::CMobEffect::EVENT_REMOVE,
            effect_id: VarInt(effect_type.to_bedrock_id()),
            effect_amplifier: VarInt(0),
            show_particles: false,
            effect_duration_ticks: VarInt(0),
            tick: VarULong(0),
            ambient: false,
        };
        self.send_to_tracking_players_and_self_editioned(entity, &je_packet, &be_packet);
    }

    pub fn send_add_mob_effect(&self, entity: &Entity, effect: &pumpkin_data::potion::Effect) {
        let mut flags: i8 = 0;
        if effect.ambient {
            flags |= 0x01;
        }
        if effect.show_particles {
            flags |= 0x02;
        }
        if effect.show_icon {
            flags |= 0x04;
        }

        let je_packet = CUpdateMobEffect::new(
            VarInt(entity.entity_id),
            VarInt(i32::from(effect.effect_type.id)),
            VarInt(i32::from(effect.amplifier)),
            VarInt(effect.duration),
            flags,
        );

        let be_packet = pumpkin_protocol::bedrock::client::CMobEffect {
            target_runtime_id: VarULong(entity.entity_id as u64),
            event_id: pumpkin_protocol::bedrock::client::CMobEffect::EVENT_ADD,
            effect_id: VarInt(effect.effect_type.to_bedrock_id()),
            effect_amplifier: VarInt(i32::from(effect.amplifier)),
            show_particles: effect.show_particles,
            effect_duration_ticks: VarInt(effect.duration),
            tick: VarULong(0),
            ambient: effect.ambient,
        };

        self.send_to_tracking_players_and_self_editioned(entity, &je_packet, &be_packet);
    }

    pub fn send_to_tracking_players<P: ClientPacket + Sync>(&self, entity: &Entity, packet: &P) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players(packet, self);
        }
    }

    pub fn send_to_tracking_players_bedrock<P: BClientPacket + Sync>(
        &self,
        entity: &Entity,
        packet: &P,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_bedrock(packet, self);
        }
    }

    pub fn send_to_tracking_players_editioned<J: ClientPacket + Sync, B: BClientPacket + Sync>(
        &self,
        entity: &Entity,
        je_packet: &J,
        be_packet: &B,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_editioned(je_packet, be_packet, self);
        }
    }

    pub fn send_to_tracking_players_and_self<P: ClientPacket + Sync>(
        &self,
        entity: &Entity,
        packet: &P,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_and_self(packet, self);
        }
    }

    pub fn send_to_tracking_players_and_self_editioned<
        J: ClientPacket + Sync,
        B: BClientPacket + Sync,
    >(
        &self,
        entity: &Entity,
        je_packet: &J,
        be_packet: &B,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_and_self_editioned(je_packet, be_packet, self);
        }
    }

    pub fn send_to_tracking_players_filtered<P: ClientPacket + Sync, F: Fn(&Player) -> bool>(
        &self,
        entity: &Entity,
        packet: &P,
        filter: F,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_filtered(packet, self, filter);
        }
    }

    pub fn send_to_tracking_players_filtered_editioned<
        J: ClientPacket + Sync,
        B: BClientPacket + Sync,
        F: Fn(&Player) -> bool,
    >(
        &self,
        entity: &Entity,
        je_packet: &J,
        be_packet: &B,
        filter: F,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_filtered_editioned(je_packet, be_packet, self, filter);
        }
    }

    #[must_use]
    pub fn is_tracked_by_any_player(&self, entity: &Entity) -> bool {
        self.entity_tracker
            .is_tracked_by_any_player(entity.entity_id)
    }

    pub fn set_difficulty(&self, difficulty: Difficulty) {
        let current_info = self.level_info.load();
        let mut new_info = (**current_info).clone();
        new_info.difficulty = difficulty;
        self.level_info.store(Arc::new(new_info));
    }

    pub fn get_game_rule(&self, rule: &GameRule) -> GameRuleValue<i64, bool> {
        let level_info = self.level_info.load();
        match level_info.game_rules.get(rule) {
            GameRuleValue::Int(v) => GameRuleValue::Int(*v),
            GameRuleValue::Bool(v) => GameRuleValue::Bool(*v),
        }
    }

    pub fn set_game_rule(&self, rule: &GameRule, value: GameRuleValue<i64, bool>) {
        let current_info = self.level_info.load();
        let mut new_info = (**current_info).clone();
        match (new_info.game_rules.get_mut(rule), value) {
            (GameRuleValue::Int(target), GameRuleValue::Int(val)) => {
                *target = val;
            }
            (GameRuleValue::Bool(target), GameRuleValue::Bool(val)) => {
                *target = val;
            }
            _ => {}
        }
        self.level_info.store(Arc::new(new_info));
    }

    pub fn add_synced_block_event(&self, pos: BlockPos, r#type: u8, data: u8) {
        let mut queue = self
            .synced_block_event_queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.push_back(BlockEvent { pos, r#type, data });
    }

    pub fn flush_synced_block_events(self: &Arc<Self>) {
        // Vanilla `ServerLevel.runBlockEvents` (`ServerLevel.java:1258`) drains with
        // `while (!this.blockEvents.isEmpty()) { removeFirst() }`, so an event enqueued
        // by a handler runs in the *same* tick. Taking one snapshot and iterating it
        // instead deferred nested events by a tick, which pistons notice: extending a
        // piston triggers further block events, and vanilla resolves that chain before
        // the tick ends.
        //
        // The lock is only ever held to pop a single event, never across a handler --
        // handlers re-enter through `add_synced_block_event`, and holding it there would
        // deadlock the non-reentrant Mutex.
        let mut processed: u32 = 0;
        loop {
            let Some(event) = ({
                let mut queue = self
                    .synced_block_event_queue
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                queue.pop_front()
            }) else {
                break;
            };

            // Not a vanilla rule: vanilla's drain is unbounded and relies on block logic
            // terminating. A Pumpkin-side bug that re-enqueues forever would hang the
            // server outright, so bound it and report instead.
            processed += 1;
            if processed > MAX_SYNCED_BLOCK_EVENTS_PER_TICK {
                error!(
                    "Synced block event chain exceeded {MAX_SYNCED_BLOCK_EVENTS_PER_TICK} events in one tick; dropping the rest. Last position: {}, {}, {}",
                    event.pos.0.x, event.pos.0.y, event.pos.0.z
                );
                let mut queue = self
                    .synced_block_event_queue
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                queue.clear();
                break;
            }

            let block = self.get_block(&event.pos);
            if !self.block_registry.on_synced_block_event(
                block,
                self,
                &event.pos,
                event.r#type,
                event.data,
            ) {
                continue;
            }
            let chunk_pos = event.pos.chunk_position();
            self.broadcast_to_chunk_editioned(
                chunk_pos,
                &CBlockEvent::new(
                    event.pos,
                    event.r#type,
                    event.data,
                    VarInt(block.id.as_u16() as i32),
                ),
                &CBedrockBlockEvent {
                    block_position: event.pos,
                    event_type: event.r#type.into(),
                    event_value: event.data.into(),
                },
            );
        }
    }

    pub(crate) fn collect_java_recipients_by_version<'a>(
        players: impl Iterator<Item = &'a Arc<Player>>,
    ) -> BTreeMap<JavaMinecraftVersion, Vec<&'a JavaClient>> {
        let mut recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&'a JavaClient>> =
            BTreeMap::new();
        for player in players {
            if let ClientPlatform::Java(java_client) = player.client.as_ref() {
                recipients_by_version
                    .entry(java_client.version.load())
                    .or_default()
                    .push(java_client);
            }
        }
        recipients_by_version
    }

    pub fn broadcast_java_clients<'a, P: ClientPacket>(
        packet: &P,
        recipients: impl Iterator<Item = &'a JavaClient>,
    ) {
        let mut recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&JavaClient>> =
            BTreeMap::new();
        for client in recipients {
            recipients_by_version
                .entry(client.version.load())
                .or_default()
                .push(client);
        }
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    fn broadcast_java_grouped<P: ClientPacket>(
        packet: &P,
        recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&JavaClient>>,
    ) {
        for (version, recipients) in recipients_by_version {
            let packet_data = match JavaClient::serialize_packet_for_version(packet, version) {
                Ok(packet_data) => packet_data,
                Err(pumpkin_protocol::ser::WritingError::UnsupportedVersion(_)) => {
                    continue;
                }
                Err(err) => {
                    error!(
                        "Failed to serialize packet {} for version {:?}: {}",
                        std::any::type_name::<P>(),
                        version,
                        err
                    );
                    continue;
                }
            };

            for recipient in recipients {
                recipient.try_enqueue_packet(packet_data.clone());
            }
        }
    }

    fn broadcast_bedrock_grouped<'a, P: BClientPacket>(
        packet: &P,
        recipients: impl Iterator<Item = &'a Arc<BedrockClient>>,
    ) {
        for recipient in recipients {
            match recipient.serialize_packet(packet) {
                Ok(packet_data) => recipient.try_enqueue_packet(packet_data),
                Err(err) => {
                    error!(
                        "Failed to serialize bedrock packet {}: {}",
                        std::any::type_name::<P>(),
                        err
                    );
                }
            }
        }
    }

    /// Broadcasts a packet to all connected players within the world.
    /// Please avoid this as we want to replace it with `broadcast_editioned`
    ///
    /// Sends the specified packet to every player currently logged in to the world.
    ///
    /// **Note:** This function acquires a lock on the `current_players` map, ensuring thread safety.
    pub fn broadcast_packet_all<P: ClientPacket>(&self, packet: &P) {
        let players = self.players.load();
        let recipients_by_version = Self::collect_java_recipients_by_version(players.iter());
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn broadcast_system_message(&self, message: &TextComponent, overlay: bool) {
        let je_packet = CSystemChatMessage::new(message, overlay);
        let be_packet = Self::component_to_bedrock_text(message);
        self.broadcast_editioned(&je_packet, &be_packet);
    }

    fn component_to_bedrock_text(message: &TextComponent) -> SText<'static> {
        match &*message.0.content {
            pumpkin_util::text::TextContent::Translate {
                translate,
                bedrock_translate,
                with,
            } => {
                let key = bedrock_translate.as_deref().unwrap_or(translate.as_ref());
                let parameters = with
                    .iter()
                    .map(pumpkin_util::text::TextComponentBase::to_bedrock_string)
                    .collect();
                SText::translation(key.to_string(), parameters)
            }
            _ => SText::system_message(
                message
                    .0
                    .to_bedrock_legacy(pumpkin_util::translation::Locale::EnUs),
            ),
        }
    }

    pub fn broadcast_message(
        &self,
        message: &TextComponent,
        sender_name: &TextComponent,
        chat_type: u8,
        target_name: Option<&TextComponent>,
    ) {
        let be_packet = SText::new(message.clone().get_text(), sender_name.clone().get_text());
        let je_packet =
            CDisguisedChatMessage::new(message, (chat_type + 1).into(), sender_name, target_name);

        self.broadcast_editioned(&je_packet, &be_packet);
    }

    // This should replace broadcast_packet_all at some point
    pub fn broadcast_editioned<J: ClientPacket, B: BClientPacket>(
        &self,
        je_packet: &J,
        be_packet: &B,
    ) {
        let players = self.players.load();
        let je_recipients_by_version = Self::collect_java_recipients_by_version(players.iter());

        Self::broadcast_java_grouped(je_packet, je_recipients_by_version);
        Self::broadcast_bedrock_grouped(
            be_packet,
            players.iter().filter_map(|p| match p.client.as_ref() {
                ClientPlatform::Bedrock(be) => Some(be),
                ClientPlatform::Java(_) => None,
            }),
        );
    }

    pub fn broadcast_chat_message(
        &self,
        message: &crate::net::chat::PlayerChatMessage,
        is_filtered: impl Fn(&Player) -> bool,
        sender_player: Option<&Arc<Player>>,
        chat_type: VarInt,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        let tracked = crate::net::chat::OutgoingChatMessage::create(message.clone());
        let mut was_fully_filtered = false;

        let players = self.players.load();
        for player in players.iter() {
            let filtered = is_filtered(player);
            tracked.send_to_player(player, filtered, chat_type, sender_name, target_name);
            was_fully_filtered |= filtered && message.is_fully_filtered();
        }

        if was_fully_filtered && let Some(sender) = sender_player {
            let filter_notice =
                TextComponent::translate(pumpkin_data::translation::java::CHAT_FILTERED_FULL, [])
                    .color_named(pumpkin_util::text::color::NamedColor::Red)
                    .italic();
            sender.send_system_message(&filter_notice);
        }
    }

    pub fn broadcast_secure_player_chat(
        &self,
        sender: &Arc<Player>,
        chat_message: &SChatMessage<'_>,
        decorated_message: &TextComponent,
    ) {
        let messages_sent: i32 = sender
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages_sent;
        let sender_last_seen = {
            let cache = sender
                .signature_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.last_seen.as_ref().to_vec()
        };

        let link = crate::net::chat::SignedMessageLink::new(
            messages_sent,
            sender.gameprofile.id,
            Uuid::nil(),
        );
        let signed_body = crate::net::chat::SignedMessageBody::new(
            chat_message.message.to_string(),
            chat_message.timestamp,
            chat_message.salt,
            sender_last_seen,
        );
        let player_chat_msg = crate::net::chat::PlayerChatMessage::new(
            link,
            chat_message.signature.map(std::convert::Into::into),
            signed_body,
            Some(decorated_message.clone()),
            crate::net::chat::FilterMask::PassThrough,
        );

        self.broadcast_chat_message(
            &player_chat_msg,
            Player::is_text_filtering_enabled,
            Some(sender),
            (RAW + 1).into(),
            &TextComponent::empty(),
            None,
        );

        sender
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages_sent += 1;
    }

    pub fn broadcast_packet_except_editioned<J: ClientPacket, B: BClientPacket>(
        &self,
        except: &[uuid::Uuid],
        je_packet: &J,
        be_packet: &B,
    ) {
        let players = self.players.load();
        let mut java_recipients = Vec::new();
        let mut bedrock_recipients = Vec::new();

        for p in players.iter() {
            if except.contains(&p.gameprofile.id) {
                continue;
            }
            match p.client.as_ref() {
                ClientPlatform::Java(_) => java_recipients.push(p),
                ClientPlatform::Bedrock(be_client) => bedrock_recipients.push(be_client),
            }
        }

        let recipients_by_version =
            Self::collect_java_recipients_by_version(java_recipients.into_iter());
        Self::broadcast_java_grouped(je_packet, recipients_by_version);
        Self::broadcast_bedrock_grouped(be_packet, bedrock_recipients.into_iter());
    }

    /// Broadcasts the skin layers of a player, encoding the metadata for each Java client's own
    /// protocol version since the tracked data index differs between versions.
    fn broadcast_skin_parts<B: BClientPacket>(
        &self,
        except: &[uuid::Uuid],
        entity_id: i32,
        skin_parts: u8,
        be_packet: &B,
    ) {
        let players = self.players.load();
        let mut java_recipients = Vec::new();
        let mut bedrock_recipients = Vec::new();

        for p in players.iter() {
            if except.contains(&p.gameprofile.id) {
                continue;
            }
            match p.client.as_ref() {
                ClientPlatform::Java(_) => java_recipients.push(p),
                ClientPlatform::Bedrock(be_client) => bedrock_recipients.push(be_client),
            }
        }

        let recipients_by_version =
            Self::collect_java_recipients_by_version(java_recipients.into_iter());

        for (version, recipients) in recipients_by_version {
            if version < JavaMinecraftVersion::V_1_21 {
                continue;
            }
            let mut buf = Vec::new();
            for meta in [Metadata::new(
                pumpkin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
                skin_parts,
            )] {
                let _ = meta.write(&mut buf, &version);
            }
            buf.put_u8(255);
            let packet = CSetEntityMetadata::new(entity_id.into(), buf.into());
            if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version) {
                for recipient in recipients {
                    recipient.try_enqueue_packet(packet_data.clone());
                }
            }
        }

        Self::broadcast_bedrock_grouped(be_packet, bedrock_recipients.into_iter());
    }

    /// Broadcasts a packet to all connected players within the world, excluding the specified players.
    ///
    /// Sends the specified packet to every player currently logged in to the world, excluding the players listed in the `except` parameter.
    ///
    /// **Note:** This function acquires a lock on the `current_players` map, ensuring thread safety.
    pub fn broadcast_packet_except<P: ClientPacket>(&self, except: &[uuid::Uuid], packet: &P) {
        let players = self.players.load();
        let recipients_by_version = Self::collect_java_recipients_by_version(
            players
                .iter()
                .filter(|candidate| !except.contains(&candidate.gameprofile.id)),
        );
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn spawn_particle(
        &self,
        position: Vector3<f64>,
        offset: Vector3<f32>,
        max_speed: f32,
        particle_count: i32,
        particle: Particle,
    ) {
        let packet = CParticle::new(
            false,
            false,
            position,
            offset,
            max_speed,
            particle_count,
            VarInt(particle as i32),
            &[],
        );
        self.broadcast_particle(&packet);
    }

    /// ServerLevel.sendParticles range and BlockParticleOption payload.
    pub(crate) fn spawn_block_particles(
        &self,
        state: BlockStateId,
        position: Vector3<f64>,
        count: i32,
        offset: Vector3<f32>,
    ) {
        use pumpkin_protocol::ser::NetworkWriteExt;
        let mut data = Vec::new();
        if data
            .write_var_int(&VarInt(i32::from(state.as_u16())))
            .is_err()
        {
            return;
        }
        let packet = CParticle::new(
            false,
            false,
            position,
            offset,
            0.15,
            count,
            VarInt(Particle::Block as i32),
            &data,
        );
        self.broadcast_particle(&packet);
    }

    fn broadcast_particle(&self, packet: &CParticle<'_>) {
        let range = if packet.important { 512.0 } else { 32.0 };
        let players = self.players.load();
        let recipients =
            Self::collect_java_recipients_by_version(players.iter().filter(|player| {
                let center = player
                    .get_entity()
                    .block_pos
                    .load()
                    .to_f64()
                    .add_raw(0.5, 0.5, 0.5);
                (center - packet.position).length_squared() < range * range
            }));
        Self::broadcast_java_grouped(packet, recipients);
    }

    fn next_sound_seed(&self) -> i64 {
        self.sound_random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_i64()
    }

    pub fn play_sound(&self, sound: Sound, category: SoundCategory, position: &Vector3<f64>) {
        self.play_sound_raw(sound as u16, category, position, 1.0, 1.0);
    }

    pub fn play_sound_event(
        &self,
        sound: &pumpkin_data::data_component_impl::IdOr<
            pumpkin_data::data_component_impl::SoundEvent,
        >,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        self.play_sound_event_fine(sound, category, position, 1.0, 1.0);
    }

    pub fn play_sound_event_fine(
        &self,
        sound: &pumpkin_data::data_component_impl::IdOr<
            pumpkin_data::data_component_impl::SoundEvent,
        >,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let packet = CSoundEffect::new(
            data_to_proto_sound(sound),
            category,
            position,
            volume,
            pitch,
            self.next_sound_seed(),
        );
        self.broadcast_sound(&packet, *position, None);
    }

    pub fn play_sound_event_expect(
        &self,
        player: &Player,
        sound: &pumpkin_data::data_component_impl::IdOr<
            pumpkin_data::data_component_impl::SoundEvent,
        >,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        let seed = self.next_sound_seed();
        let packet = CSoundEffect::new(
            data_to_proto_sound(sound),
            category,
            position,
            1.0,
            1.0,
            seed,
        );
        self.broadcast_sound(&packet, *position, Some(player.gameprofile.id));
    }

    pub fn play_sound_fine(
        &self,
        sound: Sound,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        self.play_sound_raw(sound as u16, category, position, volume, pitch);
    }

    /// Plays a custom sound event by identifier for all players in range.
    pub fn play_custom_sound(
        &self,
        sound_name: &str,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = self.next_sound_seed();
        let packet = CSoundEffect::new(
            pumpkin_protocol::IdOr::Value(pumpkin_protocol::SoundEvent {
                sound_name: sound_name.into(),
                range: None,
            }),
            category,
            position,
            volume,
            pitch,
            seed,
        );
        self.broadcast_sound(&packet, *position, None);
    }

    /// Spawns a cluster of particles in the world for all players in range.
    pub fn spawn_particles(
        &self,
        particle: pumpkin_data::particle::Particle,
        pos: Vector3<f64>,
        count: u32,
        offset: Vector3<f32>,
        max_speed: f32,
    ) {
        let packet = CParticle::new(
            false,
            false,
            pos,
            offset,
            max_speed,
            count as i32,
            (particle.to_id() as i32).into(),
            &[],
        );
        self.broadcast_particle(&packet);
    }

    /// Plays a Bedrock level sound for players close enough to hear it.
    pub fn play_bedrock_level_sound(
        &self,
        sound_id: &str,
        position: &Vector3<f64>,
        extra_data: i32,
    ) {
        let packet = CLevelSoundEvent {
            sound_event: sound_id.to_string(),
            position: Vector3::new(position.x as f32, position.y as f32, position.z as f32),
            data: VarInt(extra_data),
            actor_identifier: String::new(),
            is_baby: false,
            is_global: false,
            actor_unique_id: 0,
            fire_at_position: None,
        };
        let chunk_pos = BlockPos::floored_v(*position).chunk_position();

        for player in self.players.load().iter() {
            if is_within_chebyshev_distance(chunk_pos, player.get_entity().chunk_pos.load(), 1)
                && let ClientPlatform::Bedrock(client) = player.client.as_ref()
                && let Ok(data) = client.serialize_packet(&packet)
            {
                client.try_enqueue_packet(data);
            }
        }
    }

    pub fn play_sound_expect(
        &self,
        player: &Player,
        sound: Sound,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        self.play_sound_raw_expect(player, sound as u16, category, position, 1.0, 1.0);
    }

    pub fn play_sound_raw(
        &self,
        sound_id: u16,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = self.next_sound_seed();
        let packet = CSoundEffect::new(IdOr::Id(sound_id), category, position, volume, pitch, seed);
        self.broadcast_sound(&packet, *position, None);
    }

    pub fn play_sound_raw_expect(
        &self,
        player: &Player,
        sound_id: u16,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = self.next_sound_seed();
        let packet = CSoundEffect::new(IdOr::Id(sound_id), category, position, volume, pitch, seed);
        self.broadcast_sound(&packet, *position, Some(player.gameprofile.id));
    }

    pub fn play_block_sound(&self, sound: Sound, category: SoundCategory, position: BlockPos) {
        let new_vec = Vector3::new(
            f64::from(position.0.x) + 0.5,
            f64::from(position.0.y) + 0.5,
            f64::from(position.0.z) + 0.5,
        );
        self.play_sound(sound, category, &new_vec);
    }

    pub fn play_block_sound_expect(
        &self,
        player: &Player,
        sound: Sound,
        category: SoundCategory,
        position: BlockPos,
    ) {
        let new_vec = Vector3::new(
            f64::from(position.0.x) + 0.5,
            f64::from(position.0.y) + 0.5,
            f64::from(position.0.z) + 0.5,
        );
        self.play_sound_expect(player, sound, category, &new_vec);
    }

    #[expect(clippy::too_many_lines)]
    // --- Level random stream (vanilla `Level.random`) ---------------------------
    //
    // Vanilla threads a single `RandomSource` through the level and passes it into
    // `randomTick(level, pos, random)`. These wrappers give block code the same stream
    // without handing out a lock guard: each draw locks and unlocks internally, so no
    // guard can escape into a caller expression and deadlock the non-reentrant Mutex.
    //
    // Match vanilla's call exactly -- `nextInt(n)` is `rand_bounded_i32(n)`, not a
    // range helper -- because the number and kind of draws is part of the contract.

    /// Vanilla `random.nextInt(bound)`.
    pub fn rand_bounded_i32(&self, bound: i32) -> i32 {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_bounded_i32(bound)
    }

    /// Vanilla `random.nextInt()`.
    pub fn rand_i32(&self) -> i32 {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_i32()
    }

    /// Recover the owning Arc for callbacks that need to construct entities.
    pub(crate) fn as_arc(&self) -> Option<Arc<Self>> {
        self.self_reference.upgrade()
    }

    /// Vanilla `random.nextLong()` on the same level random stream.
    pub fn rand_i64(&self) -> i64 {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_i64()
    }

    /// Vanilla `random.nextFloat()`.
    pub fn rand_f32(&self) -> f32 {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f32()
    }

    /// Vanilla `random.nextDouble()`.
    pub fn rand_f64(&self) -> f64 {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f64()
    }

    /// Vanilla `random.nextBoolean()`.
    pub fn rand_bool(&self) -> bool {
        use pumpkin_util::random::RandomImpl;
        self.random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_bool()
    }

    pub fn tick(self: &Arc<Self>, server: &Arc<Server>) {
        let start = std::time::Instant::now();

        // Vanilla sets `handlingTick` at the top of `ServerLevel.tick` (`:355`) and
        // clears it immediately after `runBlockEvents` (`:407`).
        self.handling_tick
            .store(true, std::sync::atomic::Ordering::Relaxed);

        self.flush_block_updates();
        self.update_active_chunks();

        // 1. Environment tick: weather, sleeping check, sky brightness, game time
        // Reference: Vanilla Java 26.2 `ServerLevel.java:359-383`
        self.worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tick();
        self.tick_environment();

        // 2. Pending scheduled ticks: blockTicks, then fluidTicks
        // Reference: Vanilla Java 26.2 `ServerLevel.java:385-393` (`tickPending`)
        self.tick_scheduled_ticks(server);

        // 3. Raids tick
        // Reference: Vanilla Java 26.2 `ServerLevel.java:395-398` (`raid`)
        let mut raids = {
            let mut guard = self
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        raids.tick(self);
        {
            let mut guard = self
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (id, raid) in guard.raid_map.drain() {
                raids.raid_map.insert(id, raid);
            }
            raids.next_id = raids.next_id.max(guard.next_id);
            *guard = raids;
        };

        // 4. Chunk ticking: random ticks, chunk spawning, inhabited time
        // Reference: Vanilla Java 26.2 `ServerLevel.java:400-401` (`chunkSource`)
        let t_chunks = std::time::Instant::now();
        self.tick_chunks(server);
        let chunk_elapsed = t_chunks.elapsed();

        // 5. Block events (synced block events, e.g. pistons triggered by redstone/scheduled ticks)
        // Reference: Vanilla Java 26.2 `ServerLevel.java:402-405` (`blockEvents` / `runBlockEvents`)
        self.flush_synced_block_events();

        // Vanilla clears `handlingTick` here (`ServerLevel.java:407`), immediately after
        // block events and before entities and block entities tick.
        self.handling_tick
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // 6. Broadcast chunk changes resulting from chunk ticks & block events
        // Reference: Vanilla Java 26.2 `ServerChunkCache.java:350-367` (`broadcastChangedChunks`)
        self.flush_block_updates();
        self.level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send_change();

        // 7. Dragon fight tick (runs before entities tick)
        // Reference: Vanilla Java 26.2 `ServerLevel.java:420-424` (`dragonFight`)
        if let Some(ref fight_mutex) = self.dragon_fight {
            dragon_fight::DragonFight::tick(fight_mutex, self);
        }

        let handle = server.runtime.clone();

        // 8. Player ticking
        // Reference: Vanilla Java 26.2 `ServerLevel.java:434-452`
        let players = self.players.load();
        let player_count = players.len();
        let players_cache: Vec<_> = players
            .iter()
            .map(|player| {
                let entity = player.get_entity();
                let pos = entity.pos.load();
                let bb = entity.bounding_box.load().expand(1.0, 0.5, 1.0);
                let chunk_pos = Vector2::new(
                    get_section_cord(pos.x.floor() as i32),
                    get_section_cord(pos.z.floor() as i32),
                );
                (player, pos, bb, chunk_pos)
            })
            .collect();

        let t_players = std::time::Instant::now();
        let player_handle = handle.clone();
        let _player_guard = player_handle.enter();
        for player in players.iter() {
            if player.get_entity().teleporting.load(Ordering::Acquire)
                || !std::ptr::eq(player.get_entity().world.load().as_ref(), self.as_ref())
            {
                continue;
            }
            player.tick(server);
            player.get_entity().flush_pending_inside_effects(player.as_ref());
        }
        drop(_player_guard);
        let player_elapsed = t_players.elapsed();

        // 9. Entity ticking
        // Reference: Vanilla Java 26.2 `ServerLevel.java:426-452`
        let entities_to_tick = self.entities.load();
        let entity_count = entities_to_tick.len();
        let active_chunks = self
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let level_for_entities = self.level.clone();
        let entity_handle = handle.clone();

        let t_entities = std::time::Instant::now();
        let tickable: Vec<_> = entities_to_tick
            .iter()
            .filter_map(|entity| {
                let entity_pos = entity.get_entity().pos.load();
                let entity_chunk = Vector2::new(
                    get_section_cord(entity_pos.x.floor() as i32),
                    get_section_cord(entity_pos.z.floor() as i32),
                );
                if !active_chunks.contains(&entity_chunk) {
                    return None;
                }
                if !level_for_entities.is_chunk_loaded(&entity_chunk) {
                    return None;
                }
                Some((entity, entity_chunk))
            })
            .collect();

        let server_ref = server.as_ref();
        {
            let _guard = entity_handle.enter();
            {
                for (entity, entity_chunk) in &tickable {
                    if entity.get_entity().is_removed()
                        || entity.get_entity().teleporting.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    entity.get_entity().tick_count.fetch_add(1, Relaxed);
                    crate::entity::projectile::begin_tick(entity.as_ref());
                    crate::entity::projectile::emit_shoot_event(entity.as_ref());
                    entity.tick(entity.as_ref(), server_ref);
                    if entity.get_entity().is_removed() {
                        continue;
                    }
                    crate::entity::projectile::check_left_owner(entity.as_ref());
                    entity.get_entity().flush_pending_inside_effects(entity.as_ref());

                    let entity_inner = entity.get_entity();
                    let entity_pos = entity_inner.pos.load();
                    let entity_bb = entity_inner.bounding_box.load();

                    for (player, player_pos, player_bb, player_chunk) in &players_cache {
                        if (player_chunk.x - entity_chunk.x).abs() <= 1
                            && (player_chunk.y - entity_chunk.y).abs() <= 1
                            && (player_pos.x - entity_pos.x).abs() < 5.0
                            && (player_pos.y - entity_pos.y).abs() < 5.0
                            && (player_pos.z - entity_pos.z).abs() < 5.0
                            && player_bb.intersects(&entity_bb)
                        {
                            entity.on_player_collision(player);
                            break;
                        }
                    }
                }
            }
        }
        let entity_elapsed = t_entities.elapsed();

        // 10. Entity tracker & management
        // Reference: Vanilla Java 26.2 `ServerLevel.java:458-460` (`entityManager`)
        self.entity_tracker.update_all(self);

        // 11. Block entities tick & comparators
        // Reference: Vanilla Java 26.2 `ServerLevel.java:453-455` (`blockEntities`)
        let mut block_entities: Vec<Arc<dyn BlockEntity>> = Vec::new();
        {
            // Collect (chunk, position) keys first and sort them, so the tick order does
            // not depend on DashMap/FxHashSet iteration order.
            let mut keyed: Vec<(Vector2<i32>, BlockPos, Arc<dyn BlockEntity>)> = Vec::new();
            if self.block_entities.len() < active_chunks.len() {
                for chunk_block_entities in &self.block_entities {
                    let chunk_pos = *chunk_block_entities.key();
                    if active_chunks.contains(&chunk_pos) {
                        keyed.extend(
                            chunk_block_entities
                                .value()
                                .iter()
                                .map(|(pos, be)| (chunk_pos, *pos, be.clone())),
                        );
                    }
                }
            } else {
                for chunk_pos in active_chunks.iter() {
                    if let Some(chunk_block_entities) = self.block_entities.get(chunk_pos) {
                        keyed.extend(
                            chunk_block_entities
                                .value()
                                .iter()
                                .map(|(pos, be)| (*chunk_pos, *pos, be.clone())),
                        );
                    }
                }
            }
            keyed.sort_unstable_by_key(|(chunk_pos, pos, _)| {
                (chunk_pos.x, chunk_pos.y, pos.0.x, pos.0.y, pos.0.z)
            });
            block_entities.extend(keyed.into_iter().map(|(_, _, be)| be));
        }
        drop(active_chunks);
        let block_entity_count = block_entities.len();

        let t_be = std::time::Instant::now();
        let be_handle = handle;
        {
            let _guard = be_handle.enter();
            for be in &block_entities {
                be.tick(self);
            }
        }
        // Drained after all ticks, so changes (hopper -> chest) land in the same tick.
        let guard = be_handle.enter();
        self.flush_comparator_updates(&block_entities);
        drop(guard);
        let block_entity_elapsed = t_be.elapsed();

        // Final broadcast of any updates caused by entities or block entities
        self.flush_block_updates();
        self.level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send_change();

        let total_elapsed = start.elapsed();
        if total_elapsed.as_millis() > 50 {
            debug!(
                "Slow Tick [{}ms]: Chunks: {:?} | Players({}): {:?} | Entities({}): {:?} | Block Entities({}): {:?}",
                total_elapsed.as_millis(),
                chunk_elapsed,
                player_count,
                player_elapsed,
                entity_count,
                entity_elapsed,
                block_entity_count,
                block_entity_elapsed,
            );
        }
    }

    pub fn register_block_change(&self, position: BlockPos, block_state_id: BlockStateId) {
        self.unsent_block_changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(position, block_state_id);
    }

    /// Queues block state changes for broadcast to nearby players.
    ///
    /// Call [`flush_block_updates`](Self::flush_block_updates) afterward to send the packets.
    pub fn queue_block_updates(&self, changes: &[(BlockPos, BlockStateId)]) {
        let mut guard = self
            .unsent_block_changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pos, state_id) in changes {
            guard.insert(*pos, *state_id);
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn flush_block_updates(&self) {
        let mut block_state_updates_by_chunk_section: HashMap<
            Vector3<i32>,
            Vec<(BlockPos, BlockStateId)>,
        > = HashMap::new();
        let changes = {
            let mut guard = self
                .unsent_block_changes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        for (position, block_state_id) in changes {
            let chunk_section = chunk_section_from_pos(&position);
            block_state_updates_by_chunk_section
                .entry(chunk_section)
                .or_default()
                .push((position, block_state_id));
        }

        // TODO: only send packet to players who have the chunks loaded
        // TODO: Send light updates to update the wire directly next to a broken block
        for (chunk_section, updates) in block_state_updates_by_chunk_section {
            if updates.is_empty() {
                continue;
            }
            let chunk_pos = Vector2::new(chunk_section.x, chunk_section.z);
            if updates.len() == 1 {
                let (block_pos, block_state_id) = updates[0];
                let be_block_id = BlockState::to_be_network_id(block_state_id);
                self.broadcast_to_chunk_editioned(
                    chunk_pos,
                    &CBlockUpdate::new(block_pos, i32::from(block_state_id.as_u16()).into()),
                    &pumpkin_protocol::bedrock::client::CUpdateBlock::new(
                        block_pos,
                        be_block_id as u32,
                    ),
                );
                if let Some(block_entity) = self.get_block_entity(&block_pos)
                    && let Some(nbt) = block_entity.chunk_data_nbt()
                {
                    let bytes = pumpkin_nbt::Nbt::from(nbt).write_unnamed();
                    self.broadcast_to_chunk(
                        chunk_pos,
                        &CBlockEntityData::new(
                            block_pos,
                            VarInt(block_entity.get_id() as i32),
                            bytes.as_ref().into(),
                        ),
                    );
                }
                if let Some(data) = self.bedrock_block_entity_data(block_state_id, block_pos) {
                    self.broadcast_to_chunk_bedrock(
                        chunk_pos,
                        &CBlockActorData::new(block_pos, data),
                    );
                }
            } else {
                let players = self.players.load();
                let mut java_recipients = Vec::new();

                let recipients = players.iter().filter(|p| {
                    p.watched_section
                        .load()
                        .is_within_distance(chunk_pos.x, chunk_pos.y)
                });

                let mut bedrock_packets = Vec::new();
                for (block_pos, block_state_id) in &updates {
                    let be_block_id = BlockState::to_be_network_id(*block_state_id);
                    let update_packet = pumpkin_protocol::bedrock::client::CUpdateBlock::new(
                        *block_pos,
                        be_block_id as u32,
                    );
                    let actor_packet = self
                        .bedrock_block_entity_data(*block_state_id, *block_pos)
                        .map(|data| CBlockActorData::new(*block_pos, data));
                    bedrock_packets.push((update_packet, actor_packet));
                }

                let mut bedrock_recipients = Vec::new();
                for p in recipients {
                    match p.client.as_ref() {
                        ClientPlatform::Java(_) => java_recipients.push(p),
                        ClientPlatform::Bedrock(be_client) => {
                            bedrock_recipients.push(be_client);
                        }
                    }
                }

                for be_client in bedrock_recipients {
                    for (update_packet, actor_packet) in &bedrock_packets {
                        if let Ok(data) = be_client.serialize_packet(update_packet) {
                            be_client.try_enqueue_packet(data);
                        }
                        if let Some(actor_packet) = actor_packet
                            && let Ok(data) = be_client.serialize_packet(actor_packet)
                        {
                            be_client.try_enqueue_packet(data);
                        }
                    }
                }

                let recipients_by_version =
                    Self::collect_java_recipients_by_version(java_recipients.into_iter());
                Self::broadcast_java_grouped(
                    &CMultiBlockUpdate::new(&updates),
                    recipients_by_version,
                );

                for (block_pos, _) in &updates {
                    if let Some(block_entity) = self.get_block_entity(block_pos)
                        && let Some(nbt) = block_entity.chunk_data_nbt()
                    {
                        let bytes = pumpkin_nbt::Nbt::from(nbt).write_unnamed();
                        self.broadcast_to_chunk(
                            chunk_pos,
                            &CBlockEntityData::new(
                                *block_pos,
                                VarInt(block_entity.get_id() as i32),
                                bytes.as_ref().into(),
                            ),
                        );
                    }
                }
            }

            let mut bedrock_water_packets = Vec::new();
            for (block_pos, block_state_id) in &updates {
                let water_state = bedrock_water_state(*block_state_id);
                let packet = pumpkin_protocol::bedrock::client::CUpdateBlock::with_layer(
                    *block_pos,
                    u32::from(BlockState::to_be_network_id(water_state)),
                    1,
                );
                bedrock_water_packets.push(packet);
            }

            if !bedrock_water_packets.is_empty() {
                let players = self.players.load();
                let recipients = players.iter().filter(|player| {
                    player
                        .watched_section
                        .load()
                        .is_within_distance(chunk_pos.x, chunk_pos.y)
                });
                for player in recipients {
                    if let ClientPlatform::Bedrock(client) = player.client.as_ref() {
                        for packet in &bedrock_water_packets {
                            if let Ok(data) = client.serialize_packet(packet) {
                                client.try_enqueue_packet(data);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn tick_environment(self: &Arc<Self>) {
        let (world_age, is_night, time_of_day) = {
            let mut level_time = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let advance_time = self.level_info.load().game_rules.advance_time;
            level_time.tick(advance_time);

            // Auto-save logic
            if level_time.world_age % 100 == 0 {
                self.level.should_unload.store(true, Relaxed);
                let cleaned_chunks = self.level.clean_memory();
                if !cleaned_chunks.is_empty() {
                    let world_clone = self.clone();
                    if let Some(server) = self.server.upgrade() {
                        server.spawn_task(async move {
                            world_clone.remove_entities_in_chunks(&cleaned_chunks).await;
                            world_clone.level.clean_entity_chunks(&cleaned_chunks);
                        });
                    }
                }
                // If autosave is configured and this tick will trigger an autosave, don't double notify
                if self.level.autosave_ticks == 0 {
                    self.level.level_channel.notify();
                } else {
                    let autosave = self.level.autosave_ticks as i64;
                    if autosave == 0 || level_time.world_age % autosave != 0 {
                        self.level.level_channel.notify();
                    }
                }
            }
            if self.level.autosave_ticks > 0 && self.level.save_enabled.load(Relaxed) {
                let autosave = self.level.autosave_ticks as i64;
                if autosave > 0 && level_time.world_age % autosave == 0 {
                    self.save_world_border();
                    self.level.should_save.store(true, Relaxed);
                    self.level.level_channel.notify();
                }
            }
            (
                level_time.world_age,
                level_time.is_night(),
                level_time.time_of_day,
            )
        };

        let (should_reset_weather, weather_cycle_enabled) = {
            let mut weather = self
                .weather
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            weather.tick_weather(self);
            (
                weather.raining || weather.thundering,
                weather.weather_cycle_enabled,
            )
        };

        if self.should_skip_night() && is_night {
            let level_time = {
                let mut guard = self
                    .level_time
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let time = time_of_day + 24000;
                guard.set_time(time - time % 24000);
                guard.clone()
            };
            level_time.send_time(self);

            for player in self.players.load().iter() {
                player.wake_up();
            }

            if weather_cycle_enabled && should_reset_weather {
                let mut weather = self
                    .weather
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                weather.reset_weather_cycle(self);
            }
        } else if world_age % 20 == 0 {
            let level_time = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            level_time.send_time(self);
        }
    }

    /// Ticks pending scheduled block and fluid ticks.
    ///
    /// Reference: Vanilla Java 26.2 `ServerLevel.java:385-394` (`tickPending` phase).
    pub fn tick_scheduled_ticks(self: &Arc<Self>, server: &Arc<Server>) {
        let (block_ticks, fluid_ticks) = self.level.get_scheduled_ticks();
        let handle = server.runtime.clone();

        // 1. Scheduled Block Ticks
        // Java delivers scheduled block callbacks serially and checks the target
        // type at delivery time. A callback can invalidate another collected tick.
        {
            let _guard = handle.enter();
            scheduled_dispatch::dispatch(
                block_ticks
                    .into_iter()
                    .map(|tick| (tick.position, tick.value)),
                |pos| self.get_block(pos),
                |current, expected| current.id == expected.id,
                |block, pos| {
                    if let Some(pumpkin_block) = self.block_registry.get_pumpkin_block(block.id) {
                        pumpkin_block.on_scheduled_tick(OnScheduledTickArgs {
                            world: self,
                            block,
                            position: pos,
                        });
                    }
                },
            );
        }

        // 2. Fluid Ticks -- sequential, in the order level.rs sorted them.
        let fluid_handle = handle.clone();
        {
            let _guard = fluid_handle.enter();
            for scheduled_tick in &fluid_ticks {
                let pos = scheduled_tick.position;
                let fluid = self.get_fluid(&pos);
                if let Some(pumpkin_fluid) = self.block_registry.get_pumpkin_fluid(fluid.id) {
                    pumpkin_fluid.on_scheduled_tick(self, fluid, &pos);
                }
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn tick_chunks(self: &Arc<Self>, server: &Arc<Server>) {
        const INHABITED_TIME_BATCH_SIZE: usize = 1024;
        let random_tick_speed = self.level_info.load().game_rules.random_tick_speed;

        let active_chunks = self
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let random_ticks = self
            .level
            .get_random_ticks(&active_chunks, random_tick_speed);
        let handle = server.runtime.clone();

        // 1. Ice and Snow / Precipitation ticking
        // Reference: Vanilla Java 26.2 `ServerLevel.java:506-510` (`iceandsnow` phase in `tickChunk`).
        if random_tick_speed > 0 {
            let mut active_chunks_sorted: Vec<_> = active_chunks.iter().copied().collect();
            active_chunks_sorted.sort_unstable_by_key(|pos| (pos.x, pos.y));
            for chunk_pos in &active_chunks_sorted {
                let min_x = chunk_pos.x * 16;
                let min_z = chunk_pos.y * 16;
                for _ in 0..random_tick_speed {
                    if self.rand_bounded_i32(48) == 0 {
                        let random_pos = self.level.get_block_random_pos(min_x, 0, min_z, 15);
                        precipitation::tick_precipitation(self, random_pos);
                    }
                }
            }
        }

        // 2. Random Ticks -- sequential, in collection order.
        let world = self.clone();
        let random_handle = handle.clone();
        {
            let _guard = random_handle.enter();
            {
                let world = world.clone();
                for scheduled_tick in &random_ticks {
                    let pos = scheduled_tick.position;
                    let (block, fluid) =
                        match (scheduled_tick.tick_block, scheduled_tick.tick_fluid) {
                            (true, true) => {
                                let (b, f) = world.get_block_and_fluid(&pos);
                                (Some(b), Some(f))
                            }
                            (true, false) => (Some(world.get_block(&pos)), None),
                            (false, true) => (None, Some(world.get_fluid(&pos))),
                            (false, false) => (None, None),
                        };

                    if let Some(block) = block
                        && let Some(pumpkin_block) =
                            world.block_registry.get_pumpkin_block(block.id)
                    {
                        let mut random = crate::block::random::BlockRandom::Shared(&world.random);
                        pumpkin_block.random_tick(RandomTickArgs {
                            world: &world,
                            block,
                            position: &pos,
                            random: &mut random,
                        });
                    }

                    if let Some(fluid) = fluid
                        && let Some(pumpkin_fluid) =
                            world.block_registry.get_pumpkin_fluid(fluid.id)
                    {
                        let mut random = crate::block::random::BlockRandom::Shared(&world.random);
                        pumpkin_fluid.random_tick(fluid, &world, &pos, &mut random);
                    }
                }
            }
        }

        // 2. Calculate Spawn List (Sequential setup)
        let spawn_state = self.spawn_state.load();
        let (spawn_mobs, spawn_monsters, peaceful) = {
            let lock = self.level_info.load();
            (
                lock.game_rules.spawn_mobs,
                lock.game_rules.spawn_monsters,
                lock.difficulty == Difficulty::Peaceful,
            )
        };
        let spawn_passives = self.get_world_age() % 400 == 0;
        let spawn_enemies = !peaceful && spawn_monsters && spawn_mobs;
        let spawn_passives = spawn_passives && spawn_mobs;

        let spawn_list = Arc::new(natural_spawner::get_filtered_spawning_categories(
            &spawn_state,
            spawn_mobs,
            spawn_enemies,
            spawn_passives,
        ));

        // 3. Spawn decisions share mutable caps, density charges and cached checks.
        // Vanilla processes these sequentially; atomics alone cannot preserve order.
        if !spawn_list.is_empty() {
            // Sorted before shuffling: `active_chunks` is an FxHashSet whose iteration
            // order is arbitrary, and Fisher-Yates permutes whatever order it is handed.
            // Vanilla fills `spawningChunks` from an ordered source
            // (`ChunkMap.collectSpawningChunks`, `ServerChunkCache.java:390`) before
            // shuffling, so without a deterministic input the RNG draw count is right
            // but the resulting chunk order still varies between otherwise identical runs.
            let mut chunk_positions: Vec<_> = active_chunks.iter().copied().collect();
            chunk_positions.sort_unstable_by_key(|pos| (pos.x, pos.y));

            let mut spawning_chunks = Vec::new();
            for pos in &chunk_positions {
                if let Some(chunk) = self.level.read_chunk_sync(pos, std::clone::Clone::clone) {
                    spawning_chunks.push((*pos, chunk));
                }
            }

            // Reference: Vanilla Java 26.2 `ServerChunkCache.java:392` (`Util.shuffle(spawningChunks, this.level.getRandom())`).
            {
                let mut random = self
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let size = spawning_chunks.len();
                for i in (2..=size).rev() {
                    let swap_to = random.next_bounded_i32(i as i32) as usize;
                    spawning_chunks.swap(i - 1, swap_to);
                }
            }

            let _guard = handle.enter();
            for (pos, chunk) in &spawning_chunks {
                self.tick_spawning_chunk(*pos, chunk, &spawn_list, &spawn_state);
            }
        }

        // Batch these cheap lookups and atomic increments to avoid waking Rayon
        // workers for tiny tasks every tick, while retaining parallelism for large sets.
        let loaded_chunks = self.level.loaded_chunks.clone();
        let active_chunks_vec: Vec<_> = active_chunks.iter().copied().collect();
        active_chunks_vec
            .par_iter()
            .with_min_len(INHABITED_TIME_BATCH_SIZE)
            .for_each(|pos| {
                if let Some(chunk) = loaded_chunks.get(pos) {
                    chunk.inhabited_time.fetch_add(1, Relaxed);
                }
            });
    }

    pub fn check_fluid_collision(&self, bounding_box: BoundingBox) -> bool {
        let min = bounding_box.min_block_pos();

        let max = bounding_box.max_block_pos();

        for x in min.0.x..=max.0.x {
            for y in min.0.y..=max.0.y {
                for z in min.0.z..=max.0.z {
                    let pos = BlockPos::new(x, y, z);

                    let (fluid, state) = self.get_fluid_and_fluid_state(&pos);

                    if fluid.id != Fluid::EMPTY.id {
                        let height = f64::from(state.height);

                        if height >= bounding_box.min.y {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    pub fn contains_any_liquid(&self, bounding_box: BoundingBox) -> bool {
        let min_x = bounding_box.min.x.floor() as i32;
        let max_x = bounding_box.max.x.ceil() as i32;
        let min_y = bounding_box.min.y.floor() as i32;
        let max_y = bounding_box.max.y.ceil() as i32;
        let min_z = bounding_box.min.z.floor() as i32;
        let max_z = bounding_box.max.z.ceil() as i32;

        for x in min_x..max_x {
            for y in min_y..max_y {
                for z in min_z..max_z {
                    let pos = BlockPos::new(x, y, z);
                    if self.get_fluid_and_fluid_state(&pos).0.id != Fluid::EMPTY.id {
                        return true;
                    }
                }
            }
        }

        false
    }

    // FlowingFluid.getFlow()
    pub fn get_fluid_velocity(
        &self,
        pos: BlockPos,
        fluid: &Fluid,
        state: &FluidState,
    ) -> Vector3<f64> {
        fluid_flow::velocity(
            pos,
            state.height,
            state.falling,
            |pos| fluid_flow::cell(self.get_block_state_id(&pos), fluid),
            |pos, direction| self.is_solid_face(fluid.id, pos, direction),
        )
    }

    // FlowingFluid.isSolidFace()
    fn is_solid_face(&self, fluid0_id: u16, pos: BlockPos, direction: BlockDirection) -> bool {
        let id = self.get_block_state_id(&pos);

        let fluid = Fluid::from_state_id(id).unwrap_or(&Fluid::EMPTY);

        if Fluid::same_fluid_type(fluid.id, fluid0_id) {
            return false;
        }

        if direction == BlockDirection::Up {
            return true;
        }

        let block = Block::from_state_id(id);
        let state = BlockState::from_id(id);

        // Doesn't count blue ice or packed ice

        if block == &Block::ICE || block == &Block::FROSTED_ICE {
            return false;
        }

        state.is_side_solid(direction)
    }

    pub fn check_outline<F>(
        bounding_box: &BoundingBox,
        pos: BlockPos,
        state: &BlockState,
        use_outline_shape: bool,
        mut using_outline_shape: F,
    ) -> bool
    where
        F: FnMut(&BoundingBox),
    {
        if state.outline_shapes.is_empty() {
            // Apparently we need this for air and moving pistons

            return true;
        }

        let mut inside = false;
        'shapes: for shape in state.get_block_outline_shapes_at(&pos) {
            let outline_shape = shape.at_pos(pos);

            if outline_shape.intersects(bounding_box) {
                inside = true;

                if !use_outline_shape {
                    break 'shapes;
                }

                using_outline_shape(&outline_shape);
            }
        }

        inside
    }

    pub fn check_collision<F>(
        bounding_box: &BoundingBox,
        pos: BlockPos,
        state: &BlockState,
        use_collision_shape: bool,
        mut on_collision: F,
    ) -> bool
    where
        F: FnMut(&BoundingBox),
    {
        if state.is_air() || !state.is_solid() {
            return false;
        }

        let mut shapes = state
            .get_block_collision_shapes_at(&pos)
            .map(|shape| shape.at_pos(pos));

        if use_collision_shape {
            let mut collided = false;
            for collision_shape in shapes {
                if collision_shape.intersects(bounding_box) {
                    collided = true;
                    // Convert to BB and trigger the callback
                    on_collision(&collision_shape);
                }
            }
            collided
        } else {
            shapes.any(|s| s.intersects(bounding_box))
        }
    }

    /// Local collision boxes, including block entities with a moving lid.
    pub(crate) fn block_collision_boxes(&self, pos: &BlockPos) -> Vec<BoundingBox> {
        let state = self.get_block_state(pos);
        if state.id.to_block() == &Block::MOVING_PISTON
            && let Some(entity) = self.get_block_entity(pos)
            && let Some(piston) = entity
                .as_any()
                .downcast_ref::<crate::block::entities::piston::PistonBlockEntity>()
        {
            return piston.collision_boxes();
        }

        if state
            .id
            .to_block()
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_SHULKER_BOXES)
            && let Some(entity) = self.get_block_entity(pos)
            && let Some(shulker) = entity
                .as_any()
                .downcast_ref::<crate::block::entities::shulker_box::ShulkerBoxBlockEntity>(
            )
        {
            return vec![shulker.bounding_box(state.id)];
        }
        state.get_block_collision_shapes_at(pos).collect()
    }

    fn block_collision_boxes_for_entity(
        &self,
        pos: &BlockPos,
        entity: &dyn EntityBase,
    ) -> Vec<BoundingBox> {
        let (block, state) = self.get_block_and_state(pos);
        if block == &Block::POWDER_SNOW {
            crate::block::blocks::powder_snow::collision_shape_for_entity(entity, pos)
                .into_iter()
                .collect()
        } else if block == &Block::SCAFFOLDING {
            crate::block::blocks::scaffolding::ScaffoldingBlock::collision_boxes(
                state.id,
                pos,
                entity.get_entity().bounding_box.load().min.y,
                entity.get_entity().is_sneaking(),
            )
        } else {
            self.block_collision_boxes(pos)
        }
    }

    pub fn get_entity_collisions(
        &self,
        source: &dyn EntityBase,
        area: BoundingBox,
    ) -> Vec<BoundingBox> {
        let size = area.max - area.min;
        if (size.x + size.y + size.z) / 3.0 < 1.0e-7 {
            return Vec::new();
        }
        self.get_all_at_box(&area.expand(1.0e-7, 1.0e-7, 1.0e-7))
            .into_iter()
            .filter(|entity| source.can_collide_with(entity.as_ref()))
            .map(|entity| entity.get_entity().bounding_box.load())
            .collect()
    }

    pub fn collect_movement_collisions(
        &self,
        source: &dyn EntityBase,
        area: BoundingBox,
        entities: &[BoundingBox],
    ) -> Vec<crate::entity::ai::control::collision_shapes::VoxelShape> {
        use crate::entity::ai::control::collision_shapes::{Box3, VoxelShape};
        let data = |b: &BoundingBox| Box3 {
            min: [b.min.x, b.min.y, b.min.z],
            max: [b.max.x, b.max.y, b.max.z],
        };
        let mut colliders: Vec<_> = entities
            .iter()
            .map(|b| VoxelShape::from_box(data(b)))
            .collect();
        colliders.extend(
            self.worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .collision_boxes(source.get_entity().pos.load(), area)
                .iter()
                .map(|b| VoxelShape::from_box(data(b))),
        );
        let (blocks, positions) = self.get_block_collisions(area, source);
        let mut start = 0;
        for (end, pos) in positions {
            let (block, state) = self.get_block_and_state(&pos);
            let local_boxes: Vec<_> = blocks[start..end]
                .iter()
                .map(|shape| data(&shape.shift(pos.to_f64().multiply(-1.0, -1.0, -1.0))))
                .collect();
            let piston_shape = if block == &Block::MOVING_PISTON {
                self.get_block_entity(&pos).and_then(|entity| {
                    entity
                        .as_any()
                        .downcast_ref::<crate::block::entities::piston::PistonBlockEntity>()
                        .map(|piston| piston.collision_shape())
                })
            } else {
                None
            };
            let shape = if let Some(shape) = piston_shape {
                shape
            } else if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_SHULKER_BOXES)
                || block == &Block::POWDER_SNOW
                || block == &Block::SCAFFOLDING
            {
                VoxelShape::from_boxes(local_boxes)
            } else {
                VoxelShape {
                    boxes: local_boxes,
                    coordinates: state.collision_coordinates_at(&pos),
                }
            };
            colliders.push(shape.translated([
                f64::from(pos.0.x),
                f64::from(pos.0.y),
                f64::from(pos.0.z),
            ]));
            start = end;
        }
        colliders
    }

    // For adjusting movement
    pub fn get_block_collisions(
        &self,
        bounding_box: BoundingBox,
        entity: &dyn EntityBase,
    ) -> (Vec<BoundingBox>, Vec<(usize, BlockPos)>) {
        let mut collisions = Vec::new();

        let mut positions = Vec::new();

        let min = BlockPos::floored_v(bounding_box.min.add_raw(-1.0, -1.0, -1.0));
        let max = BlockPos::floored_v(bounding_box.max.add_raw(1.0, 1.0, 1.0));
        let pos_iter = BlockPos::iterate(min, max);

        for pos in pos_iter {
            let state = self.get_block_state(&pos);

            if state.is_air() {
                continue;
            }

            let local = self.block_collision_boxes_for_entity(&pos, entity);
            let shapes: Vec<_> = local.into_iter().map(|shape| shape.at_pos(pos)).collect();
            if shapes.iter().any(|shape| shape.intersects(&bounding_box)) {
                // BlockCollisions returns the complete VoxelShape once it intersects.
                collisions.extend(shapes);
                positions.push((collisions.len(), pos));
            }
        }

        (collisions, positions)
    }

    /// Java CollisionGetter.findSupportingBlock uses colliding shapes, not non-air cells.
    pub fn find_supporting_block(
        &self,
        entity: &dyn EntityBase,
        area: BoundingBox,
    ) -> Option<BlockPos> {
        let (_, positions) = self.get_block_collisions(area, entity);
        crate::entity::support::nearest(
            entity.get_entity().pos.load(),
            positions.into_iter().map(|(_, pos)| pos),
        )
    }

    pub fn is_space_empty(&self, bounding_box: BoundingBox) -> bool {
        let min = BlockPos::floored_v(bounding_box.min.add_raw(-1.0, -1.0, -1.0));
        let max = BlockPos::floored_v(bounding_box.max.add_raw(1.0, 1.0, 1.0));

        for pos in BlockPos::iterate(min, max) {
            let collided = self
                .block_collision_boxes(&pos)
                .iter()
                .any(|shape| shape.at_pos(pos).intersects(&bounding_box));

            if collided {
                return false;
            }
        }
        true
    }

    /// DismountHelper ignores climbables and open trapdoors when locating the floor.
    pub fn get_non_climbable_dismount_height(&self, pos: &BlockPos) -> f64 {
        let shape_top = |pos: &BlockPos| {
            let (block, state) = self.get_block_and_state(pos);
            if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_CLIMBABLE)
                || (block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_TRAPDOORS)
                    && block.properties(state.id).is_some_and(|props| {
                        props
                            .to_props()
                            .iter()
                            .any(|(key, value)| *key == "open" && *value == "true")
                    }))
            {
                f64::NEG_INFINITY
            } else {
                self.block_collision_boxes(pos)
                    .into_iter()
                    .map(|shape| shape.max.y)
                    .fold(f64::NEG_INFINITY, f64::max)
            }
        };
        let top = shape_top(pos);
        if top != f64::NEG_INFINITY {
            return top;
        }
        let below = shape_top(&pos.down());
        if below >= 1.0 {
            below - 1.0
        } else {
            f64::NEG_INFINITY
        }
    }

    /// Vanilla's `BlockView.getDismountHeight()`.
    /// Returns the Y surface height for dismounting at the given block position,
    /// or `f64::NEG_INFINITY` if no valid surface exists.
    pub fn get_dismount_height(&self, pos: &BlockPos) -> f64 {
        let max_y = self
            .block_collision_boxes(pos)
            .into_iter()
            .map(|s| s.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if max_y != f64::NEG_INFINITY {
            return max_y;
        }
        // No collision at pos — check block below
        let below = BlockPos(Vector3::new(pos.0.x, pos.0.y - 1, pos.0.z));
        let below_max_y = self
            .block_collision_boxes(&below)
            .into_iter()
            .map(|s| s.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if below_max_y >= 1.0 {
            below_max_y - 1.0
        } else {
            f64::NEG_INFINITY
        }
    }

    pub fn tick_spawning_chunk(
        self: &Arc<Self>,
        chunk_pos: Vector2<i32>,
        chunk: &Arc<ChunkData>,
        spawn_list: &Vec<&'static MobCategory>,
        spawn_state: &Arc<SpawnState>,
    ) {
        // this.level.tickThunder(chunk);
        let (is_raining, is_thundering) = (self.is_raining(), self.is_thundering());

        // Reference: Vanilla Java 26.2 `ServerLevel.java:544-578` (`tickThunder`).
        if is_raining && is_thundering && self.rand_bounded_i32(100_000) == 0 {
            let rand_pos =
                self.level
                    .get_block_random_pos(chunk_pos.x << 4, 0, chunk_pos.y << 4, 15);
            let height = chunk
                .heightmap
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(MotionBlocking, rand_pos.0.x, rand_pos.0.z, self.min_y);
            let random_pos = Vector3::new(rand_pos.0.x, height, rand_pos.0.z);
            // TODO this.getBrightness(LightLayer.SKY, blockPos) >= 15;
            // TODO heightmap

            // TODO findLightningRod(blockPos)
            // TODO encapsulatingFullBlocks
            if true {
                // TODO biome.getPrecipitationAt(pos, this.getSeaLevel()) == Biome.Precipitation.RAIN
                // TODO this.getCurrentDifficultyAt(blockPos);
                if self.rand_f32() < 0.0675
                    && self.get_block(&random_pos.to_block_pos().down()) != &Block::LIGHTNING_ROD
                {
                    let entity = Entity::new(
                        self.clone(),
                        random_pos.to_f64(),
                        &EntityType::SKELETON_HORSE,
                    );
                    self.spawn_entity_non_save(Arc::new(entity));
                }
                let entity = Entity::new(
                    self.clone(),
                    random_pos.to_f64().add_raw(0.5, 0., 0.5),
                    &EntityType::LIGHTNING_BOLT,
                );
                self.spawn_entity_non_save(Arc::new(entity));
            }
        }

        if spawn_list.is_empty() {
            return;
        }
        // TODO this.level.canSpawnEntitiesInChunk(chunkPos)
        spawn_for_chunk(
            self,
            chunk_pos,
            chunk,
            spawn_state,
            spawn_list,
            is_thundering,
        );
    }

    pub fn get_world_age(&self) -> i64 {
        self.level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age
    }

    pub fn get_time_of_day(&self) -> i64 {
        self.level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .time_of_day
    }

    pub fn set_time_of_day(&self, time: i64) {
        let level_time = {
            let mut guard = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.set_time(time);
            guard.clone()
        };
        level_time.send_time(self);
    }

    pub fn is_raining(&self) -> bool {
        self.weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .raining
    }

    pub fn is_raining_at(&self, pos: &BlockPos) -> bool {
        if !self.is_raining() {
            return false;
        }
        if self.get_heightmap_height(MotionBlocking, pos.0.x, pos.0.z) + 1 > pos.0.y {
            return false;
        }
        self.can_see_sky(pos)
            && self
                .get_biome(pos)
                .weather
                .is_rain_at(pos.0.x, pos.0.y, pos.0.z, self.sea_level)
    }

    /// Ticks precipitation at the given column position.
    ///
    /// Reference: Vanilla Java 26.2 `ServerLevel.java:581-611` (`tickPrecipitation`).
    pub fn tick_precipitation(self: &Arc<Self>, pos: BlockPos) {
        precipitation::tick_precipitation(self, pos);
    }

    pub fn set_raining(&self, raining: bool) {
        if let Some(server) = self.server.upgrade() {
            let world_arc = server.get_world_from_dimension(&self.dimension);
            let mut event =
                crate::plugin::api::events::world::weather_change::WeatherChangeEvent::new(
                    world_arc, raining,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let mut weather = self
            .weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if weather.raining != raining {
            let thunder = weather.thundering;
            weather.set_weather_parameters(self, 0, 0, raining, thunder);
        }
    }

    pub fn is_thundering(&self) -> bool {
        self.weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .thundering
    }

    pub fn set_thundering(&self, thundering: bool) {
        if let Some(server) = self.server.upgrade() {
            let world_arc = server.get_world_from_dimension(&self.dimension);
            let mut event =
                crate::plugin::api::events::world::weather_change::ThunderChangeEvent::new(
                    world_arc, thundering,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let mut weather = self
            .weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if weather.thundering != thundering {
            let raining = weather.raining;
            weather.set_weather_parameters(self, 0, 0, raining, thundering);
        }
    }

    /// Gets the y position of the first non air block from the top down
    pub fn get_top_block(&self, position: Vector2<i32>) -> i32 {
        let chunk_pos = Vector2::new(position.x >> 4, position.y >> 4);
        let relative_x = (position.x & 15) as usize;
        let relative_z = (position.y & 15) as usize;

        self.level
            .read_chunk_sync(&chunk_pos, |chunk| {
                let height = chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(
                        ChunkHeightmapType::WorldSurface,
                        position.x,
                        position.y,
                        self.dimension.min_y,
                    );

                if height >= self.dimension.min_y {
                    return height;
                }

                for y in (self.dimension.min_y..self.dimension.min_y + self.dimension.height).rev()
                {
                    if let Some(block_id) = chunk
                        .section
                        .get_block_absolute_y(relative_x, y, relative_z)
                        && !is_air(block_id)
                    {
                        return y;
                    }
                }
                self.dimension.min_y
            })
            .unwrap_or(self.dimension.min_y)
    }

    pub fn get_heightmap_height(&self, height_map: ChunkHeightmapType, x: i32, z: i32) -> i32 {
        let chunk_pos = Vector2::new(x >> 4, z >> 4);
        self.level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(height_map, x, z, self.min_y)
            })
            .unwrap_or(self.min_y)
    }

    #[allow(clippy::too_many_lines)]
    pub async fn spawn_bedrock_player(
        &self,
        base_config: &BasicConfiguration,
        player: Arc<Player>,
        server: &Arc<Server>,
    ) {
        static CREATIVE_CONTENT: std::sync::OnceLock<(
            Vec<CreativeGroupInfoPayload>,
            Vec<CreativeItemEntryPayload>,
        )> = std::sync::OnceLock::new();

        static BEDROCK_CRAFTING_DATA: std::sync::OnceLock<
            Vec<pumpkin_protocol::bedrock::client::BedrockRecipe>,
        > = std::sync::OnceLock::new();

        let level_info = server.level_info.load();
        let (rain_level, lightning_level) = {
            let weather = self
                .weather
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (weather.rain_level, weather.thunder_level)
        };
        let runtime_id = player.entity_id() as u64;
        let (position, yaw, pitch) = if player.has_played_before.load(Ordering::Relaxed) {
            let position = player.position();
            let yaw = player.get_entity().yaw.load(); //info.spawn_angle;
            let pitch = player.get_entity().pitch.load();

            (position, yaw, pitch)
        } else {
            let spawn_position = Vector2::new(level_info.spawn_x, level_info.spawn_z);
            let chunk_pos = Vector2::new(level_info.spawn_x >> 4, level_info.spawn_z >> 4);
            self.level.get_or_fetch_chunk(chunk_pos, |_| ()).await;
            let top = self.get_top_block(spawn_position);
            let pos_y = if top > self.dimension.min_y {
                top + 1
            } else {
                level_info.spawn_y
            };

            let position = Vector3::new(
                f64::from(level_info.spawn_x) + 0.5,
                f64::from(pos_y),
                f64::from(level_info.spawn_z) + 0.5,
            );
            (position, level_info.spawn_yaw, level_info.spawn_pitch)
        };

        // Keep the server-side transform aligned with the StartGame position. In
        // particular, this ensures an early disconnect persists the real spawn.
        player.living_entity.entity.set_pos(position);
        player.living_entity.entity.set_rotation(yaw, pitch);
        player.living_entity.entity.last_pos.store(position);

        // Todo make the data less spread
        let level_settings = LevelSettings {
            seed: self.level.seed.0,
            spawn_biome_type: 0,
            custom_biome_name: String::new(),
            dimension: VarInt(0),
            generator_type: VarInt(1),
            world_gamemode: server
                .defaultgamemode
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .gamemode
                .into(),
            hardcore: base_config.hardcore,
            difficulty: VarInt(level_info.difficulty as i32),
            spawn_position: BlockPos::new(
                level_info.spawn_x,
                level_info.spawn_y,
                level_info.spawn_z,
            ),
            has_achievements_disabled: false,
            editor_world_type: VarInt(0),
            is_created_in_editor: false,
            is_exported_from_editor: false,
            day_cycle_stop_time: VarInt(-1),
            education_edition_offer: VarUInt(0),
            has_education_features_enabled: false,
            education_product_id: String::new(),
            rain_level,
            lightning_level,
            has_confirmed_platform_locked_content: false,
            was_multiplayer_intended: true,
            was_lan_broadcasting_intended: true,
            xbox_live_broadcast_setting: GamePublishSetting::Public,
            platform_broadcast_setting: GamePublishSetting::Public,
            commands_enabled: level_info.allow_commands,
            is_texture_packs_required: false,
            rule_data: Vec::new(),
            experiments: Experiments {
                toggles: Vec::new(),
                experiments_ever_toggled: false,
            },
            bonus_chest: false,
            has_start_with_map_enabled: false,
            // TODO Bedrock permission level are different
            permission_level: 2,
            server_simulation_distance: server
                .advanced_config
                .networking
                .bedrock
                .simulation_distance
                .get()
                .into(),
            has_locked_behavior_pack: false,
            has_locked_resource_pack: false,
            is_from_locked_world_template: false,
            is_using_msa_gamertags_only: false,
            is_from_world_template: false,
            is_world_template_option_locked: false,
            is_only_spawning_v1_villagers: false,
            is_disabling_personas: false,
            is_disabling_custom_skins: false,
            emote_chat_muted: false,
            game_version: CURRENT_BEDROCK_MC_VERSION.into(),
            limited_world_width: 0,
            limited_world_height: 0,
            new_nether: true,
            edu_shared_uri_button_name: String::new(),
            edu_shared_uri_link_uri: String::new(),
            override_force_experimental_gameplay_has_value: false,
            chat_restriction_level: 0,
            disable_player_interactions: false,
            server_editor_connection_policy: VarInt(0),
            allow_anonymous_block_drops_in_editor_worlds: false,
        };
        drop(level_info);

        let Some(client) = player.client.bedrock() else {
            return;
        };

        let start_game = CStartGame {
            entity_id: VarLong(runtime_id as _),
            runtime_entity_id: VarULong(runtime_id),
            player_gamemode: player.gamemode.load().into(),
            // Bedrock represents the local player at eye height; Pumpkin stores feet position.
            position: Vector3::new(
                position.x as f32,
                position.y as f32 + player.get_entity().entity_type.eye_height,
                position.z as f32,
            ),
            pitch,
            yaw,
            level_settings,
            level_id: String::new(),
            level_name: "Pumpkin world".to_string(),
            premium_world_template_id: String::new(),
            is_trial: false,
            rewind_history_size: VarInt(0),
            server_authoritative_block_breaking: true,
            current_level_time: self.get_world_age() as _,
            enchantment_seed: VarInt(0),
            block_properties_size: VarUInt(0),
            // TODO Make this unique
            multiplayer_correlation_id: Uuid::default().to_string(),
            enable_itemstack_net_manager: true,
            server_version: "Pumpkin Rust Server".to_string(),
            compound_id: 10,
            compound_len: VarUInt(0),
            compound_end: 0,
            block_registry_checksum: 0,
            world_template_id: Uuid::nil(),
            enable_clientside_generation: false,
            blocknetwork_ids_are_hashed: false,
            server_auth_sounds: true,
            server_join_information: None,
            telemetry: ServerTelemetryData {
                server_id: String::new(),
                scenario_id: String::new(),
                world_id: String::new(),
                owner_id: String::new(),
            },
        };
        if let Ok(data) = client.serialize_packet(&start_game) {
            client.send_game_packet(data).await;
        }

        if let Ok(data) = client.serialize_packet(&CBiomeDefinitionList) {
            client.send_game_packet(data).await;
        }

        let item_registry = CItemRegistry {
            items: BedrockItem::ALL_BEDROCK_ITEMS
                .iter()
                .map(|b| ItemData {
                    item_name: b.registry_key.into(),
                    item_id: b.id,
                    is_component_based: b.component_based,
                    item_version: VarInt::from(match b.version {
                        BedrockItemVersion::Legacy => 0,
                        BedrockItemVersion::DataDriven => 1,
                        BedrockItemVersion::None => 2,
                    }),
                    component_data: b.definition_components.into(),
                })
                .collect::<Vec<_>>(),
        };
        if let Ok(data) = client.serialize_packet(&item_registry) {
            client.send_game_packet(data).await;
        }

        let (groups, entries) = CREATIVE_CONTENT.get_or_init(|| {
            let groups = pumpkin_data::bedrock_creative::CREATIVE_GROUPS
                .iter()
                .map(|g| {
                    let creative_category = match g.category {
                        1 => CreativeCategory::Construction,
                        2 => CreativeCategory::Nature,
                        3 => CreativeCategory::Equipment,
                        4 => CreativeCategory::Items,
                        5 => CreativeCategory::ItemCommandOnly,
                        _ => CreativeCategory::Undefined,
                    };
                    let icon_item = if g.icon_item_id != 0 {
                        NetworkItemDescriptor {
                            id: VarInt::from(g.icon_item_id),
                            stack_size: 1,
                            aux_value: VarUInt(g.icon_item_aux_value),
                            block_runtime_id: VarInt(0),
                            nbt_data: pumpkin_nbt::Nbt::default(),
                            place_on_blocks: Vec::new(),
                            destroy_blocks: Vec::new(),
                            shield_blocking_tick: 0,
                        }
                    } else {
                        NetworkItemDescriptor::default()
                    };

                    CreativeGroupInfoPayload {
                        creative_category,
                        name: g.name.to_string(),
                        group_icon_item: icon_item,
                    }
                })
                .collect::<Vec<_>>();

            let entries = pumpkin_data::bedrock_creative::CREATIVE_ENTRIES
                .iter()
                .enumerate()
                .map(|(i, e)| CreativeItemEntryPayload {
                    id: VarUInt((i + 1) as u32),
                    item: NetworkItemDescriptor {
                        id: VarInt::from(e.item_id),
                        stack_size: 1,
                        aux_value: VarUInt(e.item_aux_value),
                        block_runtime_id: VarInt(0),
                        nbt_data: pumpkin_nbt::Nbt::default(),
                        place_on_blocks: Vec::new(),
                        destroy_blocks: Vec::new(),
                        shield_blocking_tick: 0,
                    },
                    group_index: VarUInt(e.group_index),
                })
                .collect::<Vec<_>>();

            (groups, entries)
        });
        let creative_content = CCreativeContent { groups, entries };
        if let Ok(data) = client.serialize_packet(&creative_content) {
            client.send_game_packet(data).await;
        }

        let bedrock_recipes = BEDROCK_CRAFTING_DATA.get_or_init(|| {
            use pumpkin_data::item::{Item, JavaToBedrockItemMapping};
            use pumpkin_data::recipes::{CraftingRecipeTypes, RecipeIngredientTypes};
            use pumpkin_protocol::bedrock::client::{
                BedrockRecipe, BedrockShapedRecipe, BedrockShapelessRecipe, ItemDescriptorCount,
                RecipeUnlockRequirement,
            };
            use pumpkin_protocol::bedrock::network_item::NetworkItemDescriptor;
            use pumpkin_protocol::codec::{var_int::VarInt, var_uint::VarUInt};

            let mut mapped_recipes = Vec::new();
            let mut network_id_counter = 1u32;

            for recipe in pumpkin_data::recipes::RECIPES_CRAFTING {
                let map_ingredient = |ing: &RecipeIngredientTypes| -> ItemDescriptorCount {
                    let item_key = match ing {
                        RecipeIngredientTypes::Simple(name) => Some(*name),
                        RecipeIngredientTypes::Tagged(tag) => {
                            let tag_name = tag.strip_prefix('#').unwrap_or(tag);
                            pumpkin_data::tag::get_tag_ids(
                                pumpkin_data::tag::RegistryKey::Item,
                                tag_name,
                            )
                            .and_then(|ids| {
                                ids.first().and_then(|&first_id| {
                                    Item::from_id(first_id).map(|item| item.registry_key)
                                })
                            })
                        }
                        RecipeIngredientTypes::OneOf(names) => names.first().copied(),
                    };

                    if let Some(key) = item_key {
                        let registry_key = key.strip_prefix("minecraft:").unwrap_or(key);
                        if let Some(item) = Item::from_registry_key(registry_key)
                            && let Some(mapping) =
                                JavaToBedrockItemMapping::from_java_item_id(item.id)
                        {
                            return ItemDescriptorCount {
                                item_identifier: mapping.bedrock_item.registry_key.to_string(),
                                metadata_value: mapping.bedrock_data as i32,
                                count: 1,
                            };
                        }
                    }

                    ItemDescriptorCount {
                        item_identifier: String::new(),
                        metadata_value: 0,
                        count: 0,
                    }
                };

                match recipe {
                    CraftingRecipeTypes::CraftingShaped {
                        category: _,
                        group: _,
                        show_notification: _,
                        key,
                        pattern,
                        result,
                    } => {
                        let height = pattern.len() as i32;
                        let width = pattern.iter().map(|s| s.len()).max().unwrap_or(0) as i32;

                        let mut input = Vec::new();
                        for r in 0..height {
                            let pattern_row = pattern[r as usize];
                            for c in 0..width {
                                let ch = pattern_row.chars().nth(c as usize).unwrap_or(' ');
                                if ch == ' ' {
                                    input.push(ItemDescriptorCount {
                                        item_identifier: String::new(),
                                        metadata_value: 0,
                                        count: 0,
                                    });
                                } else {
                                    let mut ingredient = None;
                                    for &(key_ch, ref ing) in *key {
                                        if key_ch == ch {
                                            ingredient = Some(ing);
                                            break;
                                        }
                                    }
                                    if let Some(ing) = ingredient {
                                        input.push(map_ingredient(ing));
                                    } else {
                                        input.push(ItemDescriptorCount {
                                            item_identifier: String::new(),
                                            metadata_value: 0,
                                            count: 0,
                                        });
                                    }
                                }
                            }
                        }

                        let output_item = Item::from_registry_key(result.id);
                        if let Some(item) = output_item
                            && let Some(mapping) =
                                JavaToBedrockItemMapping::from_java_item_id(item.id)
                        {
                            let output_descriptor = NetworkItemDescriptor {
                                id: VarInt::from(mapping.bedrock_item.id),
                                stack_size: result.count as u16,
                                aux_value: VarUInt(mapping.bedrock_data),
                                block_runtime_id: VarInt::from(mapping.bedrock_block_state),
                                nbt_data: pumpkin_nbt::Nbt::default(),
                                place_on_blocks: Vec::new(),
                                destroy_blocks: Vec::new(),
                                shield_blocking_tick: 0,
                            };

                            mapped_recipes.push(BedrockRecipe::Shaped(BedrockShapedRecipe {
                                recipe_id: format!("pumpkin:recipe_{network_id_counter}"),
                                width: VarInt(width),
                                height: VarInt(height),
                                input,
                                output: vec![output_descriptor],
                                uuid: Uuid::nil(),
                                block: "crafting_table".to_string(),
                                priority: VarInt(1),
                                assume_symmetry: true,
                                unlock_requirement: RecipeUnlockRequirement { context: 1 },
                                recipe_network_id: VarUInt(network_id_counter),
                            }));
                            network_id_counter += 1;
                        }
                    }
                    CraftingRecipeTypes::CraftingShapeless {
                        category: _,
                        group: _,
                        ingredients,
                        result,
                    } => {
                        let input = ingredients.iter().map(map_ingredient).collect::<Vec<_>>();

                        let output_item = Item::from_registry_key(result.id);
                        if let Some(item) = output_item
                            && let Some(mapping) =
                                JavaToBedrockItemMapping::from_java_item_id(item.id)
                        {
                            let output_descriptor = NetworkItemDescriptor {
                                id: VarInt::from(mapping.bedrock_item.id),
                                stack_size: result.count as u16,
                                aux_value: VarUInt(mapping.bedrock_data),
                                block_runtime_id: VarInt::from(mapping.bedrock_block_state),
                                nbt_data: pumpkin_nbt::Nbt::default(),
                                place_on_blocks: Vec::new(),
                                destroy_blocks: Vec::new(),
                                shield_blocking_tick: 0,
                            };

                            mapped_recipes.push(BedrockRecipe::Shapeless(BedrockShapelessRecipe {
                                recipe_id: format!("pumpkin:recipe_{network_id_counter}"),
                                input,
                                output: vec![output_descriptor],
                                uuid: Uuid::nil(),
                                block: "crafting_table".to_string(),
                                priority: VarInt(1),
                                unlock_requirement: RecipeUnlockRequirement { context: 1 },
                                recipe_network_id: VarUInt(network_id_counter),
                            }));
                            network_id_counter += 1;
                        }
                    }
                    _ => {}
                }
            }
            mapped_recipes
        });

        let crafting_data = pumpkin_protocol::bedrock::client::CCraftingData {
            recipes: bedrock_recipes.clone(),
            clean_recipes: false,
        };
        if let Ok(data) = client.serialize_packet(&crafting_data) {
            client.send_game_packet(data).await;
        }

        player.on_screen_handler_opened(&player.player_screen_handler);

        {
            let mut abilities = player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            abilities.set_for_gamemode(player.gamemode.load());
        };

        let entity = &player.get_entity();
        let metadata = entity.bedrock_metadata();

        let actor_data = CSetActorData {
            target_runtime_id: VarULong(runtime_id),
            actor_data: metadata,
            synced_properties: PropertySyncData {
                int_entries_list: HashMap::new(),
                float_entries_list: HashMap::new(),
            },
            tick: VarULong(0),
        };
        if let Ok(data) = client.serialize_packet(&actor_data) {
            client.send_game_packet(data).await;
        }
        player.send_abilities_update();

        {
            let command_dispatcher = server.command_dispatcher.load();
            client_suggestions::send_bedrock_commands_packet(&player, server, &command_dispatcher);
        };

        client
            .enqueue_client_packet(&CUpdateAttributes {
                target_runtime_id: VarULong(runtime_id),
                attribute_list: vec![
                    AttributeData {
                        min_value: 0.0,
                        max_value: 3.402_823_5E38,
                        current_value: 0.1,
                        default_min_value: 0.0,
                        default_max_value: 3.402_823_5E38,
                        default_value: 0.1,
                        name: "minecraft:movement".to_string(),
                        modifiers: Vec::new(),
                    },
                    AttributeData {
                        min_value: 0.0,
                        max_value: 3.402_823_5E38,
                        current_value: 0.02,
                        default_min_value: 0.0,
                        default_max_value: 3.402_823_5E38,
                        default_value: 0.02,
                        name: "minecraft:underwater_movement".to_string(),
                        modifiers: Vec::new(),
                    },
                    AttributeData {
                        min_value: 0.0,
                        max_value: 1.0,
                        current_value: 0.08,
                        default_min_value: 0.0,
                        default_max_value: 1.0,
                        default_value: 0.08,
                        name: "minecraft:gravity".to_string(),
                        modifiers: Vec::new(),
                    },
                    AttributeData {
                        min_value: 0.0,
                        max_value: 400.0,
                        current_value: 400.0,
                        default_min_value: 0.0,
                        default_max_value: 400.0,
                        default_value: 400.0,
                        name: "minecraft:air".to_string(),
                        modifiers: Vec::new(),
                    },
                    AttributeData {
                        min_value: 0.0,
                        max_value: 20.0,
                        current_value: player.living_entity.health.load(),
                        default_min_value: 0.0,
                        default_max_value: 20.0,
                        default_value: 20.0,
                        name: "minecraft:health".to_string(),
                        modifiers: Vec::new(),
                    },
                    AttributeData {
                        min_value: 0.0,
                        max_value: 20.0,
                        current_value: player.hunger_manager.level.load().into(),
                        default_min_value: 0.0,
                        default_max_value: 20.0,
                        default_value: 20.0,
                        name: "minecraft:player.hunger".to_string(),
                        modifiers: Vec::new(),
                    },
                ],
                tick: VarULong(0),
            })
            .await;

        // --- MULTIPLAYER BROADCASTING ---

        let gameprofile = &player.gameprofile;
        let velocity = player.get_entity().velocity.load();

        // 1. Broadcast the new Bedrock player to everyone else (Java + Bedrock)
        let bedrock_player_list = CPlayerList {
            action: CPlayerList::ACTION_ADD,
            entries: vec![PlayerListEntry {
                uuid: gameprofile.id,
                entity_unique_id: VarLong(runtime_id as i64),
                username: gameprofile.name.clone(),
                xuid: String::new(),
                platform_chat_id: String::new(),
                build_platform: BuildPlatform::Unknown,
                skin: (**player.bedrock_skin.load()).clone(),
                is_teacher: false,
                is_host: false,
                is_sub_client: false,
                player_color: [0, 0, 0, 0],
            }],
        };

        let gamemode = player.gamemode.load();
        self.broadcast_packet_except_editioned(
            &[gameprofile.id],
            &CPlayerInfoUpdate::new(
                (PlayerInfoFlags::ADD_PLAYER
                    | PlayerInfoFlags::UPDATE_GAME_MODE
                    | PlayerInfoFlags::UPDATE_LISTED
                    | PlayerInfoFlags::UPDATE_LATENCY
                    | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                    | PlayerInfoFlags::UPDATE_HAT)
                    .bits(),
                &[pumpkin_protocol::java::client::play::Player {
                    uuid: gameprofile.id,
                    actions: &[
                        PlayerAction::AddPlayer {
                            name: &gameprofile.name,
                            properties: &gameprofile.properties.load(),
                        },
                        PlayerAction::UpdateGameMode(VarInt(gamemode as i32)),
                        PlayerAction::UpdateListed(true),
                        PlayerAction::UpdateLatency(VarInt(0)),
                        PlayerAction::UpdateListOrder(VarInt(0)),
                        PlayerAction::UpdateHat(true),
                    ],
                }],
            ),
            &bedrock_player_list,
        );

        let bedrock_add_player = CAddPlayer {
            uuid: gameprofile.id,
            player_name: gameprofile.name.clone(),
            target_runtime_id: VarULong(runtime_id),
            platform_chat_id: String::new(),
            position: Vector3::new(position.x as f32, position.y as f32, position.z as f32),
            velocity: Vector3::new(velocity.x as f32, velocity.y as f32, velocity.z as f32),
            rotation: Vector2::new(pitch, yaw),
            y_head_rotation: yaw,
            carried_item: NetworkItemStackDescriptor::default(),
            player_game_type: player.gamemode.load().into(),
            entity_data: entity.bedrock_metadata(),
            synced_properties: PropertySyncData::default(),
            abilities_data: pumpkin_protocol::bedrock::client::SerializedAbilitiesData {
                target_player_raw_id: runtime_id as i64,
                player_permissions:
                    pumpkin_protocol::bedrock::client::PlayerPermissionLevel::Visitor,
                command_permissions: pumpkin_protocol::bedrock::client::CommandPermissionLevel::Any,
                layers: vec![
                    pumpkin_protocol::bedrock::client::SerializedAbilitiesDataSerializedLayer {
                        serialized_layer: 0,
                        abilities_set: 0,
                        ability_value: 0,
                        fly_speed: 0.05,
                        vertical_fly_speed: 0.05,
                        walk_speed: 0.1,
                    },
                ],
            },
            actor_links: Vec::new(),
            device_id: String::new(),
            build_platform: BuildPlatform::Unknown,
        };

        self.broadcast_packet_except_editioned(
            &[gameprofile.id],
            &CSpawnEntity::new(
                (runtime_id as i32).into(),
                gameprofile.id,
                i32::from(EntityType::PLAYER.id).into(),
                position,
                pitch,
                yaw,
                yaw,
                0.into(),
                velocity,
            ),
            &bedrock_add_player,
        );

        self.send_player_equipment(&player);
        player
            .living_entity
            .send_current_equipment_attribute_modifiers();

        // Broadcast metadata to Java players so they can correctly interact with the new player
        let skin_parts = player.config.load().skin_parts;

        self.broadcast_skin_parts(
            &[gameprofile.id],
            runtime_id as i32,
            skin_parts,
            &actor_data,
        );

        // 2. Spawn existing players for our new Bedrock client
        let players = self.players.load();

        for existing_player in players
            .iter()
            .filter(|p| p.gameprofile.id != gameprofile.id)
        {
            let ex_profile = &existing_player.gameprofile;
            let ex_entity = &existing_player.get_entity();
            let ex_pos = ex_entity.pos.load();
            let ex_vel = ex_entity.velocity.load();

            let ex_player_list = CPlayerList {
                action: CPlayerList::ACTION_ADD,
                entries: vec![PlayerListEntry {
                    uuid: ex_profile.id,
                    entity_unique_id: VarLong(existing_player.entity_id() as i64),
                    username: ex_profile.name.clone(),
                    xuid: String::new(),
                    platform_chat_id: String::new(),
                    build_platform: BuildPlatform::Unknown,
                    skin: (**existing_player.bedrock_skin.load()).clone(),
                    is_teacher: false,
                    is_host: false,
                    is_sub_client: false,
                    player_color: [0, 0, 0, 0],
                }],
            };
            // Send PlayerList FIRST
            client.send_packet(&ex_player_list).await;

            let ex_add_player = CAddPlayer {
                uuid: ex_profile.id,
                player_name: ex_profile.name.clone(),
                target_runtime_id: VarULong(existing_player.entity_id() as u64),
                platform_chat_id: String::new(),
                position: Vector3::new(ex_pos.x as f32, ex_pos.y as f32, ex_pos.z as f32),
                velocity: Vector3::new(ex_vel.x as f32, ex_vel.y as f32, ex_vel.z as f32),
                rotation: Vector2::new(ex_entity.pitch.load(), ex_entity.yaw.load()),
                y_head_rotation: ex_entity.head_yaw.load(),
                carried_item: NetworkItemStackDescriptor::default(),
                player_game_type: existing_player.gamemode.load().into(),
                entity_data: ex_entity.bedrock_metadata(),
                synced_properties: PropertySyncData::default(),
                abilities_data: pumpkin_protocol::bedrock::client::SerializedAbilitiesData {
                    target_player_raw_id: existing_player.entity_id() as i64,
                    player_permissions:
                        pumpkin_protocol::bedrock::client::PlayerPermissionLevel::Visitor,
                    command_permissions:
                        pumpkin_protocol::bedrock::client::CommandPermissionLevel::Any,
                    layers: vec![
                        pumpkin_protocol::bedrock::client::SerializedAbilitiesDataSerializedLayer {
                            serialized_layer: 0,
                            abilities_set: 0,
                            ability_value: 0,
                            fly_speed: 0.05,
                            vertical_fly_speed: 0.05,
                            walk_speed: 0.1,
                        },
                    ],
                },
                actor_links: Vec::new(),
                device_id: String::new(),
                build_platform: BuildPlatform::Unknown,
            };

            client.send_packet(&ex_add_player).await;

            let ex_held_item = existing_player.inventory().held_item();

            let ex_be_mob_equipment = pumpkin_protocol::bedrock::client::CMobEquipment {
                target_runtime_id: (existing_player.entity_id() as u64).into(),
                item: (&ex_held_item).into(),
                slot: 0,
                selected_slot: 0,
                container_id: 0,
            };

            client.send_packet(&ex_be_mob_equipment).await;
        }

        player.has_played_before.store(true, Ordering::Relaxed);

        // 3. Trigger Join Event and Broadcast Join Message
        let msg_comp = TextComponent::translate_cross(
            translation::java::MULTIPLAYER_PLAYER_JOINED,
            translation::bedrock::MULTIPLAYER_PLAYER_JOINED,
            [TextComponent::text(player.gameprofile.name.clone())],
        )
        .color_named(NamedColor::Yellow);

        let mut event = PlayerJoinEvent::new(player.clone(), msg_comp);
        server.plugin_manager.fire(server, &mut event).await;

        if !event.cancelled {
            self.broadcast_system_message(&event.join_message, false);
            info!("{}", event.join_message.to_pretty_console());
        }
    }

    #[expect(clippy::too_many_lines)]
    pub async fn spawn_java_player(
        &self,
        base_config: &BasicConfiguration,
        player: &Arc<Player>,
        server: &Arc<Server>,
    ) {
        let dimensions: Vec<ResourceLocation> = server
            .dimensions
            .iter()
            .map(|d| ResourceLocation::from(d.minecraft_name))
            .collect();

        // This code follows the vanilla packet order
        let entity_id = player.entity_id();
        let gamemode = player.gamemode.load();
        debug!(
            "spawning player {}, entity id {}",
            player.gameprofile.name, entity_id
        );

        let Some(client) = player.client.java() else {
            return;
        };
        // Send the login packet for our new player
        client
            .send_packet(&CLogin::new(
                entity_id,
                base_config.hardcore,
                &dimensions,
                server
                    .advanced_config
                    .networking
                    .java
                    .max_players
                    .try_into()
                    .unwrap_or(u16::MAX.into()),
                server
                    .advanced_config
                    .networking
                    .java
                    .view_distance
                    .get()
                    .into(), //  TODO: view distance
                server
                    .advanced_config
                    .networking
                    .java
                    .simulation_distance
                    .get()
                    .into(), // TODO: sim view dinstance
                false,
                true,
                false,
                PlayerSpawnData::new(
                    self.dimension.clone(),
                    biome::hash_seed(self.level.seed.0), // seed
                    gamemode as u8,
                    player
                        .previous_gamemode
                        .load()
                        .map_or(-1, |gamemode| gamemode as i8),
                    false,
                    false,
                    None,
                    VarInt(player.get_entity().portal_cooldown.load(Ordering::Relaxed) as i32),
                    self.sea_level.into(),
                ),
                server.advanced_config.networking.java.online_mode,
                // This should stay true even when reports are disabled.
                // It prevents the annoying popup when joining the server.
                true,
            ))
            .await;

        self.pair_new_player_with_tracked_entities(player);

        // Send the current ticking state to the new player so they are in sync.
        server.tick_rate_manager.update_joining_player(player).await;

        // Permissions, i.e. the commands a player may use.
        player.send_permission_lvl_update();

        // Difficulty of the world
        player.send_difficulty_update();
        {
            let command_dispatcher = server.command_dispatcher.load();

            client_suggestions::send_c_commands_packet(player, server, &command_dispatcher);
        };
        if client.version.load() < JavaMinecraftVersion::V_1_20_2
            && client.version.load() >= JavaMinecraftVersion::V_1_13
        {
            let version = client.version.load();
            let mut tags = Vec::new();
            for &key in pumpkin_data::tag::RegistryKey::NETWORK_KEYS {
                if pumpkin_data::tag::get_registry_key_tags(version, key)
                    .is_some_and(|map| !map.is_empty())
                {
                    tags.push(key);
                }
            }
            let packet = pumpkin_protocol::java::client::play::CUpdateTagsPlay::new(&tags);
            if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version) {
                client.send_packet_now(packet_data).await;
            }
        }

        let (position, yaw, pitch) = if player.has_played_before.load(Ordering::Relaxed) {
            let position = player.position();
            let yaw = player.get_entity().yaw.load(); //info.spawn_angle;
            let pitch = player.get_entity().pitch.load();

            (position, yaw, pitch)
        } else {
            let info = &self.level_info.load();
            let spawn_position = Vector2::new(info.spawn_x, info.spawn_z);
            let chunk_pos = Vector2::new(info.spawn_x >> 4, info.spawn_z >> 4);
            self.level.get_or_fetch_chunk(chunk_pos, |_| ()).await;
            let top = self.get_top_block(spawn_position);
            let pos_y = if top > self.dimension.min_y {
                top + 1
            } else {
                info.spawn_y
            };

            let position = Vector3::new(
                f64::from(info.spawn_x) + 0.5,
                f64::from(pos_y),
                f64::from(info.spawn_z) + 0.5,
            );
            (position, info.spawn_yaw, info.spawn_pitch)
        };

        // Load chunks around the real spawn position before teleporting the client there.
        player.living_entity.entity.set_pos(position);
        player.living_entity.entity.set_rotation(yaw, pitch);
        player.living_entity.entity.last_pos.store(position);
        chunker::update_position(player);

        let center_chunk = player.living_entity.entity.chunk_pos.load();
        let chunk = self
            .level
            .get_or_fetch_chunk(center_chunk, std::clone::Clone::clone)
            .await;
        if let Some(server) = self.server.upgrade() {
            let mut event =
                crate::plugin::world::chunk_send::ChunkSend::new(player.world(), chunk.clone());
            server.plugin_manager.fire(&server, &mut event).await;
            if event.cancelled {
                return;
            }
        }
        client.send_chunks(&[chunk]).await;

        let velocity = player.living_entity.entity.velocity.load();

        debug!("Sending player teleport to {}", player.gameprofile.name);
        player.request_teleport(position, yaw, pitch);

        let gameprofile = &player.gameprofile;
        let bedrock_player_list = CPlayerList {
            action: CPlayerList::ACTION_ADD,
            entries: vec![PlayerListEntry {
                uuid: gameprofile.id,
                entity_unique_id: VarLong(entity_id as i64),
                username: gameprofile.name.clone(),
                xuid: String::new(),
                platform_chat_id: String::new(),
                build_platform: BuildPlatform::Unknown,
                skin: (**player.bedrock_skin.load()).clone(),
                is_teacher: false,
                is_host: false,
                is_sub_client: false,
                player_color: [0, 0, 0, 0],
            }],
        };

        let player_actions = [
            PlayerAction::AddPlayer {
                name: &gameprofile.name,
                properties: &gameprofile.properties.load(),
            },
            PlayerAction::UpdateGameMode(VarInt(gamemode as i32)),
            PlayerAction::UpdateListed(true),
            PlayerAction::UpdateLatency(VarInt(0)),
            PlayerAction::UpdateListOrder(VarInt(0)),
            PlayerAction::UpdateHat(true),
        ];
        let java_player = [pumpkin_protocol::java::client::play::Player {
            uuid: gameprofile.id,
            actions: &player_actions,
        }];
        let player_info_update = CPlayerInfoUpdate::new(
            (PlayerInfoFlags::ADD_PLAYER
                | PlayerInfoFlags::UPDATE_GAME_MODE
                | PlayerInfoFlags::UPDATE_LISTED
                | PlayerInfoFlags::UPDATE_LATENCY
                | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                | PlayerInfoFlags::UPDATE_HAT)
                .bits(),
            &java_player,
        );

        self.broadcast_editioned(&player_info_update, &bedrock_player_list);

        // If the player has a custom tab_list_name, send an update for it
        if let Some(tab_list_name) = player.get_tab_list_name() {
            let actions = [PlayerAction::UpdateDisplayName(Some(&tab_list_name))];
            let java_player = [pumpkin_protocol::java::client::play::Player {
                uuid: gameprofile.id,
                actions: &actions,
            }];
            self.broadcast_packet_all(&CPlayerInfoUpdate::new(
                PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
                &java_player,
            ));
        }

        // Here, we send all the infos of players who already joined.
        let mut players_tab_list_names = Vec::new();
        {
            let players = self.players.load();
            let mut data_to_process = Vec::new();
            for p in players
                .iter()
                .filter(|p| p.gameprofile.id != player.gameprofile.id)
            {
                let props_guard = p.gameprofile.properties.load();
                data_to_process.push((props_guard, p));
            }

            let mut current_player_data = Vec::new();
            for (properties, player) in &data_to_process {
                let chat_session = player
                    .chat_session
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let tab_list_name = player.get_tab_list_name();

                let mut player_actions = vec![PlayerAction::AddPlayer {
                    name: &player.gameprofile.name,
                    properties,
                }];

                if base_config.allow_chat_reports {
                    let initialized = chat_session.session_id != uuid::Uuid::nil()
                        && !chat_session.public_key.is_empty()
                        && !chat_session.signature.is_empty();
                    player_actions.push(PlayerAction::InitializeChat(initialized.then(|| {
                        InitChat {
                            session_id: chat_session.session_id,
                            expires_at: chat_session.expires_at,
                            public_key: chat_session.public_key.clone(),
                            signature: chat_session.signature.clone(),
                        }
                    })));
                }

                player_actions.extend([
                    PlayerAction::UpdateGameMode(VarInt(player.gamemode.load() as i32)),
                    PlayerAction::UpdateListed(player.tab_list_listed.load(Ordering::Relaxed)),
                    PlayerAction::UpdateLatency(VarInt(
                        player.tab_list_latency.load(Ordering::Relaxed),
                    )),
                    PlayerAction::UpdateListOrder(VarInt(
                        player.tab_list_order.load(Ordering::Relaxed),
                    )),
                    PlayerAction::UpdateHat(true),
                ]);
                drop(chat_session);

                current_player_data.push((&player.gameprofile.id, player_actions));

                // Collect tab_list_names for sending later
                if tab_list_name.is_some() {
                    players_tab_list_names.push((player.gameprofile.id, tab_list_name));
                }
            }

            let mut action_flags = PlayerInfoFlags::ADD_PLAYER
                | PlayerInfoFlags::UPDATE_LISTED
                | PlayerInfoFlags::UPDATE_LATENCY
                | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                | PlayerInfoFlags::UPDATE_GAME_MODE
                | PlayerInfoFlags::UPDATE_HAT;
            if base_config.allow_chat_reports {
                action_flags |= PlayerInfoFlags::INITIALIZE_CHAT;
            }

            let entries = current_player_data
                .iter()
                .map(|(id, actions)| java::client::play::Player {
                    uuid: **id,
                    actions,
                })
                .collect::<Vec<_>>();

            debug!("Sending player info to {}", player.gameprofile.name);
            client
                .enqueue_client_packet(&CPlayerInfoUpdate::new(action_flags.bits(), &entries))
                .await;

            // Send tab_list_names for existing players with custom names
            for (player_id, tab_list_name) in &players_tab_list_names {
                if let Some(name) = tab_list_name {
                    let actions = [PlayerAction::UpdateDisplayName(Some(name))];
                    let java_player = [pumpkin_protocol::java::client::play::Player {
                        uuid: *player_id,
                        actions: &actions,
                    }];
                    client
                        .enqueue_client_packet(&CPlayerInfoUpdate::new(
                            PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
                            &java_player,
                        ))
                        .await;
                }
            }
        };

        let gameprofile = &player.gameprofile;

        let bedrock_add_player = CAddPlayer {
            uuid: gameprofile.id,
            player_name: gameprofile.name.clone(),
            target_runtime_id: VarULong(entity_id as u64),
            platform_chat_id: String::new(),
            position: Vector3::new(position.x as f32, position.y as f32, position.z as f32),
            velocity: Vector3::new(velocity.x as f32, velocity.y as f32, velocity.z as f32),
            rotation: Vector2::new(pitch, yaw),
            y_head_rotation: yaw,
            carried_item: NetworkItemStackDescriptor::default(),
            player_game_type: player.gamemode.load().into(),
            entity_data: player.get_entity().bedrock_metadata(),
            synced_properties: PropertySyncData::default(),
            abilities_data: pumpkin_protocol::bedrock::client::SerializedAbilitiesData {
                target_player_raw_id: entity_id as i64,
                player_permissions:
                    pumpkin_protocol::bedrock::client::PlayerPermissionLevel::Visitor,
                command_permissions: pumpkin_protocol::bedrock::client::CommandPermissionLevel::Any,
                layers: vec![
                    pumpkin_protocol::bedrock::client::SerializedAbilitiesDataSerializedLayer {
                        serialized_layer: 0,
                        abilities_set: 0,
                        ability_value: 0,
                        fly_speed: 0.05,
                        vertical_fly_speed: 0.05,
                        walk_speed: 0.1,
                    },
                ],
            },
            actor_links: Vec::new(),
            device_id: String::new(),
            build_platform: BuildPlatform::Unknown,
        };

        // Spawn the player for every client.
        let spawn_entity = CSpawnEntity::new(
            entity_id.into(),
            gameprofile.id,
            i32::from(EntityType::PLAYER.id).into(),
            position,
            pitch,
            yaw,
            yaw,
            0.into(),
            velocity,
        );

        self.broadcast_packet_except_editioned(
            &[player.gameprofile.id],
            &spawn_entity,
            &bedrock_add_player,
        );

        // Broadcast metadata to Java players so they can correctly interact with the new player
        let skin_parts = player.config.load().skin_parts;

        self.broadcast_skin_parts(
            &[gameprofile.id],
            entity_id,
            skin_parts,
            &CSetActorData {
                target_runtime_id: VarULong(entity_id as u64),
                actor_data: player.get_entity().bedrock_metadata(),
                synced_properties: PropertySyncData {
                    int_entries_list: HashMap::new(),
                    float_entries_list: HashMap::new(),
                },
                tick: VarULong(0),
            },
        );

        // Spawn players for our client.
        let id = player.gameprofile.id;
        for existing_player in self
            .players
            .load()
            .iter()
            .filter(|c| c.gameprofile.id != id)
        {
            let entity = &existing_player.get_entity();
            let pos = entity.pos.load();
            let gameprofile = &existing_player.gameprofile;
            let bedrock_add_player = CAddPlayer {
                uuid: gameprofile.id,
                player_name: gameprofile.name.clone(),
                target_runtime_id: VarULong(existing_player.entity_id() as u64),
                platform_chat_id: String::new(),
                position: Vector3::new(pos.x as f32, pos.y as f32, pos.z as f32),
                velocity: Vector3::new(
                    entity.velocity.load().x as f32,
                    entity.velocity.load().y as f32,
                    entity.velocity.load().z as f32,
                ),
                rotation: Vector2::new(entity.pitch.load(), entity.yaw.load()),
                y_head_rotation: entity.head_yaw.load(),
                carried_item: NetworkItemStackDescriptor::default(),
                player_game_type: existing_player.gamemode.load().into(),
                entity_data: entity.bedrock_metadata(),
                synced_properties: PropertySyncData::default(),
                abilities_data: pumpkin_protocol::bedrock::client::SerializedAbilitiesData {
                    target_player_raw_id: existing_player.entity_id() as i64,
                    player_permissions:
                        pumpkin_protocol::bedrock::client::PlayerPermissionLevel::Visitor,
                    command_permissions:
                        pumpkin_protocol::bedrock::client::CommandPermissionLevel::Any,
                    layers: vec![
                        pumpkin_protocol::bedrock::client::SerializedAbilitiesDataSerializedLayer {
                            serialized_layer: 0,
                            abilities_set: 0,
                            ability_value: 0,
                            fly_speed: 0.05,
                            vertical_fly_speed: 0.05,
                            walk_speed: 0.1,
                        },
                    ],
                },
                actor_links: Vec::new(),
                device_id: String::new(),
                build_platform: BuildPlatform::Unknown,
            };

            let bedrock_player_list = CPlayerList {
                action: CPlayerList::ACTION_ADD,
                entries: vec![PlayerListEntry {
                    uuid: gameprofile.id,
                    entity_unique_id: VarLong(existing_player.entity_id() as i64),
                    username: gameprofile.name.clone(),
                    xuid: String::new(),
                    platform_chat_id: String::new(),
                    build_platform: BuildPlatform::Unknown,
                    skin: (**existing_player.bedrock_skin.load()).clone(),
                    is_teacher: false,
                    is_host: false,
                    is_sub_client: false,
                    player_color: [0, 0, 0, 0],
                }],
            };

            let actions = [
                PlayerAction::AddPlayer {
                    name: &gameprofile.name,
                    properties: &gameprofile.properties.load(),
                },
                PlayerAction::UpdateGameMode(VarInt(existing_player.gamemode.load() as i32)),
                PlayerAction::UpdateListed(existing_player.tab_list_listed.load(Ordering::Relaxed)),
                PlayerAction::UpdateLatency(VarInt(
                    existing_player.tab_list_latency.load(Ordering::Relaxed),
                )),
                PlayerAction::UpdateListOrder(VarInt(
                    existing_player.tab_list_order.load(Ordering::Relaxed),
                )),
                PlayerAction::UpdateHat(true),
            ];
            let java_player = [pumpkin_protocol::java::client::play::Player {
                uuid: gameprofile.id,
                actions: &actions,
            }];
            player
                .client
                .enqueue_packet_editioned(
                    &CPlayerInfoUpdate::new(
                        (PlayerInfoFlags::ADD_PLAYER
                            | PlayerInfoFlags::UPDATE_LISTED
                            | PlayerInfoFlags::UPDATE_GAME_MODE
                            | PlayerInfoFlags::UPDATE_LATENCY
                            | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                            | PlayerInfoFlags::UPDATE_HAT)
                            .bits(),
                        &java_player,
                    ),
                    &bedrock_player_list,
                )
                .await;

            player
                .client
                .enqueue_packet_editioned(
                    &CSpawnEntity::new(
                        existing_player.entity_id().into(),
                        gameprofile.id,
                        i32::from(EntityType::PLAYER.id).into(),
                        pos,
                        entity.pitch.load(),
                        entity.yaw.load(),
                        entity.head_yaw.load(),
                        0.into(),
                        entity.velocity.load(),
                    ),
                    &bedrock_add_player,
                )
                .await;

            if client.version.load() >= JavaMinecraftVersion::V_1_21 {
                let config = existing_player.config.load();
                let mut buf = Vec::new();
                {
                    let meta = Metadata::new(
                        pumpkin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
                        config.skin_parts,
                    );
                    let _ = meta.write(&mut buf, &client.version.load());
                };
                drop(config);
                // END
                buf.put_u8(255);
                client
                    .enqueue_client_packet(&CSetEntityMetadata::new(
                        existing_player.get_entity().entity_id.into(),
                        buf.into(),
                    ))
                    .await;
            }

            {
                let held_item = existing_player.inventory.held_item();
                let equipment_list = {
                    let mut equipment_list =
                        vec![(EquipmentSlot::MAIN_HAND.discriminant(), held_item.clone())];

                    let equipment_guard = existing_player
                        .inventory
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for (slot, item_stack) in &equipment_guard.equipment {
                        equipment_list.push((slot.discriminant(), item_stack.clone()));
                    }
                    equipment_list
                };

                let equipment: Vec<(i8, ItemStackSerializer)> = equipment_list
                    .iter()
                    .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
                    .collect();

                let je_packet = CSetEquipment::new(existing_player.entity_id().into(), equipment);

                let be_mob_equipment = pumpkin_protocol::bedrock::client::CMobEquipment {
                    target_runtime_id: (existing_player.entity_id() as u64).into(),
                    item: (&held_item).into(),
                    slot: 0,
                    selected_slot: 0,
                    container_id: 0,
                };

                player
                    .client
                    .enqueue_packet_editioned(&je_packet, &be_mob_equipment)
                    .await;
            }
        }
        player.send_client_information();

        player.send_abilities_update();

        // Sync selected slot
        player.enqueue_set_held_item_packet(&CSetSelectedSlot::new(
            player.get_inventory().get_selected_slot() as i8,
        ));

        if client.version.load() >= JavaMinecraftVersion::V_1_20_2 {
            // Start waiting for level chunks. Sets the "Loading Terrain" screen (Added in 1.20.2)
            debug!("Sending waiting chunks to {}", player.gameprofile.name);
            client
                .send_packet(&CGameEvent::new(GameEvent::StartWaitingChunks, 0.0))
                .await;
        }

        self.worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .init_client(client);

        // Sends initial time
        player.send_time(self);

        // Sends initial scoreboard state
        player.send_scoreboard();

        let (spawn_block_pos, yaw, pitch) = {
            let level_info_lock = self.level_info.load();
            (
                BlockPos::new(
                    level_info_lock.spawn_x,
                    level_info_lock.spawn_y,
                    level_info_lock.spawn_z,
                ),
                level_info_lock.spawn_yaw,
                level_info_lock.spawn_pitch,
            )
        };

        client
            .send_packet(&CPlayerSpawnPosition::new(
                spawn_block_pos,
                yaw,
                pitch,
                self.dimension.minecraft_name.to_owned(),
            ))
            .await;

        // Send initial weather state
        let (is_raining, rain_level, thunder_level) = {
            let weather = self
                .weather
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                weather.raining,
                weather.rain_level.clamp(0.0, 1.0),
                weather.thunder_level.clamp(0.0, 1.0),
            )
        };
        if is_raining {
            client
                .enqueue_client_packet(&CGameEvent::new(GameEvent::BeginRaining, 0.0))
                .await;

            client
                .enqueue_client_packet(&CGameEvent::new(GameEvent::RainLevelChange, rain_level))
                .await;
            client
                .enqueue_client_packet(&CGameEvent::new(
                    GameEvent::ThunderLevelChange,
                    thunder_level,
                ))
                .await;
        }

        let player_bossbars = server
            .bossbars
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_player_bars(&player.gameprofile.id)
            .map(|bars| bars.into_iter().cloned().collect::<Vec<_>>());
        if let Some(bossbars) = player_bossbars {
            for bossbar in &bossbars {
                player.send_bossbar(bossbar);
            }
        }

        player.has_played_before.store(true, Ordering::Relaxed);
        player.on_screen_handler_opened(&player.player_screen_handler);

        player.send_active_effects();
        player.breath_manager.send_air_supply(player);
        self.send_player_equipment(player);
        player
            .living_entity
            .send_current_equipment_attribute_modifiers();

        if let crate::net::ClientPlatform::Java(java_client) = player.client.as_ref()
            && server.advanced_config.recipe.send_recipes
            && java_client.version.load() >= JavaMinecraftVersion::V_1_21_2
        {
            let settings_packet = CRecipeBookSettings::default_closed();
            if let Ok(data) = java_client.serialize_packet(&settings_packet) {
                java_client.send_packet_now(data).await;
            }
            let dynamic_recipes = server.recipe_manager.get_dynamic_recipes();
            let add_packet = CRecipeBookAdd::new(true, &dynamic_recipes);
            if let Ok(data) = java_client.serialize_packet(&add_packet) {
                java_client.send_packet_now(data).await;
            }
        }
        let msg_comp = TextComponent::translate_cross(
            translation::java::MULTIPLAYER_PLAYER_JOINED,
            translation::bedrock::MULTIPLAYER_PLAYER_JOINED,
            [TextComponent::text(player.gameprofile.name.clone())],
        )
        .color_named(NamedColor::Yellow);
        let mut event = PlayerJoinEvent::new(player.clone(), msg_comp);

        server.plugin_manager.fire(server, &mut event).await;

        if !event.cancelled {
            self.broadcast_system_message(&event.join_message, false);
            // TODO: Switch to structured logging, e.g. info!(player = %name, "connected")
            info!("{}", event.join_message.to_pretty_console());
        }
    }

    fn send_player_equipment(&self, from: &Player) {
        let held_item = from.inventory.held_item();
        let mut equipment_list = vec![(EquipmentSlot::MAIN_HAND.discriminant(), held_item.clone())];

        let equipment_guard = from
            .inventory
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (slot, item_stack) in &equipment_guard.equipment {
            equipment_list.push((slot.discriminant(), item_stack.clone()));
        }
        drop(equipment_guard);

        let equipment: Vec<(i8, ItemStackSerializer)> = equipment_list
            .iter()
            .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
            .collect();
        let je_packet = CSetEquipment::new(from.entity_id().into(), equipment);

        let be_mob_equipment = pumpkin_protocol::bedrock::client::CMobEquipment {
            target_runtime_id: (from.entity_id() as u64).into(),
            item: (&held_item).into(),
            slot: 0,
            selected_slot: 0,
            container_id: 0,
        };

        self.send_to_tracking_players_editioned(from.get_entity(), &je_packet, &be_mob_equipment);
    }

    pub fn send_world_info(
        &self,
        player: &Arc<Player>,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
    ) {
        if let ClientPlatform::Java(client) = player.client.as_ref() {
            self.worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .init_client(client);
        }

        // TODO: World spawn (compass stuff)

        if let ClientPlatform::Java(client) = player.client.as_ref()
            && client.version.load() >= JavaMinecraftVersion::V_1_20_2
        {
            player.try_send_client_packet(&CGameEvent::new(GameEvent::StartWaitingChunks, 0.0));
        }

        let entity = &player.get_entity();

        self.broadcast_packet_except(
            &[player.gameprofile.id],
            // TODO: add velo
            &CSpawnEntity::new(
                entity.entity_id.into(),
                player.gameprofile.id,
                i32::from(EntityType::PLAYER.id).into(),
                position,
                pitch,
                yaw,
                yaw,
                0.into(),
                Vector3::new(0.0, 0.0, 0.0),
            ),
        );

        player.send_client_information();

        chunker::update_position(player);
        // Update commands

        player.set_health(20.0);
    }

    pub fn explode(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        interaction: ExplosionInteraction,
    ) {
        self.explode_with_calculator(position, power, interaction, None, false);
    }

    /// `caused_by_player` mirrors vanilla's `Explosion#getIndirectSourceEntity() instanceof
    /// Player` check (`BlockBehaviour.java:180`): pass `true` only when the explosion's
    /// indirect source entity is a player (e.g. TNT a player ignited), so ore it breaks
    /// drops experience. A creeper or other non-player `LivingEntity` source, and a
    /// sourceless explosion (bed / respawn anchor), must pass `false`.
    pub fn explode_with_calculator(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        interaction: ExplosionInteraction,
        damage_calculator: Option<Arc<dyn ExplosionDamageCalculator>>,
        caused_by_player: bool,
    ) {
        self.explode_with_source(
            position,
            power,
            interaction,
            damage_calculator,
            caused_by_player,
            None,
        );
    }

    pub fn explode_with_source(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        interaction: ExplosionInteraction,
        damage_calculator: Option<Arc<dyn ExplosionDamageCalculator>>,
        caused_by_player: bool,
        source: Option<&dyn EntityBase>,
    ) {
        let block_interaction = self.get_block_interaction(interaction);
        let mut explosion = Explosion::new(power, position, block_interaction).with_source(source);
        if caused_by_player {
            explosion = explosion.caused_by_player();
        }
        if let Some(calc) = damage_calculator {
            explosion = explosion.with_damage_calculator(calc);
        }
        self.run_explosion(&explosion, position, power);
    }

    pub fn explode_bad_respawn_point(
        self: &Arc<Self>,
        position: Vector3<f64>,
        calculator: Option<Arc<dyn ExplosionDamageCalculator>>,
    ) {
        let mut explosion = Explosion::new(
            5.0,
            position,
            self.get_block_interaction(ExplosionInteraction::Block),
        )
        .bad_respawn_point();
        if let Some(calculator) = calculator {
            explosion = explosion.with_damage_calculator(calculator);
        }
        self.run_explosion(&explosion, position, 5.0);
    }

    pub fn explode_tnt_minecart(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        source: &dyn EntityBase,
    ) {
        let block_interaction = self.get_block_interaction(ExplosionInteraction::Tnt);
        let explosion = Explosion::new(power, position, block_interaction)
            .with_source(Some(source))
            .preserving_rails();
        self.run_explosion(&explosion, position, power);
    }

    #[must_use]
    pub fn get_block_interaction(&self, interaction: ExplosionInteraction) -> BlockInteraction {
        let game_rules = &self.level_info.load().game_rules;
        match interaction {
            ExplosionInteraction::None => BlockInteraction::Keep,
            ExplosionInteraction::Block => {
                Self::get_destroy_type(game_rules.block_explosion_drop_decay)
            }
            ExplosionInteraction::Mob => {
                if game_rules.mob_griefing {
                    Self::get_destroy_type(game_rules.mob_explosion_drop_decay)
                } else {
                    BlockInteraction::Keep
                }
            }
            ExplosionInteraction::Tnt => {
                Self::get_destroy_type(game_rules.tnt_explosion_drop_decay)
            }
            ExplosionInteraction::Trigger => BlockInteraction::TriggerBlock,
        }
    }

    #[must_use]
    pub const fn get_destroy_type(drop_decay: bool) -> BlockInteraction {
        if drop_decay {
            BlockInteraction::DestroyWithDecay
        } else {
            BlockInteraction::Destroy
        }
    }

    fn run_explosion(
        self: &Arc<Self>,
        explosion: &Explosion<'_>,
        position: Vector3<f64>,
        power: f32,
    ) {
        let mut event = crate::plugin::api::events::entity::entity_explode::EntityExplodeEvent::new(
            explosion
                .source
                .map_or(0, |source| source.get_entity().entity_id),
            position,
            power,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        self.emit_game_event_from_entity("explode", position, explosion.source, None);
        let block_count = explosion.explode(self);
        let particle = if power < 2.0 {
            Particle::Explosion
        } else {
            Particle::ExplosionEmitter
        };
        for player in self.players.load().iter() {
            let mut sound_id = Sound::EntityGenericExplode as u16;
            if let ClientPlatform::Java(java_client) = player.client.as_ref() {
                sound_id = remap_sound_id_for_version(sound_id, java_client.version.load());
            }
            let sound = IdOr::<SoundEvent>::Id(sound_id);
            if player.position().squared_distance_to_vec(&position) > 4096.0 {
                continue;
            }
            player.try_send_client_packet(&CExplosion::new(
                position,
                power,
                block_count as i32,
                None,
                VarInt(particle as i32),
                sound,
            ));
        }
    }

    pub(crate) fn despawn_dead_java_player_for_bedrock(&self, subject: &Entity) {
        let Some(player) = self.get_player_by_id(subject.entity_id) else {
            return;
        };
        if matches!(player.client.as_ref(), ClientPlatform::Java(_)) {
            self.broadcast_to_chunk_bedrock(
                subject.chunk_pos.load(),
                &CRemoveActor::new(VarLong(subject.entity_id.into())),
            );
        }
    }

    async fn refresh_java_player_for_bedrock(&self, subject: &Player) {
        if !matches!(subject.client.as_ref(), ClientPlatform::Java(_)) {
            return;
        }

        let entity = subject.get_entity();
        let entity_id = subject.entity_id();
        let position = entity.pos.load();
        let velocity = entity.velocity.load();
        let player_list = CPlayerList {
            action: CPlayerList::ACTION_ADD,
            entries: vec![PlayerListEntry {
                uuid: subject.gameprofile.id,
                entity_unique_id: VarLong(entity_id.into()),
                username: subject.gameprofile.name.clone(),
                xuid: String::new(),
                platform_chat_id: String::new(),
                build_platform: BuildPlatform::Unknown,
                skin: (**subject.bedrock_skin.load()).clone(),
                is_teacher: false,
                is_host: false,
                is_sub_client: false,
                player_color: [0; 4],
            }],
        };
        let add_player = CAddPlayer {
            uuid: subject.gameprofile.id,
            player_name: subject.gameprofile.name.clone(),
            target_runtime_id: VarULong(entity_id as u64),
            platform_chat_id: String::new(),
            position: Vector3::new(position.x as f32, position.y as f32, position.z as f32),
            velocity: Vector3::new(velocity.x as f32, velocity.y as f32, velocity.z as f32),
            rotation: Vector2::new(entity.pitch.load(), entity.yaw.load()),
            y_head_rotation: entity.head_yaw.load(),
            carried_item: NetworkItemStackDescriptor::default(),
            player_game_type: subject.gamemode.load().into(),
            entity_data: entity.bedrock_metadata(),
            synced_properties: PropertySyncData::default(),
            abilities_data: pumpkin_protocol::bedrock::client::SerializedAbilitiesData {
                target_player_raw_id: entity_id as i64,
                player_permissions:
                    pumpkin_protocol::bedrock::client::PlayerPermissionLevel::Visitor,
                command_permissions: pumpkin_protocol::bedrock::client::CommandPermissionLevel::Any,
                layers: vec![
                    pumpkin_protocol::bedrock::client::SerializedAbilitiesDataSerializedLayer {
                        serialized_layer: 0,
                        abilities_set: 0,
                        ability_value: 0,
                        fly_speed: 0.05,
                        vertical_fly_speed: 0.05,
                        walk_speed: 0.1,
                    },
                ],
            },
            actor_links: Vec::new(),
            device_id: String::new(),
            build_platform: BuildPlatform::Unknown,
        };
        let remove = CRemoveActor::new(VarLong(entity_id.into()));

        for recipient in self.players.load().iter() {
            if let ClientPlatform::Bedrock(client) = recipient.client.as_ref() {
                client.send_packet(&remove).await;
                client.send_packet(&player_list).await;
                client.send_packet(&add_player).await;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn respawn_player(self: &Arc<Self>, player: &Arc<Player>, alive: bool) {
        let last_pos = player.get_entity().last_pos.load();
        let death_dimension = ResourceLocation::from(player.world().dimension.minecraft_name);
        let death_location = BlockPos(Vector3::new(
            last_pos.x.round() as i32,
            last_pos.y.round() as i32,
            last_pos.z.round() as i32,
        ));

        let data_kept = u8::from(alive);

        let server = self.server.upgrade();
        let default_world = server.as_ref().map_or_else(
            || self.clone(),
            |s| s.get_world_from_dimension(&Dimension::OVERWORLD),
        );

        // Copy spawn info from default world level_info to avoid holding lock across await
        let (spawn_x, spawn_y, spawn_z, spawn_yaw, spawn_pitch, keep_inventory) = {
            let info = default_world.level_info.load();
            (
                info.spawn_x,
                info.spawn_y,
                info.spawn_z,
                info.spawn_yaw,
                info.spawn_pitch,
                info.game_rules.keep_inventory,
            )
        };

        // Get respawn position and dimension
        let (position, yaw, pitch, respawn_dimension) = if let Some(respawn) =
            player.calculate_respawn_point().await
        {
            (
                respawn.position,
                respawn.yaw,
                respawn.pitch,
                respawn.dimension,
            )
        } else {
            // No valid respawn point - send notification if player had one set
            if player
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            {
                player
                    .send_client_packet(&CGameEvent::new(GameEvent::NoRespawnBlockAvailable, 0.0))
                    .await;
                let mut guard = player
                    .respawn_point
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(point) = guard.as_ref()
                    && !point.force
                {
                    *guard = None;
                }
            }

            // FIXME: This spawn position calculation is incorrect. Should use vanilla's
            // proper spawn position calculation (see #1381). The y-level calculation
            // needs to account for spawn radius and find a safe spawn position.
            let chunk_pos = Vector2::new(spawn_x >> 4, spawn_z >> 4);
            default_world
                .level
                .get_or_fetch_chunk(chunk_pos, |_| ())
                .await;
            let top = default_world.get_top_block(Vector2::new(spawn_x, spawn_z));
            let pos_y = if top > default_world.dimension.min_y {
                top + 1
            } else {
                spawn_y
            };

            (
                Vector3::new(
                    f64::from(spawn_x) + 0.5,
                    f64::from(pos_y),
                    f64::from(spawn_z) + 0.5,
                ),
                spawn_yaw,
                spawn_pitch,
                default_world.dimension.clone(),
            )
        };

        let mut spawn_loc_event = crate::plugin::api::events::player::player_spawn_location::PlayerSpawnLocationEvent::new(
            player.clone(),
            position,
        );
        if let Some(ref s) = server {
            s.plugin_manager.fire(s, &mut spawn_loc_event).await;
        }
        let position = spawn_loc_event.spawn_pos;

        // Candidate destination world for a cross-dimension respawn.
        let candidate_world = if respawn_dimension == self.dimension {
            None
        } else {
            server.as_ref().map_or_else(
                || {
                    warn!("Could not get server for cross-dimension respawn");
                    None
                },
                |s| {
                    let worlds = s.worlds.load();
                    worlds
                        .iter()
                        .find(|w| w.dimension == respawn_dimension)
                        .cloned()
                },
            )
        };

        // Fire PlayerChangeWorldEvent (cancellable) before the transfer; it runs before
        // the non-cancellable PlayerRespawnEvent, which observes the resolved world.
        let (resolved_world, position, yaw, pitch) = if let Some(new_world) = candidate_world {
            if let Some(ref s) = server {
                let mut event = PlayerChangeWorldEvent {
                    player: player.clone(),
                    previous_world: self.clone(),
                    new_world: new_world.clone(),
                    position,
                    yaw,
                    pitch,
                    cancelled: false,
                };
                s.plugin_manager.fire(s, &mut event).await;

                if event.cancelled {
                    (None, position, yaw, pitch)
                } else {
                    let destination = event.new_world;
                    let position = event.position;
                    let yaw = event.yaw;
                    let pitch = event.pitch;

                    // Skip the transfer if redirected back to the current world.
                    if destination.uuid != self.uuid {
                        debug!(
                            "Cross-dimension respawn: {} -> {}",
                            self.dimension.minecraft_name, destination.dimension.minecraft_name
                        );

                        // Detach from the old world before publishing into the new one, so no
                        // observer sees the player in a world whose chunk manager doesn't match.
                        self.remove_player(player, false).await;
                        player.unload_watched_chunks(self).await;
                        player.change_world_chunks(&self.level, &destination);
                        player.living_entity.entity.set_world(destination.clone());
                        destination.players.rcu(|current_list| {
                            let mut new_list = (**current_list).clone();
                            new_list.push(player.clone());
                            new_list
                        });
                    }

                    (Some(destination), position, yaw, pitch)
                }
            } else {
                warn!("Server dropped during cross-dimension respawn");
                (None, position, yaw, pitch)
            }
        } else {
            if respawn_dimension != self.dimension {
                warn!(
                    "Target world {:?} not found, using world spawn in {:?}",
                    respawn_dimension, self.dimension
                );
            }
            (None, position, yaw, pitch)
        };

        // Cancelled or unresolved cross-dimension respawns fall back to the current
        // world's spawn below; otherwise the resolved values from the event apply.
        let (target_world, position, yaw, pitch) = resolved_world.as_ref().map_or_else(
            || (self.clone(), position, yaw, pitch),
            |new_world| (new_world.clone(), position, yaw, pitch),
        );

        // Notify plugins that the player has respawned (non-cancellable).
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire(
                    &server,
                    &mut PlayerRespawnEvent::new(
                        player.clone(),
                        self.clone(),
                        target_world.clone(),
                        position,
                        yaw,
                        pitch,
                        alive,
                    ),
                )
                .await;
        }

        // Send respawn packet with target dimension (using send_packet_now to ensure proper order)
        player
            .send_client_packet(&CRespawn::new(
                PlayerSpawnData::new(
                    target_world.dimension.clone(),
                    biome::hash_seed(target_world.level.seed.0),
                    player.gamemode.load() as u8,
                    player.gamemode.load() as i8,
                    false,
                    false,
                    Some((death_dimension, death_location)),
                    VarInt(player.get_entity().portal_cooldown.load(Ordering::Relaxed) as i32),
                    target_world.sea_level.into(),
                ),
                data_kept,
            ))
            .await;

        // Inform the client of the default spawn position so the client doesn't
        // fall back to (0, 2, 0) while the world reloads (fixes rubberbanding).
        // This must be sent after the CRespawn packet for proper client positioning.
        let spawn_block_pos = BlockPos(Vector3::new(
            position.x.round() as i32,
            position.y.round() as i32,
            position.z.round() as i32,
        ));
        let bedrock_dimension = match target_world.dimension.minecraft_name {
            "minecraft:the_nether" => 1,
            "minecraft:the_end" => 2,
            _ => 0,
        };
        player
            .send_packet_now_editioned(
                &CPlayerSpawnPosition::new(
                    spawn_block_pos,
                    yaw,
                    pitch,
                    target_world.dimension.minecraft_name.to_string(),
                ),
                &pumpkin_protocol::bedrock::client::CSetSpawnPosition {
                    spawn_position_type:
                        pumpkin_protocol::bedrock::client::SpawnPositionType::WorldRespawn,
                    block_position: spawn_block_pos,
                    dimension_type: bedrock_dimension.into(),
                    spawn_block_pos,
                },
            )
            .await;

        player.living_entity.reset_state();
        player.reset_flight_after_respawn();

        player.send_permission_lvl_update();

        player.hunger_manager.restart();

        if !keep_inventory {
            player.set_experience(0, 0.0, 0);
            player.inventory.clear();
        }

        // Set entity position BEFORE loading chunks, so chunks load at the right location
        // This mirrors the initial spawn flow where update_position is called before teleport
        player.get_entity().set_pos(position);
        player.get_entity().set_rotation(yaw, pitch);
        player.get_entity().last_pos.store(position);

        // TODO: difficulty, exp bar, status effect

        // Load chunks and send world info FIRST (before teleport packet)
        target_world.send_world_info(player, position, yaw, pitch);

        // Ensure at least the center chunk is sent synchronously before teleport.
        if let crate::net::ClientPlatform::Java(java_client) = player.client.as_ref() {
            let center_chunk = player.get_entity().chunk_pos.load();
            let chunk = target_world
                .level
                .get_or_fetch_chunk(center_chunk, std::clone::Clone::clone)
                .await;
            java_client.send_chunks(&[chunk]).await;
        }

        // Send teleport packet after at least the center chunk was delivered
        player.request_teleport(position, yaw, pitch);

        target_world.refresh_java_player_for_bedrock(player).await;
    }

    /// Returns true if enough players are sleeping and we should skip the night.
    pub fn should_skip_night(&self) -> bool {
        let players = self.players.load();

        let player_count = players.len();
        let sleeping_player_count = players
            .iter()
            .filter(|player| {
                player
                    .sleeping_since
                    .load()
                    .is_some_and(|since| since >= 100)
            })
            .count();
        drop(players);

        if player_count == 0 {
            return false;
        }

        let sleep_percentage = self
            .level_info
            .load()
            .game_rules
            .players_sleeping_percentage;
        if sleep_percentage > 100 {
            return false;
        }
        let sleep_percentage = sleep_percentage.max(0);
        let required_sleeping =
            ((player_count as f64 * sleep_percentage as f64) / 100.0).ceil() as usize;
        let required_sleeping = required_sleeping.max(1);

        sleeping_player_count >= required_sleeping
    }

    // NOTE: This function doesn't actually await on anything, it just spawns two tokio tasks
    /// IMPORTANT: Chunks have to be non-empty
    fn spawn_world_entity_chunks(self: &Arc<Self>, player: Arc<Player>, chunks: Vec<Vector2<i32>>) {
        #[cfg(debug_assertions)]
        let inst = std::time::Instant::now();

        // Note: `chunks` originates from `Cylindrical::changed_chunks`, which is
        // already ordered from closest to farthest from center by the precompiled
        // cylindrical chunk view LUT. No re-sorting needed.

        let mut entity_receiver = self.level.receive_entity_chunks(chunks);
        let level = self.level.clone();
        let world = self.clone();

        player.clone().spawn_task(async move {
            'main: loop {
                let recv_result = tokio::select! {
                    () = player.client.await_close_interrupt() => {
                        debug!("Canceling player packet processing");
                        None
                    },
                    recv_result = entity_receiver.recv() => {
                        recv_result
                    }
                };

                let Some((chunk_weak, first_load)) = recv_result else {
                    break;
                };

                let Some(chunk) = chunk_weak.upgrade() else {
                    continue;
                };

                let position = Vector2::new(chunk.x, chunk.z);

                if !level.is_chunk_watched(&position) {
                    // No longer watched: don't make its entities live. Leave the
                    // serialized data untouched so the normal unload path persists
                    // it as-is (nothing went live, so there is nothing to save).
                    trace!(
                        "Received entity chunk {:?}, but it is no longer watched; leaving it for the unload path",
                        &position
                    );
                    continue 'main;
                }

                if first_load {
                    // First watcher: consume the serialized entities and make them
                    // live. The live entity list becomes the single source of
                    // truth, so the chunk's NBT is taken (cleared) to avoid keeping
                    // a duplicate copy that would be re-appended on the next unload
                    // and doubled on every reload.
                    let entity_nbts = std::mem::take(
                        &mut *chunk
                            .data
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                    let mut entities_to_add: Vec<Arc<dyn EntityBase>> =
                        Vec::with_capacity(entity_nbts.len());
                    for entity_nbt in &entity_nbts {
                        let Some(id) = entity_nbt.get_string("id") else {
                            debug!("Entity has no ID");
                            continue;
                        };
                        let Some(entity_type) =
                            EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))
                        else {
                            warn!("Entity has no valid Entity Type {id}");
                            continue;
                        };

                        // Keep the persisted UUID so the entity keeps its identity
                        // across reloads (matching vanilla); only fall back to a
                        // fresh one if it is missing/corrupt.
                        let uuid = entity_nbt.get_uuid("UUID").unwrap_or_else(Uuid::new_v4);
                        // Pos is zero since it will be read from nbt.
                        let entity =
                            from_type(entity_type, Vector3::new(0.0, 0.0, 0.0), &world, uuid);
                        entity.read_nbt_non_mut(entity_nbt);
                        entity.init_data_tracker();

                        // Motion loaded from NBT is authoritative. Resetting it
                        // here strands falling blocks, TNT and other moving entities.

                        player.client.enqueue_spawn_packet(&entity);
                        player.try_restore_vehicle(&entity);
                        entities_to_add.push(entity);
                    }

                    if !entities_to_add.is_empty() {
                        world.entities.rcu(|current_entities| {
                            let mut new_entities = (**current_entities).clone();
                            new_entities.extend(entities_to_add.iter().cloned());
                            new_entities
                        });
                    }
                } else {
                    // The chunk's entities are already live (another watcher loaded
                    // them). Just send this player the spawn packets for the live
                    // entities currently in this chunk.
                    for entity in world.entities.load().iter() {
                        let base_entity = entity.get_entity();
                        if base_entity.chunk_pos.load() == position {
                            player.client.enqueue_spawn_packet(entity);
                            player.try_restore_vehicle(entity);
                        }
                    }
                }
            }

            #[cfg(debug_assertions)]
            debug!("Chunks queued after {}ms", inst.elapsed().as_millis());
        });
    }

    /// Gets a `Player` by an entity id
    pub fn get_player_by_id(&self, id: i32) -> Option<Arc<Player>> {
        for player in self.players.load().iter() {
            if player.entity_id() == id {
                return Some(player.clone());
            }
        }
        None
    }

    /// Gets an entity by an entity id
    pub fn get_entity_by_id(&self, id: i32) -> Option<Arc<dyn EntityBase>> {
        for entity in self.entities.load().iter() {
            if entity.get_entity().entity_id == id {
                return Some(entity.clone());
            }
        }
        for player in self.players.load().iter() {
            if player.get_entity().entity_id == id {
                return Some(player.clone() as Arc<dyn EntityBase>);
            }
        }
        None
    }

    /// Gets a `Player` by a username
    pub fn get_player_by_name(&self, name: &str) -> Option<Arc<Player>> {
        for player in self.players.load().iter() {
            if player.gameprofile.name.eq_ignore_ascii_case(name) {
                return Some(player.clone());
            }
        }
        None
    }

    // Gets all entities at a Box
    pub fn get_all_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<dyn EntityBase>> {
        let entities_guard = self.entities.load();
        let players_guard = self.players.load();

        entities_guard
            .iter()
            .map(|e| e.clone() as Arc<dyn EntityBase>)
            .chain(
                players_guard
                    .iter()
                    .map(|p| p.clone() as Arc<dyn EntityBase>),
            )
            .filter(|entity| entity.get_entity().bounding_box.load().intersects(aabb))
            .collect()
    }

    // Gets all non Player entities at a Box
    pub fn get_entities_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<dyn EntityBase>> {
        self.entities
            .load()
            .iter()
            .filter(|entity| entity.get_entity().bounding_box.load().intersects(aabb))
            .cloned()
            .collect()
    }

    // Gets all Player entities at a Box
    pub fn get_players_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<Player>> {
        let players_guard = self.players.load();
        players_guard
            .iter()
            .filter(|player| player.get_entity().bounding_box.load().intersects(aabb))
            .cloned()
            .collect()
    }

    /// Retrieves a player by their unique UUID.
    ///
    /// This function searches the world's active player list for a player with the specified UUID.
    /// If found, it returns an `Arc<Player>` reference to the player. Otherwise, it returns `None`.
    ///
    /// # Arguments
    ///
    /// * `id`: The UUID of the player to retrieve.
    ///
    /// # Returns
    ///
    /// An `Option<Arc<Player>>` containing the player if found, or `None` if not.
    pub fn get_player_by_uuid(&self, id: uuid::Uuid) -> Option<Arc<Player>> {
        self.players
            .load()
            .iter()
            .find(|p| p.gameprofile.id == id)
            .cloned()
    }

    /// Retrieves an entity by their unique UUID.
    ///
    /// This function searches the world's entities for one with the specified UUID.
    /// If found, it returns an `Arc<dyn EntityBase>` reference to that entity. Otherwise, it returns `None`.
    ///
    /// # Arguments
    ///
    /// * `id`: The UUID of the entity to retrieve.
    ///
    /// # Returns
    ///
    /// An `Option<Arc<dyn EntityBase>>` containing the player if found, or `None` if not.
    pub fn get_entity_by_uuid(&self, id: uuid::Uuid) -> Option<Arc<dyn EntityBase>> {
        self.entities
            .load()
            .iter()
            .find(|p| p.get_entity().entity_uuid == id)
            .cloned()
    }

    /// ServerLevel.getEntityInAnyDimension: current world first, then server order.
    pub fn get_entity_in_any_dimension(&self, uuid: uuid::Uuid) -> Option<Arc<dyn EntityBase>> {
        let in_world = |world: &World| {
            world.get_entity_by_uuid(uuid).or_else(|| {
                world
                    .get_player_by_uuid(uuid)
                    .map(|player| player as Arc<dyn EntityBase>)
            })
        };
        if let Some(entity) = in_world(self) {
            return Some(entity);
        }
        let server = self.server.upgrade()?;
        for world in server.worlds.load().iter() {
            if std::ptr::eq(self, world.as_ref()) {
                continue;
            }
            if let Some(entity) = in_world(world) {
                return Some(entity);
            }
        }
        None
    }

    /// Gets a list of players whose location equals the given position in the world.
    ///
    /// It iterates through the players in the world and checks their location. If the player's location matches the
    /// given position, it will add this to a `Vec` which it later returns. If no
    /// player was found in that position, it will just return an empty `Vec`.
    ///
    /// # Arguments
    ///
    /// * `position`: The position the function will check.
    pub fn get_players_by_pos(&self, position: BlockPos) -> Vec<Arc<Player>> {
        self.players
            .load()
            .iter()
            .filter_map(|player| {
                let player_block_pos = player.get_entity().block_pos.load().0;
                (position.0.x == player_block_pos.x
                    && position.0.y == player_block_pos.y
                    && position.0.z == player_block_pos.z)
                    .then(|| Arc::clone(player))
            })
            .collect::<_>()
    }

    /// Gets the nearby players around a given world position.
    /// It "creates" a sphere and checks if whether players are inside
    /// and returns a `HashMap` where the UUID is the key and the `Player`
    /// object is the value.
    ///
    /// # Arguments
    /// * `pos`: The center of the sphere.
    /// * `radius`: The radius of the sphere. The higher the radius, the more area will be checked (in every direction).
    pub fn get_nearby_players(&self, pos: Vector3<f64>, radius: f64) -> Vec<Arc<Player>> {
        let radius_squared = radius.powi(2);

        self.players
            .load()
            .iter()
            .filter_map(|player| {
                let player_pos = player.get_entity().pos.load();
                (player_pos.squared_distance_to_vec(&pos) <= radius_squared).then(|| player.clone())
            })
            .collect()
    }

    pub fn get_nearby_entities(
        &self,
        pos: Vector3<f64>,
        radius: f64,
    ) -> HashMap<uuid::Uuid, Arc<dyn EntityBase>> {
        let radius_squared = radius.powi(2);

        self.entities
            .load()
            .iter()
            .filter_map(|entity| {
                let entity_pos = entity.get_entity().pos.load();
                (entity_pos.squared_distance_to_vec(&pos) <= radius_squared)
                    .then(|| (entity.get_entity().entity_uuid, entity.clone()))
            })
            .collect()
    }

    pub fn get_closest_player(&self, pos: Vector3<f64>, radius: f64) -> Option<Arc<Player>> {
        let players = self.get_nearby_players(pos, radius);
        players
            .iter()
            .min_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&pos)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&pos))
            })
            .cloned()
    }

    /// Gets the closest entity to a position, with optional filtering by entity type.
    ///
    /// # Arguments
    ///
    /// * `pos` - The position to search around.
    /// * `radius` - The radius to search within.
    /// * `entity_types` - Optional array of entity types to filter by. If None, all entity types are included.
    ///
    /// # Returns
    ///
    /// The closest entity that matches the filter criteria, or None if no entities are found.
    pub fn get_closest_entity(
        &self,
        pos: Vector3<f64>,
        radius: f64,
        entity_types: Option<&[&'static EntityType]>,
    ) -> Option<Arc<dyn EntityBase>> {
        // Get regular entities
        let entities = self.get_nearby_entities(pos, radius);

        // Filter by entity type if specified
        let filtered_entities = if let Some(types) = entity_types {
            entities
                .into_iter()
                .filter(|(_, entity)| {
                    let entity_type = entity.get_entity().entity_type;
                    types.contains(&entity_type)
                })
                .collect::<HashMap<_, _>>()
        } else {
            entities
        };

        // Find the closest entity
        filtered_entities
            .iter()
            .min_by(|a, b| {
                a.1.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&pos)
                    .total_cmp(&b.1.get_entity().pos.load().squared_distance_to_vec(&pos))
            })
            .map(|p| p.1.clone())
    }

    /// Adds entities to the provided [`Vec`] that satisfy a particular condition and are
    /// present in the provided [`BoundingBox`].
    ///
    /// # Arguments
    ///
    /// * `list`: The `Vec` to add to.
    /// * `max_list_capacity`: The maximum capacity of `list` for adding entities. If this limit is reached, no more
    ///   entities will be added to the list. If `list` already reaches this limit, nothing happens.
    /// * `bounding_box`: The bounding box to filter any added entities.
    /// * `predicate`: A predicate function, which has to be `true` for an entity to be added to the list.
    pub fn extend_entities_in_box_where(
        &self,
        list: &mut Vec<Arc<dyn EntityBase>>,
        max_list_capacity: usize,
        bounding_box: BoundingBox,
        predicate: impl Fn(&dyn EntityBase) -> bool,
    ) {
        self.extend_entities_where(list, max_list_capacity, |e| {
            bounding_box.intersects(&e.get_entity().bounding_box.load()) && predicate(e)
        });
    }

    /// Adds entities to the provided [`Vec`] that satisfy a particular condition.
    ///
    /// # Arguments
    ///
    /// * `list`: The `Vec` to add to.
    /// * `max_list_capacity`: The maximum capacity of `list` for adding entities. If this limit is reached, no more
    ///   entities will be added to the list. If `list` already reaches this limit, nothing happens.
    /// * `predicate`: A predicate function, which has to be `true` for an entity to be added to the list.
    pub fn extend_entities_where(
        &self,
        list: &mut Vec<Arc<dyn EntityBase>>,
        max_list_capacity: usize,
        predicate: impl Fn(&dyn EntityBase) -> bool,
    ) {
        if list.len() >= max_list_capacity {
            return;
        }
        // Loop the players.
        for player in self.players.load().iter() {
            if !predicate(player.as_ref()) {
                continue;
            }
            // We add the player to the list.
            list.push(player.clone());
            // Check if the list is too big.
            if list.len() > max_list_capacity {
                return;
            }
        }
        // Same with entities.
        for entity in self.entities.load().iter() {
            if !predicate(entity.as_ref()) {
                continue;
            }
            list.push(entity.clone());
            if list.len() > max_list_capacity {
                return;
            }
            // TODO: Implement ender dragon handling
        }
    }

    /// Adds a player to the world and broadcasts a join message if enabled.
    ///
    /// This function takes a player's UUID and an `Arc<Player>` reference.
    /// It inserts the player into the world's `current_players` map using the UUID as the key.
    /// Additionally, it broadcasts a join message to all connected players in the world.
    ///
    /// # Arguments
    ///
    /// * `player`: An `Arc<Player>` reference to the player object.
    pub fn add_player(&self, player: &Arc<Player>) -> Result<(), String> {
        self.players.rcu(|current_list| {
            let mut new_list = (**current_list).clone();
            new_list.push(player.clone());
            new_list
        });
        self.entity_tracker
            .add_entity(&(player.clone() as Arc<dyn EntityBase>), self);
        Ok(())
    }

    /// Must only be called after the player's own `CLogin` packet has been sent.
    pub fn pair_new_player_with_tracked_entities(&self, player: &Arc<Player>) {
        self.entity_tracker
            .pair_new_player_with_tracked_entities(player, self);
    }

    /// Removes a player from the world and broadcasts a disconnect message if enabled.
    ///
    /// This function removes a player from the world based on their `Player` reference.
    /// It performs the following actions:
    ///
    /// 1. Removes the player from the `current_players` map using their UUID.
    /// 2. Broadcasts a `CRemovePlayerInfo` packet to all connected players to inform them about the player leaving.
    /// 3. Removes the player's entity from the world using its entity ID.
    /// 4. Optionally sends a disconnect message to all other players notifying them about the player leaving.
    ///
    /// # Arguments
    ///
    /// * `player`: A reference to the `Player` object to be removed.
    /// * `fire_event`: A boolean flag indicating whether to fire a `PlayerLeaveEvent` event.
    ///
    /// # Notes
    ///
    /// - This function assumes `broadcast_packet_expect` and `remove_entity` are defined elsewhere.
    /// - The disconnect message sending is currently optional. Consider making it a configurable option.
    pub async fn remove_player(
        &self,
        player: &Arc<Player>,
        fire_event: bool,
    ) -> Option<Arc<Player>> {
        let mut removed_player: Option<Arc<Player>> = None;

        self.players.rcu(|current_list| {
            let mut new_list = (**current_list).clone();
            // Find the player before we filter them out
            let pos = new_list
                .iter()
                .position(|p| p.gameprofile.id == player.gameprofile.id);
            if let Some(pos) = pos {
                removed_player = Some(new_list.remove(pos));
            }
            new_list
        });
        if let Some(ref player) = removed_player {
            self.notify_mob_observers_of_removal(
                player.get_entity().entity_id,
                if fire_event {
                    RemovalReason::UnloadedWithPlayer
                } else {
                    RemovalReason::ChangedDimension
                },
            );
            self.entity_tracker
                .remove_entity(player.as_ref() as &dyn EntityBase, self);
            let uuid = player.gameprofile.id;
            let entity_id = player.entity_id();

            let bedrock_remove_player = CPlayerList {
                action: CPlayerList::ACTION_REMOVE,
                entries: vec![PlayerListEntry {
                    uuid,
                    entity_unique_id: VarLong(entity_id as i64),
                    username: player.gameprofile.name.clone(),
                    xuid: String::new(),
                    platform_chat_id: String::new(),
                    build_platform: BuildPlatform::Unknown,
                    skin: Skin::steve(),
                    is_teacher: false,
                    is_host: false,
                    is_sub_client: false,
                    player_color: [0, 0, 0, 0],
                }],
            };

            self.broadcast_editioned(&CRemovePlayerInfo::new(&[uuid]), &bedrock_remove_player);

            self.broadcast_editioned(
                &CRemoveEntities::new(&[entity_id.into()]),
                &CRemoveActor::new(VarLong(entity_id as i64)),
            );

            if fire_event {
                let msg_comp = TextComponent::translate_cross(
                    translation::java::MULTIPLAYER_PLAYER_LEFT,
                    translation::bedrock::MULTIPLAYER_PLAYER_LEFT,
                    [TextComponent::text(player.gameprofile.name.clone())],
                )
                .color_named(NamedColor::Yellow);
                let mut event = PlayerLeaveEvent::new(player.clone(), msg_comp);

                if let Some(server) = self.server.upgrade() {
                    server.plugin_manager.fire(&server, &mut event).await;

                    if !event.cancelled {
                        for player in self.players.load().iter() {
                            player.send_system_message(&event.leave_message);
                        }
                        info!("{}", event.leave_message.to_pretty_console());
                    }
                }
            }
        }
        removed_player
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn spawn_entity_non_save(&self, entity: Arc<dyn EntityBase>) {
        self.spawn_state.load().add_entity(self, entity.as_ref());
        self.insert_entity_non_save(entity);
    }

    /// NaturalSpawner separately accounts for an accepted spawn in its tick state.
    fn insert_entity_non_save(&self, entity: Arc<dyn EntityBase>) {
        self.entity_tracker.add_entity(&entity, self);

        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.push(entity.clone());
            new_entities
        });
    }

    pub fn spawn_entity(self: &Arc<Self>, entity: Arc<dyn EntityBase>) {
        self.spawn_entity_inner(entity, true);
    }

    /// Insert a mob loaded from NBT or already finalized by its spawn source.
    pub fn spawn_initialized_entity(self: &Arc<Self>, entity: Arc<dyn EntityBase>) {
        self.spawn_entity_inner(entity, false);
    }

    fn spawn_entity_inner(self: &Arc<Self>, entity: Arc<dyn EntityBase>, equip: bool) {
        let mut event = crate::plugin::api::events::entity::entity_spawn::EntitySpawnEvent::new(
            entity.get_entity().entity_id,
            entity.get_entity().entity_type.id.to_string(),
            entity.get_entity().pos.load(),
            self.clone(),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        entity.init_data_tracker();
        if equip {
            crate::entity::spawn::initialize_spawn_equipment(entity.as_ref());
        }
        self.add_entity_silent(entity);
    }

    pub fn broadcast_entity_spawn(&self, entity: &Arc<dyn EntityBase>) {
        let base_entity = entity.get_entity();
        let chunk_pos = base_entity.chunk_pos.load();

        let players = self.players.load();
        for player in players.iter() {
            if player
                .watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
            {
                player.client.try_enqueue_spawn_packet(entity);
            }
        }
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn add_entity_silent(&self, entity: Arc<dyn EntityBase>) {
        let base_entity = entity.get_entity();

        // Guard against duplicate entities with the same UUID.
        // This can happen when chunk entity data is loaded while the entity
        // already exists in the world (e.g. another player is still tracking it).
        let already_exists = self
            .entities
            .load()
            .iter()
            .any(|e| e.get_entity().entity_uuid == base_entity.entity_uuid);
        if already_exists {
            return;
        }

        // The entity stays live-only: it is written to its chunk's saved data on
        // unload (see `save_entity`), never at spawn, so it can't be both live and
        // serialized at once (which would double it on the next reload).
        self.spawn_state.load().add_entity(self, entity.as_ref());
        self.entity_tracker.add_entity(&entity, self);

        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.push(entity.clone());
            new_entities
        });
    }

    fn notify_mob_observers_of_removal(&self, id: i32, reason: RemovalReason) {
        for observer in self.entities.load().iter() {
            if let Some(mob) = observer.get_mob() {
                mob.mob_observe_removal(id, reason);
            }
        }
    }

    pub fn remove_entity(&self, entity: &dyn EntityBase) {
        self.remove_entity_with_reason(entity, RemovalReason::Discarded);
    }

    pub(crate) fn remove_entity_with_reason(
        &self,
        entity: &dyn EntityBase,
        reason: RemovalReason,
    ) -> bool {
        let base_entity = entity.get_entity();
        if base_entity
            .removal_reason
            .compare_exchange(None, Some(reason))
            .is_err()
        {
            return false;
        }
        base_entity.removed.store(true, Ordering::Release);

        self.notify_mob_observers_of_removal(base_entity.entity_id, reason);
        if reason == RemovalReason::ChangedDimension {
            base_entity
                .leashed_to
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }

        self.spawn_state.load().remove_entity(self, entity);
        self.entity_tracker.remove_entity(entity, self);
        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.retain(|e| e.get_entity().entity_uuid != base_entity.entity_uuid);
            new_entities
        });
        true
    }

    pub async fn remove_entities_in_chunks(
        &self,
        chunks: impl IntoIterator<Item = impl std::borrow::Borrow<Vector2<i32>>>,
    ) {
        let chunks_set: FxHashSet<_> = chunks.into_iter().map(|c| *c.borrow()).collect();
        if chunks_set.is_empty() {
            return;
        }
        let mut entities_to_remove = Vec::new();

        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.retain(|entity| {
                let base_entity = entity.get_entity();
                let pos = base_entity.chunk_pos.load();
                if chunks_set.contains(&pos) {
                    entities_to_remove.push(entity.clone());
                    false
                } else {
                    true
                }
            });
            new_entities
        });

        for entity in entities_to_remove {
            self.notify_mob_observers_of_removal(
                entity.get_entity().entity_id,
                RemovalReason::UnloadedToChunk,
            );
            self.entity_tracker.remove_entity(entity.as_ref(), self);
            self.save_entity(&entity).await;
            self.spawn_state.load().remove_entity(self, entity.as_ref());
        }

        for chunk_pos in &chunks_set {
            self.save_block_entities(*chunk_pos);
            if let Some((_, entities)) = self.block_entities.remove(chunk_pos) {
                for entity in entities.into_values() {
                    entity.set_removed();
                }
            }
        }
    }

    pub(crate) fn set_block_breaking(
        &self,
        from: &Entity,
        location: BlockPos,
        progress: BlockBreakingProgress,
    ) {
        let chunk_pos = location.chunk_position(); // pumpkin's BlockPos already has this method
        let (stage, bedrock_event) = match progress {
            BlockBreakingProgress::Start { stage, speed } => (
                stage,
                Some((
                    LevelEvent::BlockStartBreak,
                    bedrock_block_breaking_rate(speed),
                )),
            ),
            BlockBreakingProgress::Update { stage, speed } => (
                stage,
                speed.map(|speed| {
                    (
                        LevelEvent::BlockUpdateBreak,
                        bedrock_block_breaking_rate(speed),
                    )
                }),
            ),
            BlockBreakingProgress::Stop => (-1, Some((LevelEvent::BlockStopBreak, 0))),
        };
        let je_packet = CSetBlockDestroyStage::new(from.entity_id.into(), location, stage as i8);

        if let Some((event_id, data)) = bedrock_event {
            let be_packet = CLevelEvent {
                event_id: VarInt(event_id as i32),
                position: Vector3::new(
                    location.0.x as f32,
                    location.0.y as f32,
                    location.0.z as f32,
                ),
                data: VarInt(data),
            };

            if let Some(player) = self.get_player_by_uuid(from.entity_uuid)
                && let ClientPlatform::Bedrock(client) = player.client.as_ref()
                && let Ok(packet_data) = client.serialize_packet(&be_packet)
            {
                client.try_enqueue_packet(packet_data);
            }

            self.broadcast_to_chunk_except_editioned(
                chunk_pos,
                &[from.entity_uuid],
                &je_packet,
                &be_packet,
            );
        } else {
            self.broadcast_to_chunk_except(chunk_pos, &[from.entity_uuid], &je_packet);
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn set_block_state(
        self: &Arc<Self>,
        position: &BlockPos,
        block_state_id: BlockStateId,
        flags: BlockFlags,
    ) -> BlockStateId {
        self.set_block_state_with_limit(
            position,
            block_state_id,
            flags,
            neighbor_updater::DEFAULT_UPDATE_LIMIT,
        )
    }

    #[expect(clippy::too_many_lines)]
    pub fn set_block_state_with_limit(
        self: &Arc<Self>,
        position: &BlockPos,
        block_state_id: BlockStateId,
        flags: BlockFlags,
        update_limit: u32,
    ) -> BlockStateId {
        if !self.is_in_build_limit(*position) {
            return Block::AIR.default_state.id;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        let replaced_block_state_id = self
            .level
            .read_chunk_sync(&chunk_coordinate, |chunk| {
                let replaced_block_state_id = chunk.set_block_absolute_y(
                    relative.x as usize,
                    relative.y,
                    relative.z as usize,
                    block_state_id,
                );
                // Mark chunk dirty if it isn't already
                if replaced_block_state_id != block_state_id && !chunk.is_dirty() {
                    chunk.mark_dirty(true);
                }
                replaced_block_state_id
            })
            .unwrap_or(Block::AIR.default_state.id);

        if !flags.contains(BlockFlags::FORCE_STATE) && replaced_block_state_id == block_state_id {
            return block_state_id;
        }

        let old_block = Block::from_state_id(replaced_block_state_id);
        let new_block = Block::from_state_id(block_state_id);
        let is_new_block = old_block != new_block;
        let block_moved = flags.contains(BlockFlags::MOVED);

        // CopperChestBlock.shouldChangedStateKeepBlockEntity: scraping, waxing
        // and oxidation change the block id without destroying its inventory.
        let keep_copper_chest = old_block
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS)
            && new_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS);
        if is_new_block
            && !keep_copper_chest
            && old_block.default_state.block_entity_type != u16::MAX
            && let Some(entity) = self.get_block_entity(position)
        {
            if !flags.contains(BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK) {
                entity.on_block_replaced_with_state(self, position, replaced_block_state_id);
            }
            self.remove_block_entity(position);
        }

        if is_new_block && (flags.contains(BlockFlags::NOTIFY_NEIGHBORS) || block_moved) {
            self.block_registry.on_state_replaced(
                self,
                old_block,
                position,
                replaced_block_state_id,
                block_moved,
            );
        }

        if !flags.contains(BlockFlags::SKIP_BLOCK_ADDED_CALLBACK) {
            self.block_registry.on_placed(
                self,
                new_block,
                block_state_id,
                position,
                replaced_block_state_id,
                block_moved,
            );
            if is_new_block {
                let new_fluid = self.get_fluid(position);
                self.block_registry.on_placed_fluid(
                    self,
                    new_fluid,
                    block_state_id,
                    position,
                    replaced_block_state_id,
                    block_moved,
                );
            }
        }

        // Level.java setBlock
        if self.get_block_state_id(position) == block_state_id {
            if flags.contains(BlockFlags::NOTIFY_LISTENERS) {
                self.unsent_block_changes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(*position, block_state_id);
            }

            if flags.contains(BlockFlags::NOTIFY_NEIGHBORS) {
                self.update_neighbors_at(position, old_block, None);
                if block_state_id.has_analog_output_signal() {
                    self.update_neighbour_for_output_signal(position, new_block);
                }
            }

            // Level.setBlock propagates shapes for moved blocks too. Java's
            // propagation mask clears neighbor notifications and drop suppression.
            if !flags.contains(BlockFlags::SKIP_SHAPE_UPDATES) && update_limit > 0 {
                let mut neighbour_update_flags = flags;
                neighbour_update_flags.remove(BlockFlags::NOTIFY_NEIGHBORS | BlockFlags::SKIP_DROPS);
                let next_limit = update_limit.saturating_sub(1);
                self.block_registry.prepare(
                    self,
                    position,
                    old_block,
                    replaced_block_state_id,
                    neighbour_update_flags,
                );
                self.block_registry.update_neighbors_with_limit(
                    self,
                    position,
                    neighbour_update_flags,
                    next_limit,
                );
                self.block_registry.prepare(
                    self,
                    position,
                    new_block,
                    block_state_id,
                    neighbour_update_flags,
                );
            }

            self.villager_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .update_block(*position, new_block);

            if is_new_block {
                let mut poi = self
                    .portal_poi
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if villager_poi::profession_for_block(old_block).is_some() {
                    poi.remove(position);
                }
                if let Some(poi_type) = villager_poi::poi_type_for_block(new_block) {
                    poi.add_with_free_tickets(*position, poi_type, 1);
                }
            }
        }

        let old_state = replaced_block_state_id.to_state();
        let new_state = block_state_id.to_state();
        if pumpkin_world::lighting::LightEngine::has_different_light_properties(
            old_state, new_state,
        ) {
            self.level
                .light_engine
                .update_lighting_at(&self.level, *position);
        }

        replaced_block_state_id
    }

    pub fn break_block(
        self: &Arc<Self>,
        position: &BlockPos,
        cause: Option<&Arc<Player>>,
        flags: BlockFlags,
    ) -> Option<BlockStateId> {
        self.break_block_with_limit(
            position,
            cause,
            flags,
            neighbor_updater::DEFAULT_UPDATE_LIMIT,
        )
    }

    pub fn break_block_with_limit(
        self: &Arc<Self>,
        position: &BlockPos,
        cause: Option<&Arc<Player>>,
        flags: BlockFlags,
        update_limit: u32,
    ) -> Option<BlockStateId> {
        self.break_block_with_entity(
            position,
            cause,
            cause.map(|p| p.as_ref() as &dyn EntityBase),
            flags,
            update_limit,
        )
    }

    /// Non-player block destruction keeps its entity in loot and game-event context.
    pub fn break_block_from_entity(
        self: &Arc<Self>,
        position: &BlockPos,
        source: &dyn EntityBase,
        flags: BlockFlags,
    ) -> Option<BlockStateId> {
        self.break_block_with_entity(
            position,
            None,
            Some(source),
            flags,
            neighbor_updater::DEFAULT_UPDATE_LIMIT,
        )
    }

    fn break_block_with_entity(
        self: &Arc<Self>,
        position: &BlockPos,
        cause: Option<&Arc<Player>>,
        source: Option<&dyn EntityBase>,
        flags: BlockFlags,
        update_limit: u32,
    ) -> Option<BlockStateId> {
        if let Some(player) = cause
            && self.is_in_spawn_protection(player, position)
        {
            player.send_system_message(&TextComponent::translate_cross(
                pumpkin_data::translation::java::BUILD_SPAWN_PROTECTION,
                pumpkin_data::translation::java::BUILD_SPAWN_PROTECTION,
                [TextComponent::text(player.gameprofile.name.clone())],
            ));
            return None;
        }

        let (broken_block, broken_block_state) = self.get_block_and_state(position);
        if broken_block_state.is_air() {
            return None;
        }

        if let Some(server) = self.server.upgrade() {
            if let Some(player) = cause {
                if !crate::local_safety::allow_block_action(player, &server, broken_block) {
                    player.reset_block_change(*position);
                    return None;
                }
            } else if crate::local_safety::shulker_box_disabled(
                &server.advanced_config.local_safety,
                broken_block,
            ) {
                return None;
            }
        }

        let mut event = BlockBreakEvent::new(
            cause.cloned(),
            broken_block,
            *position,
            0,
            !flags.contains(BlockFlags::SKIP_DROPS),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return None;
        }

        let mut flags = flags;
        if event.drop {
            flags.remove(BlockFlags::SKIP_DROPS);
        } else {
            flags.insert(BlockFlags::SKIP_DROPS);
        }

        if let Some(player) = cause
            && let Some(server) = self.server.upgrade()
            && let Some(behaviour) = self.block_registry.get_pumpkin_block(broken_block.id)
        {
            behaviour.player_will_destroy(crate::block::BrokenArgs {
                block: broken_block,
                player,
                position,
                server: &server,
                world: self,
                state: broken_block_state,
            });
        }

        // playerWillDestroy may change properties (for example cracking a pot).
        // Loot, break effects and replacement fluid must see that resulting state.
        let current = self.get_block_state(position);
        let broken_block_state = if current.id.to_block() == broken_block {
            current
        } else {
            broken_block_state
        };

        // Keep the old hive available after set_block_state removes its live BE.
        let mined_hive =
            if cause.is_some() && matches!(broken_block.id, BlockId::BEEHIVE | BlockId::BEE_NEST) {
                self.get_block_entity(position)
            } else {
                None
            };
        if let (Some(player), Some(entity)) = (cause, mined_hive.as_ref())
            && let Some(hive) = entity
                .as_any()
                .downcast_ref::<crate::block::entities::beehive::BeehiveBlockEntity>()
        {
            crate::block::blocks::beehive::drop_creative_hive(
                self,
                position,
                broken_block_state,
                player,
                hive,
            );
        }

        if !flags.contains(BlockFlags::SKIP_DROPS) {
            let tool = cause.as_ref().and_then(|p| {
                let item = p.inventory().held_item();
                if item.is_empty() { None } else { Some(item) }
            });
            let params = crate::world::loot::LootContextParameters {
                tool,
                block_state: Some(broken_block_state),
                position: Some(position.to_centered_f64()),
                this_entity: source.map(|source| source.get_entity().entity_type),
                killed_by_player: Some(cause.is_some()),
                ..Default::default()
            };
            crate::block::drop_loot(self, broken_block, position, true, &params);
        }

        let (_, remaining_fluid) = Self::fluid_state_from_block_state(broken_block_state.id);
        let new_state_id = if remaining_fluid.is_empty {
            Block::AIR.default_state.id
        } else {
            remaining_fluid.block_state_id
        };

        let broken_state_id =
            self.set_block_state_with_limit(position, new_state_id, flags, update_limit);
        if let (Some(player), Some(entity)) = (cause, mined_hive)
            && let Some(hive) = entity
                .as_any()
                .downcast_ref::<crate::block::entities::beehive::BeehiveBlockEntity>()
        {
            crate::block::blocks::beehive::player_destroyed(
                self,
                position,
                broken_block_state,
                player,
                hive,
            );
        }
        let broken_block = Block::from_state_id(broken_state_id);
        if !broken_block.is_air()
            && broken_state_id != new_state_id
            && broken_block != &Block::FIRE
            && broken_block != &Block::SOUL_FIRE
        {
            let je_packet = CWorldEvent::new(
                WorldEvent::ParticlesDestroyBlock as i32,
                *position,
                broken_state_id.as_u16().into(),
                false,
            );
            let be_packet = CLevelEvent {
                event_id: VarInt(LevelEvent::ParticlesDestroyBlock as i32),
                position: position.to_centered_f64().to_f32_lossy(),
                data: VarInt(BlockState::to_be_network_id(broken_state_id).into()),
            };
            let chunk_pos = position.chunk_position();
            if let Some(player) = cause {
                // Java predicts its own break effect; Bedrock needs the server event.
                if let ClientPlatform::Bedrock(client) = player.client.as_ref() {
                    client.try_enqueue_client_packet(&be_packet);
                }
                self.broadcast_to_chunk_except_editioned(
                    chunk_pos,
                    &[player.get_entity().entity_uuid],
                    &je_packet,
                    &be_packet,
                );
            } else {
                self.broadcast_to_chunk_editioned(chunk_pos, &je_packet, &be_packet);
            }
        }

        if broken_state_id != new_state_id {
            self.emit_game_event_from_entity(
                "block_destroy",
                position.to_centered_f64(),
                source,
                Some(broken_state_id),
            );
        }
        Some(broken_state_id)
    }

    #[must_use]
    pub const fn environment_attributes(&self) -> EnvironmentAttributes<'_> {
        EnvironmentAttributes::new(self)
    }

    #[must_use]
    pub fn get_sky_darken(&self) -> i32 {
        let sky_light_level = self.environment_attributes().get_dimension_value_f32(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplaySkyLightLevel,
        );
        (15.0 - sky_light_level).clamp(0.0, 15.0) as i32
    }

    #[must_use]
    pub fn is_bright_outside(&self) -> bool {
        !self.dimension.has_fixed_time && self.get_sky_darken() < 4
    }

    #[must_use]
    pub fn is_dark_outside(&self) -> bool {
        !self.dimension.has_fixed_time && !self.is_bright_outside()
    }

    /// Checks if daylight burns undead monsters (`EnvironmentAttributes.MONSTERS_BURN`).
    #[must_use]
    pub fn monsters_burn(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayMonstersBurn,
            pos,
        )
    }

    /// Checks if bees should stay inside beehives/nests (`EnvironmentAttributes.BEES_STAY_IN_HIVE`).
    #[must_use]
    pub fn bees_stay_in_hive(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayBeesStayInHive,
            pos,
        )
    }

    /// Checks if a creaking heart is active (`EnvironmentAttributes.CREAKING_ACTIVE`).
    #[must_use]
    pub fn creaking_active(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayCreakingActive,
            pos,
        )
    }

    #[must_use]
    pub fn respawn_anchor_works(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayRespawnAnchorWorks,
            pos,
        )
    }

    /// Checks if an eyeblossom flower should be open (`EnvironmentAttributes.EYEBLOSSOM_OPEN`).
    #[must_use]
    pub fn eyeblossom_open(&self, pos: &BlockPos) -> Option<bool> {
        self.environment_attributes().get_value_tri_state(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayEyeblossomOpen,
            pos,
        )
    }

    #[must_use]
    pub fn get_effective_sky_brightness(&self, pos: &BlockPos) -> i32 {
        let sky_light = self.get_sky_light_level(pos) as i32;
        sky_light - self.get_sky_darken()
    }

    #[must_use]
    pub fn get_sun_angle(&self, pos: &BlockPos) -> f32 {
        let sun_angle_deg = self.environment_attributes().get_value_f32(
            pumpkin_data::environment_attribute::EnvironmentAttribute::VisualSunAngle,
            pos,
        );
        sun_angle_deg * (std::f32::consts::PI / 180.0)
    }

    #[must_use]
    pub fn get_moon_phase(&self) -> MoonPhase {
        self.environment_attributes()
            .get_dimension_value_moon_phase()
    }

    #[must_use]
    pub fn can_pillager_patrol_spawn(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayCanPillagerPatrolSpawn,
            pos,
        )
    }

    #[must_use]
    pub fn surface_slime_spawn_chance(&self, pos: &BlockPos) -> f32 {
        self.environment_attributes().get_value_f32(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplaySurfaceSlimeSpawnChance,
            pos,
        )
    }

    #[must_use]
    pub fn villager_activity(&self, pos: &BlockPos, baby: bool) -> Activity {
        self.environment_attributes().get_value_activity(baby, pos)
    }

    pub fn get_raw_brightness(&self, pos: &BlockPos, sky_darken: u8) -> u8 {
        let sky_light = self.get_sky_light_level(pos).saturating_sub(sky_darken);
        let block_light = self.get_block_light_level(pos).unwrap_or(0);
        sky_light.max(block_light)
    }

    pub fn get_max_local_raw_brightness(&self, pos: &BlockPos) -> u8 {
        self.get_raw_brightness(pos, self.get_sky_darken() as u8)
    }

    pub fn get_block_light_level(&self, position: &BlockPos) -> Option<u8> {
        self.level
            .light_engine
            .get_block_light_level(&self.level, position)
    }

    pub fn get_sky_light_level(&self, position: &BlockPos) -> u8 {
        self.level
            .light_engine
            .get_sky_light_level(&self.level, position)
    }

    #[must_use]
    pub fn can_see_sky(&self, position: &BlockPos) -> bool {
        position.0.y >= self.dimension.min_y
            && position.0.y < self.dimension.min_y + self.dimension.height
            && self.get_sky_light_level(position) >= MAX_LIGHT_LEVEL
    }

    pub fn set_block_light_level(&self, position: &BlockPos, light_level: u8) {
        let _ = self
            .level
            .light_engine
            .set_block_light_level(&self.level, position, light_level);
    }

    pub fn set_sky_light_level(&self, position: &BlockPos, light_level: u8) {
        let _ = self
            .level
            .light_engine
            .set_sky_light_level(&self.level, position, light_level);
    }

    pub fn get_biome(&self, position: &BlockPos) -> &'static Biome {
        let chunk_pos = position.chunk_position();
        if let Some(chunk) = self.level.loaded_chunks.get(&chunk_pos) {
            let id = chunk
                .section
                .get_rough_biome_absolute_y(
                    (position.0.x & 15) as usize,
                    position.0.y,
                    (position.0.z & 15) as usize,
                )
                .unwrap_or(0);
            Biome::from_id(id).unwrap_or(&Biome::PLAINS)
        } else {
            &Biome::PLAINS
        }
    }

    pub fn schedule_block_tick(
        &self,
        block: &Block,
        block_pos: BlockPos,
        delay: u32,
        priority: TickPriority,
    ) {
        self.level
            .schedule_block_tick(block, block_pos, delay, priority);
    }

    pub fn schedule_fluid_tick(
        &self,
        fluid: &Fluid,
        block_pos: BlockPos,
        delay: u32,
        priority: TickPriority,
    ) {
        self.level
            .schedule_fluid_tick(fluid, block_pos, delay, priority);
    }

    pub fn is_block_tick_scheduled(&self, block_pos: &BlockPos, block: &Block) -> bool {
        self.level.is_block_tick_scheduled(block_pos, block)
    }

    pub fn is_fluid_tick_scheduled(&self, block_pos: &BlockPos, fluid: &Fluid) -> bool {
        self.level.is_fluid_tick_scheduled(block_pos, fluid)
    }

    /// Close container screens for all players who have a container open at the given block position.
    pub fn close_container_screens_at(&self, position: &BlockPos) {
        let players = self.players.load();
        for player in players.iter() {
            if player.open_container_pos.load() == Some(*position) {
                player.close_handled_screen();
            }
        }
    }

    pub fn drop_stack(self: &Arc<Self>, pos: &BlockPos, stack: ItemStack) {
        if stack.is_empty() {
            return;
        }

        let half_height = f64::from(EntityType::ITEM.dimension[1]) / 2.0;
        let spawn_pos = {
            let mut r = rand::rng();
            Vector3::new(
                f64::from(pos.0.x) + 0.5 + r.random_range(-0.25..0.25),
                f64::from(pos.0.y) + 0.5 + r.random_range(-0.25..0.25) - half_height,
                f64::from(pos.0.z) + 0.5 + r.random_range(-0.25..0.25),
            )
        };

        self.drop_stack_at(spawn_pos, stack);
    }

    /// Block.popResource draws the position even when block drops are disabled.
    pub fn drop_block_stack(self: &Arc<Self>, pos: &BlockPos, stack: ItemStack) {
        let spawn_pos = Vector3::new(
            f64::from(pos.0.x) + 0.5 + (-0.25 + self.rand_f64() * 0.5),
            f64::from(pos.0.y) + 0.5 + (-0.25 + self.rand_f64() * 0.5)
                - f64::from(EntityType::ITEM.dimension[1]) / 2.0,
            f64::from(pos.0.z) + 0.5 + (-0.25 + self.rand_f64() * 0.5),
        );
        if !stack.is_empty() && self.level_info.load().game_rules.block_drops {
            self.drop_stack_at(spawn_pos, stack);
        }
    }

    pub(crate) fn drop_stack_at(self: &Arc<Self>, spawn_pos: Vector3<f64>, stack: ItemStack) {
        let entity = Entity::new(self.clone(), spawn_pos, &EntityType::ITEM);
        let mut item_event = crate::plugin::api::events::entity::item_spawn::ItemSpawnEvent::new(
            entity.entity_id,
            spawn_pos,
            stack.item.registry_key.to_string(),
        );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut item_event);
        }
        if item_event.cancelled {
            return;
        }

        let item_entity = Arc::new(ItemEntity::new(entity, stack));
        self.spawn_entity(item_entity);
    }

    pub fn drop_stack_from_face(
        self: &Arc<Self>,
        pos: &BlockPos,
        face: BlockDirection,
        stack: ItemStack,
    ) {
        if stack.is_empty() {
            return;
        }

        let offset = face.to_offset();
        let step_x = offset.x;
        let step_y = offset.y;
        let step_z = offset.z;

        let half_width = f64::from(EntityType::ITEM.dimension[0]) / 2.0;
        let half_height = f64::from(EntityType::ITEM.dimension[1]) / 2.0;

        let (spawn_pos, velocity) = {
            let mut r = rand::rng();
            let x = f64::from(pos.0.x)
                + 0.5
                + if step_x == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_x) * (0.5 + half_width)
                };
            let y = f64::from(pos.0.y)
                + 0.5
                + if step_y == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_y) * (0.5 + half_height)
                }
                - half_height;
            let z = f64::from(pos.0.z)
                + 0.5
                + if step_z == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_z) * (0.5 + half_width)
                };

            let delta_x = if step_x == 0 {
                r.random_range(-0.1..0.1)
            } else {
                f64::from(step_x) * 0.1
            };
            let delta_y = if step_y == 0 {
                r.random_range(0.0..0.1)
            } else {
                f64::from(step_y) * 0.1 + 0.1
            };
            let delta_z = if step_z == 0 {
                r.random_range(-0.1..0.1)
            } else {
                f64::from(step_z) * 0.1
            };

            (
                Vector3::new(x, y, z),
                Vector3::new(delta_x, delta_y, delta_z),
            )
        };

        let entity = Entity::new(self.clone(), spawn_pos, &EntityType::ITEM);
        let mut item_event = crate::plugin::api::events::entity::item_spawn::ItemSpawnEvent::new(
            entity.entity_id,
            spawn_pos,
            stack.item.registry_key.to_string(),
        );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut item_event);
        }
        if item_event.cancelled {
            return;
        }

        let item_entity = Arc::new(ItemEntity::new_with_velocity(entity, stack, velocity, 10));
        self.spawn_entity(item_entity);
    }

    pub fn strike_lightning(self: &Arc<Self>, pos: Vector3<f64>, effect_only: bool) {
        use pumpkin_data::entity::EntityType;
        use uuid::Uuid;
        let server_ref = self.server.upgrade();
        if let Some(server_ref) = server_ref {
            let mut event =
                crate::plugin::api::events::world::lightning_strike::LightningStrikeEvent::new(
                    pos,
                    effect_only,
                );
            server_ref
                .plugin_manager
                .fire_blocking(&server_ref, &mut event);
            if event.cancelled {
                return;
            }
        }

        let lightning = crate::entity::r#type::from_type(
            &EntityType::LIGHTNING_BOLT,
            pos,
            self,
            Uuid::new_v4(),
        );

        if let Some(bolt) = lightning
            .cast_any()
            .downcast_ref::<crate::entity::lightning::LightningBoltEntity>()
        {
            bolt.set_visual_only(effect_only);
        }

        self.spawn_entity(lightning);
    }

    /* ItemScatterer.java */
    pub fn scatter_inventory(
        self: &Arc<Self>,
        position: &BlockPos,
        inventory: &Arc<dyn Inventory>,
    ) {
        for i in 0..inventory.size() {
            self.scatter_stack(
                f64::from(position.0.x),
                f64::from(position.0.y),
                f64::from(position.0.z),
                inventory.remove_stack(i),
            );
        }
    }
    pub fn scatter_stack(self: &Arc<Self>, x: f64, y: f64, z: f64, mut stack: ItemStack) {
        const TRIANGULAR_DEVIATION: f64 = 0.114_850_001_711_398_36;

        let width = f64::from(EntityType::ITEM.dimension[0]);
        let half_width = width / 2.0;
        let spawn_area = 1.0 - width;
        let x = x.floor() + self.rand_f64() * spawn_area + half_width;
        let y = y.floor() + self.rand_f64() * spawn_area;
        let z = z.floor() + self.rand_f64() * spawn_area + half_width;
        while !stack.is_empty() {
            let item = stack.split((self.rand_bounded_i32(21) + 10) as u8);
            let entity = Entity::new(self.clone(), Vector3::new(x, y, z), &EntityType::ITEM);
            let triangle = |mode| mode + TRIANGULAR_DEVIATION * (self.rand_f64() - self.rand_f64());
            let velocity = Vector3::new(triangle(0.0), triangle(0.2), triangle(0.0));
            self.spawn_entity(Arc::new(ItemEntity::new_with_velocity(
                entity, item, velocity, 0,
            )));
        }
    }
    /* End ItemScatterer.java */

    pub fn sync_world_event(&self, world_event: WorldEvent, position: BlockPos, data: i32) {
        let chunk_pos = position.chunk_position();
        self.broadcast_to_chunk(
            chunk_pos,
            &CWorldEvent::new(world_event as i32, position, data, false),
        );
    }

    pub fn sync_global_world_event(&self, world_event: WorldEvent, position: BlockPos, data: i32) {
        self.broadcast_packet_all(&CWorldEvent::new(world_event as i32, position, data, true));
    }

    pub fn set_block_destroy_stage(&self, entity_id: i32, location: BlockPos, stage: i8) {
        let chunk_pos = location.chunk_position();
        let packet = CSetBlockDestroyStage::new(entity_id.into(), location, stage);
        self.broadcast_to_chunk(chunk_pos, &packet);
    }
    #[must_use]
    pub fn is_valid(dest: BlockPos) -> bool {
        Self::is_valid_horizontally(dest) && Self::is_valid_vertically(dest.0.y)
    }
    #[must_use]
    pub fn is_valid_horizontally(dest: BlockPos) -> bool {
        // Note: 30_000_000 is not valid, but -30_000_000 is.
        (-30_000_000..30_000_000).contains(&dest.0.x)
            && (-30_000_000..30_000_000).contains(&dest.0.z)
    }
    #[must_use]
    pub fn is_valid_vertically(y: i32) -> bool {
        // Note: 20_000_000 is not valid, but -20_000_000 is.
        (-20_000_000..20_000_000).contains(&y)
    }
    #[must_use]
    pub fn is_in_build_limit(&self, dest: BlockPos) -> bool {
        self.is_in_height_limit(dest.0.y) && Self::is_valid_horizontally(dest)
    }
    #[must_use]
    pub fn is_in_height_limit(&self, y: i32) -> bool {
        (self.get_bottom_y()..=self.get_top_y()).contains(&y)
    }
    pub const fn get_bottom_y(&self) -> i32 {
        self.dimension.min_y
    }
    /// Vanilla `ServerLevel.isHandlingTick()` (`ServerLevel.java:646`).
    pub fn is_handling_tick(&self) -> bool {
        self.handling_tick
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub const fn get_top_y(&self) -> i32 {
        self.dimension.min_y + self.dimension.height - 1
    }
    /// Gets a `Block` from the block registry. Returns `Block::AIR` if the block was not found.
    pub fn get_block(&self, position: &BlockPos) -> &'static Block {
        self.get_block_state_id_if_loaded(position)
            .map_or(&Block::AIR, Block::from_state_id)
    }

    #[must_use]
    pub fn get_block_state_id_if_loaded(&self, position: &BlockPos) -> Option<BlockStateId> {
        if !self.is_in_build_limit(*position) {
            return None;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        self.level.read_chunk_sync(&chunk_coordinate, |chunk| {
            chunk
                .section
                .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
        })?
    }

    #[must_use]
    pub fn get_block_state_if_loaded(&self, position: &BlockPos) -> Option<&'static BlockState> {
        self.get_block_state_id_if_loaded(position)
            .map(BlockState::from_id)
    }

    #[must_use]
    pub fn is_loaded(&self, position: &BlockPos) -> bool {
        self.get_block_state_id_if_loaded(position).is_some()
    }

    fn get_fluid_from_state_id(id: BlockStateId) -> &'static pumpkin_data::fluid::Fluid {
        if let Some(fluid) = Fluid::from_state_id(id) {
            return fluid.to_flowing();
        }
        // These blocks contain source water without a `waterlogged` property.
        if matches!(
            id.to_block_id(),
            pumpkin_data::BlockId::KELP
                | pumpkin_data::BlockId::KELP_PLANT
                | pumpkin_data::BlockId::SEAGRASS
                | pumpkin_data::BlockId::TALL_SEAGRASS
                | pumpkin_data::BlockId::BUBBLE_COLUMN
        ) || id.is_waterlogged()
        {
            &Fluid::FLOWING_WATER
        } else {
            &Fluid::EMPTY
        }
    }

    pub(crate) fn fluid_state_from_block_state(id: BlockStateId) -> (&'static Fluid, FluidState) {
        let fluid = Self::get_fluid_from_state_id(id);
        let source = if fluid.matches_type(&Fluid::WATER) {
            &Fluid::WATER
        } else if fluid.matches_type(&Fluid::LAVA) {
            &Fluid::LAVA
        } else {
            &Fluid::EMPTY
        };
        let mut state = source.states[source.default_state_index as usize].clone();

        if matches!(
            id.to_block_id(),
            pumpkin_data::BlockId::WATER | pumpkin_data::BlockId::LAVA
        ) {
            // LiquidBlock#getFluidState: source, amounts 7..1, then falling amount 8.
            // The fluid family's first state is not the state of the actual block.
            let level =
                pumpkin_data::block_properties::WaterLikeProperties::from_state_id(id).level;
            let amount = if level == 0 || level >= 8 {
                8
            } else {
                8 - level
            };
            state.height = f32::from(amount) / 9.0;
            state.level = i16::from(amount);
            state.is_source = level == 0;
            state.is_still = state.is_source;
            state.falling = level >= 8;
            state.block_state_id = pumpkin_data::block_properties::WaterLikeProperties {
                level: level.min(8),
            }
            .to_state_id(id.to_block());
        }

        // Keep the normalized family used by fluid callbacks, independently of source state.
        (fluid, state)
    }

    pub fn get_fluid(&self, position: &BlockPos) -> &'static pumpkin_data::fluid::Fluid {
        let id = self.get_block_state_id(position);
        Self::get_fluid_from_state_id(id)
    }

    pub fn get_block_and_fluid(
        &self,
        position: &BlockPos,
    ) -> (
        &'static pumpkin_data::Block,
        &'static pumpkin_data::fluid::Fluid,
    ) {
        let id = self.get_block_state_id(position);
        (id.to_block(), Self::get_fluid_from_state_id(id))
    }

    pub fn get_fluid_and_fluid_state(&self, position: &BlockPos) -> (&'static Fluid, FluidState) {
        let id = self.get_block_state_id(position);
        Self::fluid_state_from_block_state(id)
    }

    /// `FluidState#getHeight` includes the full block when the same fluid is above.
    /// Keep `state.height` as the own height used by flow-velocity calculations.
    pub fn get_fluid_height(&self, position: &BlockPos, fluid: &Fluid, state: &FluidState) -> f32 {
        if state.is_empty {
            0.0
        } else if fluid.matches_type(self.get_fluid(&position.up())) {
            1.0
        } else {
            state.height
        }
    }

    pub fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
        self.get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id)
    }

    /// Gets the `BlockState` from the block registry. Returns Air if the block state was not found.
    pub fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
        let id = self.get_block_state_id(position);
        BlockState::from_id(id)
    }

    /// Gets the Block + Block state from the Block Registry, Returns Air if the Block state has not been found
    pub fn get_block_and_state(
        &self,
        position: &BlockPos,
    ) -> (&'static Block, &'static BlockState) {
        let id = self.get_block_state_id(position);
        BlockState::from_id_with_block(id)
    }

    /// Gets the Block + state id from the Block Registry, Returns Air if the Block state has not been found
    pub fn get_block_and_state_id(&self, position: &BlockPos) -> (&'static Block, BlockStateId) {
        let id = self.get_block_state_id(position);
        (Block::from_state_id(id), id)
    }

    /// Updates neighboring blocks of a block with a specified source block
    pub fn update_neighbors_at(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        source_block: &Block,
        except: Option<BlockDirection>,
    ) {
        self.neighbor_updater
            .update_neighbors_at_except_from_facing(self, *block_pos, source_block.id, except);
    }

    /// Runs one queued neighbor notification. Vanilla `NeighborUpdater.executeUpdate`,
    /// reached from both the single-position and the six-direction update, which is why
    /// this handles blocks and fluids for either caller.
    ///
    /// Call this only from the neighbor updater -- going direct skips the queue and
    /// restores the recursion this replaced.
    pub(crate) fn execute_neighbor_update(
        self: &Arc<Self>,
        neighbor_pos: &BlockPos,
        source_block: &Block,
    ) {
        let (neighbor_block, neighbor_fluid) = self.get_block_and_fluid(neighbor_pos);

        let mut event = crate::plugin::api::events::block::block_physics::BlockPhysicsEvent::new(
            *neighbor_pos,
            *neighbor_pos,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        if let Some(neighbor_pumpkin_block) =
            self.block_registry.get_pumpkin_block(neighbor_block.id)
        {
            neighbor_pumpkin_block.on_neighbor_update(OnNeighborUpdateArgs {
                world: self,
                block: neighbor_block,
                position: neighbor_pos,
                source_block,
                notify: false,
            });
        }

        if let Some(neighbor_pumpkin_fluid) =
            self.block_registry.get_pumpkin_fluid(neighbor_fluid.id)
        {
            neighbor_pumpkin_fluid.on_neighbor_update(self, neighbor_fluid, neighbor_pos, false);
        }
    }

    /// Updates neighboring blocks of a block
    pub fn update_neighbors(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        except: Option<BlockDirection>,
    ) {
        let source_block = self.get_block(block_pos);
        self.update_neighbors_at(block_pos, source_block, except);
    }

    pub fn update_neighbor(self: &Arc<Self>, neighbor_block_pos: &BlockPos, source_block: &Block) {
        self.neighbor_updater
            .neighbor_changed(self, *neighbor_block_pos, source_block.id);
    }

    pub fn update_neighbour_for_output_signal(
        self: &Arc<Self>,
        pos: &BlockPos,
        changed_block: &Block,
    ) {
        for direction in BlockDirection::horizontal() {
            let mut relative_pos = pos.offset(direction.to_offset());
            if self.is_loaded(&relative_pos) {
                let state = self.get_block_state(&relative_pos);
                if state.id.to_block() == &Block::COMPARATOR {
                    self.update_neighbor(&relative_pos, changed_block);
                } else if state.is_solid_block() {
                    relative_pos = relative_pos.offset(direction.to_offset());
                    if self.is_loaded(&relative_pos) {
                        let second_state = self.get_block_state(&relative_pos);
                        if second_state.id.to_block() == &Block::COMPARATOR {
                            self.update_neighbor(&relative_pos, changed_block);
                        }
                    }
                }
            }
        }
    }

    /// Sends output-signal updates for block entities changed this tick, in list order.
    fn flush_comparator_updates(self: &Arc<Self>, block_entities: &[Arc<dyn BlockEntity>]) {
        for be in block_entities {
            // Vanilla `BlockEntity.setChanged` -> `Level.updateNeighbourForOutputSignal`.
            if !be.is_comparator_dirty() {
                continue;
            }
            be.clear_comparator_dirty();
            let pos = be.get_position();
            // The list is a snapshot, so the chunk may be gone by now.
            if let Some(state_id) = self.get_block_state_id_if_loaded(&pos) {
                self.update_neighbour_for_output_signal(&pos, state_id.to_block());
            }
        }
    }

    pub fn update_from_neighbor_shapes(
        self: &Arc<Self>,
        state_id: BlockStateId,
        pos: &BlockPos,
    ) -> BlockStateId {
        let mut current_state_id = state_id;
        let block = Block::from_state_id(state_id);
        for direction in BlockDirection::all() {
            let neighbor_pos = pos.offset(direction.to_offset());
            let neighbor_state_id = self.get_block_state_id(&neighbor_pos);
            current_state_id = self.block_registry.get_state_for_neighbor_update(
                self,
                block,
                current_state_id,
                pos,
                direction,
                &neighbor_pos,
                neighbor_state_id,
            );
        }
        current_state_id
    }

    /// Vanilla `Level.neighborShapeChanged` -> `CollectingNeighborUpdater.shapeUpdate`.
    ///
    /// Enqueues a shape update for `block_pos`. Captures `neighbor_state` at enqueue time,
    /// exactly matching vanilla's `neighborShapeChanged` / `CollectingNeighborUpdater.shapeUpdate`.
    pub fn replace_with_state_for_neighbor_update(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        direction: BlockDirection,
        flags: BlockFlags,
    ) {
        self.replace_with_state_for_neighbor_update_with_limit(
            block_pos,
            direction,
            flags,
            neighbor_updater::DEFAULT_UPDATE_LIMIT,
        );
    }

    /// Vanilla `Level.neighborShapeChanged` with explicit `updateLimit`.
    pub fn replace_with_state_for_neighbor_update_with_limit(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        direction: BlockDirection,
        flags: BlockFlags,
        update_limit: u32,
    ) {
        if update_limit == 0 {
            return;
        }

        let neighbor_pos = block_pos.offset(direction.to_offset());
        let neighbor_state_id = self.get_block_state_id(&neighbor_pos);

        self.neighbor_updater.shape_update(
            self,
            direction,
            *block_pos,
            neighbor_pos,
            neighbor_state_id,
            flags,
            update_limit,
        );
    }

    /// Executes one queued shape update with no lock held.
    /// Vanilla `NeighborUpdater.executeShapeUpdate` (`NeighborUpdater.java:36`).
    ///
    /// Call this only from `CollectingNeighborUpdater` -- going direct skips the queue
    /// and breaks ordering with neighbor updates.
    pub(crate) fn execute_shape_update(
        self: &Arc<Self>,
        direction: BlockDirection,
        block_pos: &BlockPos,
        neighbor_pos: &BlockPos,
        neighbor_state_id: BlockStateId,
        flags: BlockFlags,
        update_limit: u32,
    ) {
        if update_limit == 0 {
            return;
        }

        let (block, block_state_id) = self.get_block_and_state_id(block_pos);

        if flags.contains(BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT)
            && *block == Block::REDSTONE_WIRE
        {
            return;
        }

        let new_state_id = self.block_registry.get_state_for_neighbor_update(
            self,
            block,
            block_state_id,
            block_pos,
            direction,
            neighbor_pos,
            neighbor_state_id,
        );

        if new_state_id != block_state_id {
            if is_air(new_state_id) {
                self.break_block_with_limit(
                    block_pos,
                    None,
                    flags | BlockFlags::NOTIFY_ALL,
                    update_limit,
                );
            } else {
                self.set_block_state_with_limit(block_pos, new_state_id, flags, update_limit);
            }
        }
    }

    /// Returns whether monsters can be spawned in the world
    pub fn should_spawn_monsters(&self) -> bool {
        let level_data = self.level_info.load();
        level_data.game_rules.spawn_mobs
            && level_data.game_rules.spawn_monsters
            && level_data.difficulty != Difficulty::Peaceful
    }

    pub fn get_block_entity(&self, block_pos: &BlockPos) -> Option<Arc<dyn BlockEntity>> {
        let chunk_pos = block_pos.chunk_position();
        if let Some(entity) = self
            .block_entities
            .get(&chunk_pos)
            .and_then(|m| m.get(block_pos).cloned())
        {
            return Some(entity);
        }

        let nbt = self
            .level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(block_pos)
                    .cloned()
            })
            .flatten()?;
        if let Some(custom_data) = nbt
            .get_compound("PumpkinCustomData")
            .or_else(|| nbt.get_compound("BukkitValues"))
        {
            self.custom_block_entity_data
                .insert(*block_pos, custom_data.clone());
        }
        let entity = block_entity_from_nbt(&nbt)?;
        entity.set_world(self.self_reference.clone());
        self.block_entities
            .entry(chunk_pos)
            .or_default()
            .insert(*block_pos, entity.clone());
        Some(entity)
    }

    fn bedrock_block_entity_data(
        &self,
        state_id: BlockStateId,
        position: BlockPos,
    ) -> Option<NbtCompound> {
        self.get_block_entity(&position)?
            .bedrock_block_actor_data(state_id)
    }

    /// Builds Bedrock block actor tags that are not represented by Java block states alone.
    pub fn bedrock_chunk_block_actors(&self, chunk: &ChunkData) -> Vec<NbtCompound> {
        let chunk_pos = Vector2::new(chunk.x, chunk.z);
        let live_entities: FxHashMap<_, _> = self
            .block_entities
            .get(&chunk_pos)
            .map(|entities| {
                entities
                    .iter()
                    .map(|(position, entity)| (*position, entity.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let pending = chunk
            .pending_block_entities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        live_entities
            .iter()
            .filter_map(|(position, entity)| {
                let relative = position.chunk_relative_position();
                chunk
                    .section
                    .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
                    .and_then(|state_id| {
                        bedrock_chest_block_actor(state_id, *position)
                            .or_else(|| entity.bedrock_block_actor_data(state_id))
                    })
            })
            .chain(
                pending
                    .iter()
                    .filter(|(position, _)| !live_entities.contains_key(position))
                    .filter_map(|(position, nbt)| {
                        let relative = position.chunk_relative_position();
                        let state_id = chunk.section.get_block_absolute_y(
                            relative.x as usize,
                            relative.y,
                            relative.z as usize,
                        )?;
                        bedrock_chest_block_actor(state_id, *position).or_else(|| {
                            block_entity_from_nbt(nbt)?.bedrock_block_actor_data(state_id)
                        })
                    }),
            )
            .collect()
    }

    pub fn add_block_entity(&self, block_entity: Arc<dyn BlockEntity>) {
        block_entity.set_world(self.self_reference.clone());
        let block_pos = block_entity.get_position();
        let chunk_pos = block_pos.chunk_position();
        let block_entity_nbt = block_entity.chunk_data_nbt();
        let mut full_nbt = NbtCompound::new();
        block_entity.write_internal(&mut full_nbt);

        if let Some(nbt) = &block_entity_nbt {
            let bytes = pumpkin_nbt::Nbt::from(nbt.clone()).write_unnamed();
            self.broadcast_to_chunk(
                chunk_pos,
                &CBlockEntityData::new(
                    block_entity.get_position(),
                    VarInt(block_entity.get_id() as i32),
                    bytes.as_ref().into(),
                ),
            );
        }

        self.block_entities
            .entry(chunk_pos)
            .or_default()
            .insert(block_pos, block_entity);

        self.add_block_entity_nbt(block_pos, &full_nbt);

        self.level.read_chunk_sync(&chunk_pos, |chunk| {
            chunk.mark_dirty(true);
        });
    }

    pub(crate) fn add_block_entity_nbt(&self, block_pos: BlockPos, nbt: &NbtCompound) {
        if self
            .level
            .read_chunk_sync(&block_pos.chunk_position(), |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(block_pos, nbt.clone());
                chunk.mark_dirty(true);
            })
            .is_some()
        {
            self.pending_block_entity_migrations
                .push(block_pos.chunk_position());
        }
    }

    pub fn remove_block_entity(&self, block_pos: &BlockPos) {
        let chunk_pos = block_pos.chunk_position();
        let removed_entity = self
            .block_entities
            .get_mut(&chunk_pos)
            .and_then(|mut entities| entities.remove(block_pos));
        let removed = removed_entity.is_some();
        // Release the chunk map guard before callbacks can access block entities.
        if let Some(entity) = removed_entity {
            entity.set_removed();
        }
        self.custom_block_entity_data.remove(block_pos);
        if removed {
            // Drop the chunk's map once its last block entity is gone.
            self.block_entities
                .remove_if(&chunk_pos, |_, entities| entities.is_empty());
        }
        self.level.read_chunk_sync(&chunk_pos, |chunk| {
            let removed_saved = chunk.remove_pending_block_entity_nbt(block_pos);
            if removed && !removed_saved {
                chunk.mark_dirty(true);
            }
        });
    }

    fn migrate_pending_block_entities(&self, chunk_pos: Vector2<i32>) {
        let positions: Vec<BlockPos> = self
            .level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .keys()
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        for pos in positions {
            let already_loaded = self
                .block_entities
                .get(&chunk_pos)
                .is_some_and(|m| m.contains_key(&pos));
            if !already_loaded && let Some(entity) = self.get_block_entity(&pos) {
                self.update_block_entity(&entity);
            }
        }
    }

    pub fn update_block_entity(&self, block_entity: &Arc<dyn BlockEntity>) {
        let block_pos = block_entity.get_position();
        let chunk_pos = block_pos.chunk_position();
        let block_entity_nbt = block_entity.chunk_data_nbt();

        if let Some(nbt) = &block_entity_nbt {
            let bytes = pumpkin_nbt::Nbt::from(nbt.clone()).write_unnamed();
            self.broadcast_to_chunk(
                chunk_pos,
                &CBlockEntityData::new(
                    block_entity.get_position(),
                    VarInt(block_entity.get_id() as i32),
                    bytes.as_ref().into(),
                ),
            );
        }
        // Update packets may intentionally omit persistent fields such as cooking timers.
        let mut full_nbt = NbtCompound::new();
        block_entity.write_internal(&mut full_nbt);
        self.add_block_entity_nbt(block_pos, &full_nbt);
        self.level.read_chunk_sync(&chunk_pos, |chunk| {
            chunk.mark_dirty(true);
        });
    }

    #[must_use]
    pub fn intersects_aabb_with_hit(
        from: Vector3<f64>,
        to: Vector3<f64>,
        min: Vector3<f64>,
        max: Vector3<f64>,
    ) -> Option<(f64, BlockDirection, Vector3<f64>)> {
        block_ray::clip_aabb(from, to, BoundingBox::new(min, max))
            .map(|(fraction, hit)| (fraction, hit.direction, hit.position))
    }

    pub fn ray_outline_check_detailed(
        &self,
        block_pos: &BlockPos,
        from: Vector3<f64>,
        to: Vector3<f64>,
    ) -> Option<(BlockDirection, Vector3<f64>)> {
        self.clip_block_cell(block_pos, from, to, false, RayFluidHandling::None, None)
            .map(|hit| (hit.direction, hit.position))
    }

    /// Outline and optional fluid clipping with the original segment endpoints.
    pub fn ray_trace_block(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
        include_fluids: bool,
    ) -> Option<(BlockPos, BlockDirection, Vector3<f64>)> {
        let fluids = if include_fluids {
            RayFluidHandling::Any
        } else {
            RayFluidHandling::None
        };
        self.ray_trace_block_with_fluid(start, end, fluids)
            .map(|(pos, hit)| (pos, hit.direction, hit.position))
    }

    pub fn ray_trace_block_with_fluid(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
        fluids: RayFluidHandling,
    ) -> Option<(BlockPos, RayHit)> {
        self.ray_trace_block_with_context(start, end, fluids, false, None)
    }

    /// Shared entity-aware outline/collision ray, also used by explosion exposure.
    pub fn ray_trace_block_with_context(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
        fluids: RayFluidHandling,
        collision: bool,
        entity: Option<&dyn EntityBase>,
    ) -> Option<(BlockPos, RayHit)> {
        block_ray::traverse(start, end, |pos| {
            self.clip_block_cell(&pos, start, end, collision, fluids, entity)
                .map(|hit| (pos, hit))
        })
    }

    /// Java CollisionGetter.clipIncludingBorder, retaining the border marker for
    /// projectile deflection instead of dispatching it as an ordinary block hit.
    pub fn ray_trace_block_including_border(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
        entity: &dyn EntityBase,
    ) -> Option<(BlockPos, RayHit, bool)> {
        let block = self.ray_trace_block_with_context(
            start,
            end,
            RayFluidHandling::None,
            true,
            Some(entity),
        );
        let target = block.map_or(end, |(_, hit)| hit.position);
        let bounds = self
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .bounds();
        if let Some(hit) = block_ray::clip_border(start, target, bounds) {
            let pos = BlockPos::new(
                hit.position.x.floor() as i32,
                hit.position.y.floor() as i32,
                hit.position.z.floor() as i32,
            );
            return Some((pos, hit, true));
        }
        block.map(|(pos, hit)| (pos, hit, false))
    }

    fn clip_block_cell(
        &self,
        pos: &BlockPos,
        from: Vector3<f64>,
        to: Vector3<f64>,
        collision: bool,
        fluids: RayFluidHandling,
        entity: Option<&dyn EntityBase>,
    ) -> Option<RayHit> {
        let state = self.get_block_state(pos);
        let block = state.id.to_block();
        let held = entity
            .filter(|_| !collision && (block == &Block::SCAFFOLDING || block == &Block::LIGHT))
            .and_then(|entity| {
                entity
                    .get_living_entity()
                    .map(|living| living.held_item(entity))
            });
        let holding_outline_item = held.as_ref().is_some_and(|held| {
            !held.is_empty()
                && ((block == &Block::SCAFFOLDING
                    && held.item == &pumpkin_data::item::Item::SCAFFOLDING)
                    || (block == &Block::LIGHT && held.item == &pumpkin_data::item::Item::LIGHT))
        });
        let local_boxes = if collision {
            if let Some(entity) = entity {
                self.block_collision_boxes_for_entity(pos, entity)
            } else {
                self.block_collision_boxes(pos)
            }
        } else if holding_outline_item {
            vec![BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])]
        } else if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_SHULKER_BOXES) {
            self.block_collision_boxes(pos)
        } else {
            state.get_block_outline_shapes_at(pos).collect()
        };
        let offset = pos.0.to_f64();
        let boxes: Vec<_> = local_boxes
            .into_iter()
            .map(|shape| shape.shift(offset))
            .collect();
        let block_hit = block_ray::clip(from, to, &boxes);
        let block_hit = if block_hit.is_some() {
            let interaction: Vec<_> = block_interaction_shapes::boxes(state.id.as_u16())
                .iter()
                .map(|b| {
                    BoundingBox::new_array([b[0], b[1], b[2]], [b[3], b[4], b[5]]).shift(offset)
                })
                .collect();
            block_ray::with_interaction_override(
                from,
                block_hit,
                block_ray::clip(from, to, &interaction),
            )
        } else {
            None
        };
        let (fluid, fluid_state) = Self::fluid_state_from_block_state(state.id);
        let fluid_hit = if fluids.can_pick(fluid, &fluid_state) {
            let height = f64::from(block_ray::fluid_shape_height(
                fluid,
                &fluid_state,
                self.get_fluid_height(pos, fluid, &fluid_state),
            ));
            block_ray::clip(
                from,
                to,
                &[BoundingBox::new(offset, offset.add_raw(1.0, height, 1.0))],
            )
        } else {
            None
        };
        block_ray::nearest(from, block_hit, fluid_hit)
    }

    pub fn ray_trace_entities(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
    ) -> Vec<(Arc<dyn EntityBase>, Vector3<f64>, f64)> {
        if start == end {
            return Vec::new();
        }

        let min_x = start.x.min(end.x) - 1.0;
        let max_x = start.x.max(end.x) + 1.0;
        let min_y = start.y.min(end.y) - 1.0;
        let max_y = start.y.max(end.y) + 1.0;
        let min_z = start.z.min(end.z) - 1.0;
        let max_z = start.z.max(end.z) + 1.0;
        let ray_box = BoundingBox::new(
            Vector3::new(min_x, min_y, min_z),
            Vector3::new(max_x, max_y, max_z),
        );

        let mut hits = Vec::new();

        for entity in self.entities.load().iter() {
            let bb = entity.get_entity().bounding_box.load();
            if bb.intersects(&ray_box)
                && let Some((t, _, hit_pos)) =
                    Self::intersects_aabb_with_hit(start, end, bb.min, bb.max)
            {
                let distance = (hit_pos - start).length();
                hits.push((entity.clone(), hit_pos, distance, t));
            }
        }

        for player in self.players.load().iter() {
            let bb = player.get_entity().bounding_box.load();
            if bb.intersects(&ray_box)
                && let Some((t, _, hit_pos)) =
                    Self::intersects_aabb_with_hit(start, end, bb.min, bb.max)
            {
                let distance = (hit_pos - start).length();
                hits.push((player.clone() as Arc<dyn EntityBase>, hit_pos, distance, t));
            }
        }

        hits.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
        hits.into_iter()
            .map(|(ent, hit_pos, dist, _)| (ent, hit_pos, dist))
            .collect()
    }

    pub fn ray_trace_entity(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
    ) -> Option<(Arc<dyn EntityBase>, Vector3<f64>, f64)> {
        self.ray_trace_entities(start, end).into_iter().next()
    }

    pub fn raycast(
        self: &Arc<Self>,
        start: Vector3<f64>,
        end: Vector3<f64>,
        hit_check: impl Fn(&BlockPos, &Arc<Self>) -> bool,
    ) -> Option<(BlockPos, BlockDirection)> {
        self.raycast_with_shape(start, end, hit_check, false, RayFluidHandling::None)
    }

    pub fn has_line_of_sight(self: &Arc<Self>, start: Vector3<f64>, end: Vector3<f64>) -> bool {
        start.squared_distance_to_vec(&end) <= 128.0 * 128.0
            && self
                .raycast_with_shape(start, end, |_, _| true, true, RayFluidHandling::None)
                .is_none()
    }

    /// Entity.move's FALLDAMAGE_RESETTING / WATER ray, capped by the caller at
    /// eight blocks. Block resetting shapes are full cubes; water keeps its height.
    pub(crate) fn ray_resets_fall_distance(
        &self,
        from: Vector3<f64>,
        to: Vector3<f64>,
        player: bool,
    ) -> bool {
        let instant_portal = player
            && self
                .level_info
                .load()
                .game_rules
                .players_nether_portal_default_delay
                == 0;
        block_ray::traverse(from, to, |pos| {
            let (block, state) = self.get_block_and_state(&pos);
            let resets = block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FALL_DAMAGE_RESETTING)
                || (player && (block == &Block::END_GATEWAY || block == &Block::END_PORTAL))
                || (instant_portal && block == &Block::NETHER_PORTAL);
            let min = pos.0.to_f64();
            if resets
                && block_ray::intersects_box(
                    from,
                    to,
                    BoundingBox::new(min, min.add_raw(1.0, 1.0, 1.0)),
                )
            {
                return Some(());
            }
            let (fluid, fluid_state) = Self::fluid_state_from_block_state(state.id);
            if !fluid_state.is_empty && fluid.matches_type(&Fluid::WATER) {
                let height = f64::from(block_ray::fluid_shape_height(
                    fluid,
                    &fluid_state,
                    self.get_fluid_height(&pos, fluid, &fluid_state),
                ));
                if block_ray::intersects_box(
                    from,
                    to,
                    BoundingBox::new(min, min.add_raw(1.0, height, 1.0)),
                ) {
                    return Some(());
                }
            }
            None
        })
        .is_some()
    }

    /// Falling concrete uses collision shapes and source-fluid surfaces along
    /// its movement segment, so a thin water layer cannot be skipped at speed.
    pub fn raycast_falling_block(
        self: &Arc<Self>,
        start: Vector3<f64>,
        end: Vector3<f64>,
    ) -> Option<BlockPos> {
        self.raycast_with_shape(start, end, |_, _| true, true, RayFluidHandling::Source)
            .map(|(pos, _)| pos)
    }

    fn raycast_with_shape(
        self: &Arc<Self>,
        start: Vector3<f64>,
        end: Vector3<f64>,
        hit_check: impl Fn(&BlockPos, &Arc<Self>) -> bool,
        collision_shape: bool,
        fluids: RayFluidHandling,
    ) -> Option<(BlockPos, BlockDirection)> {
        block_ray::traverse(start, end, |pos| {
            if !hit_check(&pos, self) {
                return None;
            }
            self.clip_block_cell(&pos, start, end, collision_shape, fluids, None)
                .map(|hit| (pos, hit.direction))
        })
    }

    /// Broadcasts a packet to all players who currently have the target chunk loaded.
    pub fn broadcast_to_chunk<P: ClientPacket>(&self, chunk_pos: Vector2<i32>, packet: &P) {
        let players = self.players.load();

        let recipients = players.iter().filter(|p| {
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn broadcast_to_chunk_bedrock<P: BClientPacket>(
        &self,
        chunk_pos: Vector2<i32>,
        packet: &P,
    ) {
        let players = self.players.load();
        let recipients = players.iter().filter_map(|player| {
            if player
                .watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
                && let ClientPlatform::Bedrock(client) = player.client.as_ref()
            {
                return Some(client);
            }
            None
        });
        Self::broadcast_bedrock_grouped(packet, recipients);
    }

    pub fn broadcast_to_chunk_editioned<J: ClientPacket, B: BClientPacket>(
        &self,
        chunk_pos: Vector2<i32>,
        je_packet: &J,
        be_packet: &B,
    ) {
        let players = self.players.load();
        let mut java_recipients = Vec::new();
        let mut bedrock_recipients = Vec::new();

        let recipients = players.iter().filter(|p| {
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        for p in recipients {
            match p.client.as_ref() {
                ClientPlatform::Java(_) => java_recipients.push(p),
                ClientPlatform::Bedrock(be_client) => bedrock_recipients.push(be_client),
            }
        }

        let recipients_by_version =
            Self::collect_java_recipients_by_version(java_recipients.into_iter());
        Self::broadcast_java_grouped(je_packet, recipients_by_version);
        Self::broadcast_bedrock_grouped(be_packet, bedrock_recipients.into_iter());
    }

    /// Broadcasts a packet to chunk watchers, excluding specific players.
    pub fn broadcast_to_chunk_except<P: ClientPacket>(
        &self,
        chunk_pos: Vector2<i32>,
        except: &[uuid::Uuid],
        packet: &P,
    ) {
        let players = self.players.load();

        let recipients = players.iter().filter(|p| {
            if except.contains(&p.get_entity().entity_uuid) {
                return false;
            }
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn broadcast_to_chunk_except_editioned<J: ClientPacket, B: BClientPacket>(
        &self,
        chunk_pos: Vector2<i32>,
        except: &[uuid::Uuid],
        je_packet: &J,
        be_packet: &B,
    ) {
        let players = self.players.load();
        let recipients = players.iter().filter(|p| {
            if except.contains(&p.get_entity().entity_uuid) {
                return false;
            }
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        let mut java_recipients = Vec::new();
        let mut bedrock_recipients = Vec::new();

        for p in recipients {
            match p.client.as_ref() {
                ClientPlatform::Java(_) => java_recipients.push(p),
                ClientPlatform::Bedrock(be_client) => bedrock_recipients.push(be_client),
            }
        }

        let je_recipients_by_version =
            Self::collect_java_recipients_by_version(java_recipients.into_iter());
        Self::broadcast_java_grouped(je_packet, je_recipients_by_version);
        Self::broadcast_bedrock_grouped(be_packet, bedrock_recipients.into_iter());
    }

    pub fn emit_game_event(self: &Arc<Self>, event_key: impl Into<String>, position: Vector3<f64>) {
        self.emit_game_event_with_source(event_key, position, None);
    }

    /// Preserve attribution through sensor activation so shriekers can identify
    /// the responsible player (SculkShriekerBlockEntity.tryGetPlayer).
    pub fn emit_game_event_with_source(
        self: &Arc<Self>,
        event_key: impl Into<String>,
        position: Vector3<f64>,
        source_entity: Option<i32>,
    ) {
        self.emit_game_event_with_context(event_key, position, source_entity, None);
    }

    pub fn emit_game_event_with_context(
        self: &Arc<Self>,
        event_key: impl Into<String>,
        position: Vector3<f64>,
        source_entity: Option<i32>,
        affected_state: Option<BlockStateId>,
    ) {
        let source = source_entity.and_then(|id| self.get_entity_by_id(id));
        self.emit_game_event_from_entity(event_key, position, source.as_deref(), affected_state);
    }

    /// Keep event context valid when an entity has not yet spawned or its hit
    /// effects have already removed it from the world's entity list.
    pub fn emit_game_event_from_entity(
        self: &Arc<Self>,
        event_key: impl Into<String>,
        position: Vector3<f64>,
        source: Option<&dyn EntityBase>,
        affected_state: Option<BlockStateId>,
    ) {
        let source_entity = source.map(|entity| entity.get_entity().entity_id);
        let mut event = crate::plugin::api::events::world::generic_game::GenericGameEvent::new(
            event_key.into(),
            position,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if !event.cancelled {
            let key = event
                .event_key
                .strip_prefix("minecraft:")
                .unwrap_or(&event.event_key);
            if let Some(kind) = pumpkin_data::game_event::GameEvent::from_name(key) {
                if kind == pumpkin_data::game_event::GameEvent::EntityDie {
                    crate::block::entities::sculk_catalyst::dispatch_death(
                        self,
                        event.position,
                        source_entity,
                    );
                }
                // VibrationSystem.User.isValidVibration: affected wool suppresses
                // place/break vibrations even though the event still reaches plugins.
                use pumpkin_data::tag::Taggable;
                if affected_state.is_none_or(|state| {
                    !Block::from_state_id(state)
                        .has_tag(&pumpkin_data::tag::Block::MINECRAFT_DAMPENS_VIBRATIONS)
                }) {
                    crate::block::blocks::sculk::vibration::dispatch_from_entity(
                        self,
                        kind,
                        event.position,
                        source,
                    );
                }
            }
        }
    }

    pub async fn unload(self: &Arc<Self>) {
        let mut event =
            crate::plugin::api::events::world::world_load::WorldUnloadEvent::new(self.clone());
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
    }

    fn save_world_border(&self) {
        if let Err(error) = self
            .worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save(&self.level.level_folder.dim_folder)
        {
            tracing::warn!("Failed to save world border: {error}");
        }
    }

    pub async fn save(&self) {
        self.save_world_border();
        for entity in self.entities.load().iter() {
            self.save_entity(entity).await;
        }

        let chunks: Vec<Vector2<i32>> = self
            .block_entities
            .iter()
            .map(|chunk_block_entities| *chunk_block_entities.key())
            .collect();
        for chunk_pos in chunks {
            self.save_block_entities(chunk_pos);
        }

        if let Ok(mut portal_poi) = self.portal_poi.try_lock() {
            let _ = portal_poi.save_all();
        }

        {
            let custom_data = self
                .custom_data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !custom_data.is_empty() {
                let custom_data_path = self
                    .level
                    .level_folder
                    .root_folder
                    .join("pumpkin_custom_data.nbt");
                let nbt = pumpkin_nbt::Nbt::from(custom_data.clone());
                let _ = std::fs::write(custom_data_path, nbt.write());
            }
        }

        self.level
            .should_save
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.level.level_channel.notify();

        let mut save_event = crate::plugin::api::events::world::world_save::WorldSaveEvent::new(
            format!("{:?}", self.dimension),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire(&server, &mut save_event).await;
        }
    }

    pub fn set_custom_data(&self, namespace: &str, key: &str, value: pumpkin_nbt::tag::NbtTag) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut namespace_data = custom_data
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                pumpkin_nbt::tag::NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        custom_data.child_tags.insert(
            namespace.into(),
            pumpkin_nbt::tag::NbtTag::Compound(namespace_data),
        );
    }

    pub fn get_custom_data(&self, namespace: &str, key: &str) -> Option<pumpkin_nbt::tag::NbtTag> {
        let custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        custom_data
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_custom_data(&self, namespace: &str, key: &str) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let Some(pumpkin_nbt::tag::NbtTag::Compound(mut namespace_data)) =
            custom_data.child_tags.remove(namespace)
        else {
            return;
        };

        namespace_data.child_tags.remove(key);
        if !namespace_data.is_empty() {
            custom_data.child_tags.insert(
                namespace.into(),
                pumpkin_nbt::tag::NbtTag::Compound(namespace_data),
            );
        }
    }

    pub fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.get_custom_data(namespace, key).is_some()
    }

    pub fn set_block_entity_custom_data(
        &self,
        pos: &BlockPos,
        namespace: &str,
        key: &str,
        value: pumpkin_nbt::tag::NbtTag,
    ) {
        let mut entry = self.custom_block_entity_data.entry(*pos).or_default();
        let mut namespace_data = entry
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                pumpkin_nbt::tag::NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        entry.child_tags.insert(
            namespace.into(),
            pumpkin_nbt::tag::NbtTag::Compound(namespace_data),
        );
    }

    pub fn get_block_entity_custom_data(
        &self,
        pos: &BlockPos,
        namespace: &str,
        key: &str,
    ) -> Option<pumpkin_nbt::tag::NbtTag> {
        self.custom_block_entity_data
            .get(pos)?
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_block_entity_custom_data(&self, pos: &BlockPos, namespace: &str, key: &str) {
        if let Some(mut entry) = self.custom_block_entity_data.get_mut(pos) {
            let Some(pumpkin_nbt::tag::NbtTag::Compound(mut namespace_data)) =
                entry.child_tags.remove(namespace)
            else {
                return;
            };

            namespace_data.child_tags.remove(key);
            if !namespace_data.is_empty() {
                entry.child_tags.insert(
                    namespace.into(),
                    pumpkin_nbt::tag::NbtTag::Compound(namespace_data),
                );
            }
        }
    }

    pub fn has_block_entity_custom_data(&self, pos: &BlockPos, namespace: &str, key: &str) -> bool {
        self.get_block_entity_custom_data(pos, namespace, key)
            .is_some()
    }

    pub fn populate_chunk(&self, chunk_pos: Vector2<i32>) {
        let mut populate_event =
            crate::plugin::api::events::world::chunk_populate::ChunkPopulateEvent::new(chunk_pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut populate_event);
        }
    }

    pub fn unload_chunk(&self, chunk_pos: Vector2<i32>) {
        let mut unload_event =
            crate::plugin::api::events::world::chunk_unload::ChunkUnloadEvent::new(chunk_pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut unload_event);
        }
    }

    pub fn load_entities(&self, chunk_pos: Vector2<i32>, entity_count: usize) {
        let mut load_event =
            crate::plugin::api::events::world::entities_load::EntitiesLoadEvent::new(
                chunk_pos,
                entity_count,
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut load_event);
        }
    }

    pub fn unload_entities(&self, chunk_pos: Vector2<i32>, entity_count: usize) {
        let mut unload_event =
            crate::plugin::api::events::world::entities_unload::EntitiesUnloadEvent::new(
                chunk_pos,
                entity_count,
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut unload_event);
        }
    }

    pub fn generate_loot(&self, loot_table: String) {
        let mut loot_event =
            crate::plugin::api::events::world::loot_generate::LootGenerateEvent::new(loot_table);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut loot_event);
        }
    }

    pub fn skip_time(&self, skip_amount: i64) {
        let mut time_event =
            crate::plugin::api::events::world::time_skip::TimeSkipEvent::new(skip_amount);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut time_event);
        }
    }

    pub fn trigger_raid(&self, pos: BlockPos) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_trigger::RaidTriggerEvent::new(pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn spawn_raid_wave(&self, wave: u32, pos: BlockPos) {
        let mut wave_event =
            crate::plugin::api::events::raid::raid_spawn_wave::RaidSpawnWaveEvent::new(wave, pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut wave_event);
        }
    }

    pub fn finish_raid(&self, victory: bool) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_finish::RaidFinishEvent::new(victory);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn stop_raid(&self, reason: String) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_stop::RaidStopEvent::new(reason);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn async_structure_generate(
        &self,
        world_name: String,
        structure_name: String,
        pos: BlockPos,
    ) {
        let mut event = crate::plugin::api::events::world::async_structure_generate::AsyncStructureGenerateEvent::new(
            world_name,
            structure_name,
            pos,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }

    pub fn async_structure_spawn(&self, world_name: String, structure_name: String, pos: BlockPos) {
        let mut event =
            crate::plugin::api::events::world::async_structure_spawn::AsyncStructureSpawnEvent::new(
                world_name,
                structure_name,
                pos,
            );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }
}

impl BlockAccessor for World {
    fn get_block(&self, position: &BlockPos) -> &'static Block {
        self.get_block_state_id_if_loaded(position)
            .map_or(&Block::AIR, Block::from_state_id)
    }
    fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
        self.get_block_state_id_if_loaded(position)
            .map_or(Block::AIR.default_state, BlockState::from_id)
    }

    fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
        self.get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id)
    }

    fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState) {
        let id = self
            .get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id);
        BlockState::from_id_with_block(id)
    }
}

fn bedrock_block_breaking_rate(speed: f32) -> i32 {
    (speed.clamp(0.0, 1.0) * f32::from(u16::MAX)) as i32
}

pub struct WorldPortal(pub Arc<World>);

// Pure Beauty :cap:
impl WorldPortalExt for WorldPortal {
    fn can_place_at(
        &self,
        block: &pumpkin_data::Block,
        state: &BlockState,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
    ) -> bool {
        self.0.block_registry.can_place_at(
            None,
            Some(&self.0),
            block_accessor,
            None,
            block,
            state,
            block_pos,
            None,
            None,
        )
    }

    fn mirror(&self, block: &Block, state_id: BlockStateId, mirror: Mirror) -> &'static BlockState {
        self.0.block_registry.mirror(block, state_id, mirror)
    }

    fn rotate(
        &self,
        block: &Block,
        state_id: BlockStateId,
        rotation: Rotation,
    ) -> &'static BlockState {
        self.0.block_registry.rotate(block, state_id, rotation)
    }

    fn spawn_mobs_for_chunk_generation(
        &self,
        cache: &mut dyn GenerationCache,
        biome: &'static Biome,
        chunk_x: i32,
        chunk_z: i32,
    ) {
        natural_spawner::spawn_mobs_for_chunk_generation(&self.0, cache, biome, chunk_x, chunk_z);
    }

    fn spawn_structure_entities(&self, entities: Vec<NbtCompound>) {
        for nbt in entities {
            let Some(id) = nbt.get_string("id") else {
                continue;
            };
            let Some(entity_type) =
                EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))
            else {
                warn!("Unknown structure entity type: {id}");
                continue;
            };
            let entity = from_type(
                entity_type,
                Vector3::new(0.0, 0.0, 0.0),
                &self.0,
                Uuid::new_v4(),
            );
            entity.get_entity().read_nbt_non_mut(&nbt);
            entity.read_nbt_non_mut(&nbt);
            self.0.spawn_entity(entity);
        }
    }
}

struct CubicCurve {
    a: f32,
    b: f32,
    c: f32,
}

impl CubicCurve {
    fn new(v1: f32, v2: f32) -> Self {
        Self {
            a: 3.0 * v1 - 3.0 * v2 + 1.0,
            b: -6.0 * v1 + 3.0 * v2,
            c: 3.0 * v1,
        }
    }

    fn sample(&self, t: f32) -> f32 {
        ((self.a * t + self.b) * t + self.c) * t
    }

    fn sample_gradient(&self, t: f32) -> f32 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

/// Calculates the celestial (sun) angle fraction in `[0.0, 1.0]`.
/// Matches vanilla 26.2 `EnvironmentAttributes.SUN_ANGLE` easing with `symmetricCubicBezier(0.362, 0.241)`.
#[must_use]
pub fn calculate_celestial_angle(time_of_day: i64) -> f32 {
    let ticks = time_of_day.rem_euclid(24000);
    let alpha = if ticks < 6000 {
        (ticks + 18000) as f32 / 24000.0
    } else {
        (ticks - 6000) as f32 / 24000.0
    };

    let x_curve = CubicCurve::new(0.362, 0.638);
    let y_curve = CubicCurve::new(0.241, 0.759);

    let mut t = alpha;
    let mut solved = false;
    for _ in 0..4 {
        let error = x_curve.sample(t) - alpha;
        if error.abs() < 1e-5 {
            solved = true;
            break;
        }
        let gradient = x_curve.sample_gradient(t);
        if gradient < 1e-5 {
            break;
        }
        t -= (error / gradient).clamp(-0.25, 0.25);
    }

    if !solved {
        let mut t0 = 0.0f32;
        let mut t1 = 1.0f32;
        for _ in 0..64 {
            if t0 >= t1 {
                break;
            }
            let error = x_curve.sample(t) - alpha;
            if error.abs() < 1e-5 {
                break;
            }
            if error < 0.0 {
                t0 = t;
            } else {
                t1 = t;
            }
            t = f32::midpoint(t1, t0);
        }
    }

    y_curve.sample(t)
}

#[cfg(test)]
mod tests {
    use pumpkin_data::{
        Block,
        block_properties::{ChestLikeProperties, ChestType, HorizontalFacing, WaterLikeProperties},
        fluid::Fluid,
    };
    use pumpkin_util::math::position::BlockPos;

    use super::{World, bedrock_block_breaking_rate, bedrock_chest_block_actor};

    #[test]
    fn liquid_block_states_preserve_source_flow_and_falling_depths() {
        // Liquid block level and fluid amount are different: all falling levels
        // resolve to amount 8, rather than wrapping through the fluid state array.
        let amounts = [8, 7, 6, 5, 4, 3, 2, 1, 8, 8, 8, 8, 8, 8, 8, 8];
        for (block, expected_fluid) in [
            (&Block::WATER, &Fluid::FLOWING_WATER),
            (&Block::LAVA, &Fluid::FLOWING_LAVA),
        ] {
            for (level, amount) in amounts.into_iter().enumerate() {
                let id = WaterLikeProperties { level: level as u8 }.to_state_id(block);
                let (fluid, state) = World::fluid_state_from_block_state(id);
                assert_eq!(fluid.id, expected_fluid.id);
                assert_eq!(state.level, amount);
                assert!((state.height - f32::from(amount) / 9.0).abs() < f32::EPSILON);
                assert_eq!(state.is_source, level == 0);
                assert_eq!(state.is_still, level == 0);
                assert_eq!(state.falling, level >= 8);
                assert!(!state.is_empty);
                assert_eq!(
                    state.block_state_id,
                    WaterLikeProperties {
                        level: level.min(8) as u8
                    }
                    .to_state_id(block)
                );
            }
        }
    }

    #[test]
    fn aquatic_and_waterlogged_blocks_contain_source_water() {
        let wet_stairs = Block::OAK_STAIRS
            .set_waterlogged(Block::OAK_STAIRS.default_state.id, true)
            .unwrap();
        for id in [
            Block::KELP.default_state.id,
            Block::KELP_PLANT.default_state.id,
            Block::SEAGRASS.default_state.id,
            Block::TALL_SEAGRASS.default_state.id,
            Block::BUBBLE_COLUMN.default_state.id,
            wet_stairs,
        ] {
            let (fluid, state) = World::fluid_state_from_block_state(id);
            assert!(fluid.matches_type(&Fluid::WATER));
            assert!(state.is_source && state.is_still && !state.is_empty && !state.falling);
            assert_eq!(state.level, 8);
            assert!((state.height - 8.0 / 9.0).abs() < f32::EPSILON);
        }

        for block in [&Block::AIR, &Block::OAK_STAIRS, &Block::WATER_CAULDRON] {
            let (fluid, state) = World::fluid_state_from_block_state(block.default_state.id);
            assert_eq!(fluid.id, Fluid::EMPTY.id);
            assert!(state.is_empty);
            assert_eq!(state.height, 0.0);
        }
    }

    #[test]
    fn bedrock_block_breaking_rate_uses_progress_per_tick() {
        assert_eq!(bedrock_block_breaking_rate(0.0), 0);
        assert_eq!(bedrock_block_breaking_rate(1.0 / 30.0), 2_184);
        assert_eq!(bedrock_block_breaking_rate(1.0), 65_535);
    }

    #[test]
    fn bedrock_double_chest_block_actor_identifies_pair_and_lead() {
        let position = BlockPos::new(5, 64, 7);
        let properties = ChestLikeProperties {
            facing: HorizontalFacing::North,
            r#type: ChestType::Right,
            waterlogged: false,
        };
        let actor =
            bedrock_chest_block_actor(properties.to_state_id(&Block::CHEST), position).unwrap();

        assert_eq!(actor.get_int("pairx"), Some(4));
        assert_eq!(actor.get_int("pairz"), Some(7));
        assert_eq!(actor.get_bool("pairlead"), Some(true));
    }

    #[test]
    fn game_rules_registry() {
        use pumpkin_data::game_rules::{GameRule, GameRuleRegistry, GameRuleValue};

        let mut registry = GameRuleRegistry::default();
        match registry.get(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => assert!(!v),
            GameRuleValue::Int(_) => panic!("expected bool"),
        }

        match registry.get_mut(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => *v = true,
            GameRuleValue::Int(_) => panic!("expected bool"),
        }

        match registry.get(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => assert!(v),
            GameRuleValue::Int(_) => panic!("expected bool"),
        }

        match registry.get(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => assert_eq!(*v, 3),
            GameRuleValue::Bool(_) => panic!("expected int"),
        }

        match registry.get_mut(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => *v = 20,
            GameRuleValue::Bool(_) => panic!("expected int"),
        }

        match registry.get(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => assert_eq!(*v, 20),
            GameRuleValue::Bool(_) => panic!("expected int"),
        }
    }
}
