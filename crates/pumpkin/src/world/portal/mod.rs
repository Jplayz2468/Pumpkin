use std::sync::Arc;

use pumpkin_data::Block;
use pumpkin_data::block_properties::HorizontalAxis;
use pumpkin_data::dimension::Dimension;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use super::World;
use crate::entity::teleport_state::{self, TeleportState};

pub mod end;
pub mod nether;
pub mod poi;
pub mod tickets;

pub use nether::{NetherPortal, PortalSearchResult};
pub use poi::PortalPoiStorage;

#[derive(Clone)]
pub struct SourcePortalInfo {
    pub lower_corner: BlockPos,
    pub axis: HorizontalAxis,
    pub width: u32,
    pub height: u32,
}

impl From<&PortalSearchResult> for SourcePortalInfo {
    fn from(result: &PortalSearchResult) -> Self {
        Self {
            lower_corner: result.lower_corner,
            axis: result.axis,
            width: result.width,
            height: result.height,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PortalType {
    Nether,
    End,
}

impl PortalType {
    pub fn get_portal_transition_time(
        &self,
        current_world: &World,
        entity: &dyn crate::entity::EntityBase,
    ) -> u32 {
        match self {
            Self::End => 0,
            Self::Nether => {
                let entity_type = entity.get_entity().entity_type;
                let level_info = current_world.level_info.load();
                match entity_type.id {
                    id if id == pumpkin_data::entity::EntityType::PLAYER.id => (current_world
                        .get_player_by_id(entity.get_entity().entity_id))
                    .map_or(80, |player| match player.gamemode.load() {
                        pumpkin_util::GameMode::Creative => {
                            level_info.game_rules.players_nether_portal_creative_delay as u32
                        }
                        _ => level_info.game_rules.players_nether_portal_default_delay as u32,
                    }),
                    _ => 0,
                }
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn get_portal_destination(
        &self,
        current_level: &World,
        dest_world: Arc<World>,
        caller: &dyn crate::entity::EntityBase,
        source_portal: Option<&SourcePortalInfo>,
    ) -> Option<TeleportTransition> {
        match self {
            Self::End => {
                let is_end_portal = dest_world.dimension == Dimension::THE_END
                    || current_level.dimension == Dimension::THE_END;

                if is_end_portal {
                    if dest_world.dimension == Dimension::THE_END {
                        // Entering the End: spawn on the obsidian platform at (100, 49, 0) for players, or (100, 50, 0) for other entities
                        let is_player = caller
                            .get_living_entity()
                            .is_some_and(crate::entity::living::LivingEntity::is_player);
                        let y = if is_player { 49.0 } else { 50.0 };

                        let platform_pos = BlockPos::new(100, 49, 0);

                        // Ensure chunks covering the platform are loaded/generated
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    let center_chunk =
                                        Vector2::new(platform_pos.0.x >> 4, platform_pos.0.z >> 4);
                                    dest_world
                                        .level
                                        .get_or_fetch_chunk(center_chunk, |_| ())
                                        .await;
                                });
                            });
                        }

                        // Generate/regenerate the obsidian platform (5x5 obsidian at Y=48, and 5x5x3 air above it)
                        for dx in -2..=2 {
                            for dz in -2..=2 {
                                for dy in -1..3 {
                                    let block = if dy == -1 {
                                        Block::OBSIDIAN
                                    } else {
                                        Block::AIR
                                    };
                                    let target_pos = BlockPos::new(
                                        platform_pos.0.x + dx,
                                        platform_pos.0.y + dy,
                                        platform_pos.0.z + dz,
                                    );
                                    dest_world.set_block_state(
                                        &target_pos,
                                        block.default_state.id,
                                        BlockFlags::NOTIFY_ALL,
                                    );
                                }
                            }
                        }

                        Some(TeleportTransition::portal(
                            dest_world,
                            Vector3::new(100.5, y, 0.5),
                            90.0,
                            0.0,
                            teleport_state::DELTA | teleport_state::X_ROT,
                        ))
                    } else {
                        // Leaving the End through the exit portal: return to overworld spawn
                        if let Some(player) =
                            current_level.get_player_by_id(caller.get_entity().entity_id)
                        {
                            match player.client.as_ref() {
                                crate::net::ClientPlatform::Java(client) => {
                                    if let Ok(data) = client.serialize_packet(&pumpkin_protocol::java::client::play::CGameEvent::new(
                                        pumpkin_protocol::java::client::play::GameEvent::WinGame,
                                        1.0,
                                    )) {
                                        client.try_enqueue_packet(data);
                                    }
                                }
                                crate::net::ClientPlatform::Bedrock(client) => {
                                    if let Ok(data) = client.serialize_packet(
                                        &pumpkin_protocol::bedrock::client::CShowCredits {
                                            player_runtime_id: (caller.get_entity().entity_id
                                                as u64)
                                                .into(),
                                            credits_state: 0.into(),
                                        },
                                    ) {
                                        client.try_enqueue_packet(data);
                                    }
                                }
                            }
                        }

                        let info = dest_world.level_info.load();
                        Some(TeleportTransition::portal(
                            dest_world,
                            Vector3::new(
                                f64::from(info.spawn_x) + 0.5,
                                f64::from(info.spawn_y),
                                f64::from(info.spawn_z) + 0.5,
                            ),
                            info.spawn_yaw,
                            info.spawn_pitch,
                            teleport_state::DELTA | teleport_state::ROTATION,
                        ))
                    }
                } else {
                    None
                }
            }
            Self::Nether => {
                let pos = caller.get_entity().pos.load();

                let dimensions = caller.get_entity().entity_dimension.load();
                let scale_factor_new = dest_world.dimension.coordinate_scale;
                let scale_factor_current = current_level.dimension.coordinate_scale;

                let teleportation_scale = scale_factor_current / scale_factor_new;
                let worldborder = dest_world
                    .worldborder
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let (clamped_x, clamped_z) = worldborder.clamp_block(
                    (pos.x * teleportation_scale).floor() as i32,
                    (pos.z * teleportation_scale).floor() as i32,
                );
                drop(worldborder);

                let approximate_exit_pos =
                    BlockPos::new(clamped_x, pos.y.floor() as i32, clamped_z);
                let source_portal_axis = source_portal.map_or(HorizontalAxis::X, |p| p.axis);

                let exit_portal = NetherPortal::search_for_portal(
                    &dest_world,
                    approximate_exit_pos,
                )
                .or_else(|| {
                    // Ensure the chunks around approximate_exit_pos are generated/loaded in dest_world
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        tokio::task::block_in_place(|| {
                            handle.block_on(async {
                                let center_chunk = Vector2::new(
                                    approximate_exit_pos.0.x >> 4,
                                    approximate_exit_pos.0.z >> 4,
                                );
                                for dx in -1..=1 {
                                    for dz in -1..=1 {
                                        let chunk_pos =
                                            Vector2::new(center_chunk.x + dx, center_chunk.y + dz);
                                        dest_world
                                            .level
                                            .get_or_fetch_chunk(chunk_pos, |_| ())
                                            .await;
                                    }
                                }
                            });
                        });
                    }

                    if let Some((build_pos, axis, is_fallback)) = NetherPortal::find_safe_location(
                        &dest_world,
                        approximate_exit_pos,
                        source_portal_axis,
                    ) {
                        NetherPortal::build_portal_frame(&dest_world, build_pos, axis, is_fallback);
                        Some(PortalSearchResult {
                            lower_corner: build_pos,
                            axis,
                            width: 2,
                            height: 3,
                        })
                    } else {
                        None
                    }
                });

                let (final_pos, yaw) = exit_portal.map_or_else(
                    || (approximate_exit_pos.0.to_f64(), 0.0),
                    |exit_portal| {
                        let relative_offset = source_portal.map_or_else(
                            || Vector3::new(0.5, 0.0, 0.0),
                            |source| {
                                let source_result = PortalSearchResult {
                                    lower_corner: source.lower_corner,
                                    axis: source.axis,
                                    width: source.width,
                                    height: source.height,
                                };
                                source_result.entity_pos_in_portal(pos, &dimensions)
                            },
                        );
                        let target_pos =
                            exit_portal.calculate_exit_position(relative_offset, &dimensions);
                        let collision_free_pos =
                            exit_portal.find_open_position(&dest_world, target_pos, &dimensions);
                        let yaw = exit_portal.calculate_teleport_yaw(0.0, Some(source_portal_axis));
                        (collision_free_pos, yaw)
                    },
                );

                Some(TeleportTransition::portal(
                    dest_world,
                    final_pos,
                    yaw,
                    0.0,
                    teleport_state::DELTA | teleport_state::ROTATION,
                ))
            }
        }
    }
}

#[derive(Clone)]
pub struct TeleportTransition {
    pub new_world: Arc<World>,
    pub state: TeleportState,
    pub relatives: u16,
    pub as_passenger: bool,
    pub portal: bool,
}

impl TeleportTransition {
    pub fn from_target(
        world: Arc<World>,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
    ) -> Self {
        Self {
            new_world: world,
            state: TeleportState {
                position,
                velocity: Vector3::default(),
                yaw: yaw.unwrap_or(0.0),
                pitch: pitch.unwrap_or(0.0),
            },
            relatives: teleport_state::DELTA
                | if yaw.is_none() {
                    teleport_state::Y_ROT
                } else {
                    0
                }
                | if pitch.is_none() {
                    teleport_state::X_ROT
                } else {
                    0
                },
            as_passenger: false,
            portal: false,
        }
    }

    fn portal(
        world: Arc<World>,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
        relatives: u16,
    ) -> Self {
        Self {
            new_world: world,
            state: TeleportState {
                position,
                velocity: Vector3::default(),
                yaw,
                pitch,
            },
            relatives,
            as_passenger: false,
            portal: true,
        }
    }
}

#[derive(Clone)]
pub struct PortalProcessor {
    pub portal_type: PortalType,
    pub entry_position: BlockPos,
    pub portal_time: u32,
    pub inside_portal_this_tick: bool,
    pub destination_world: Arc<World>,
    pub source_portal: Option<SourcePortalInfo>,
}

impl PortalProcessor {
    pub const fn new(
        portal_type: PortalType,
        entry_position: BlockPos,
        destination_world: Arc<World>,
    ) -> Self {
        Self {
            portal_type,
            entry_position,
            portal_time: 0,
            inside_portal_this_tick: true,
            destination_world,
            source_portal: None,
        }
    }

    pub const fn set_source_portal(&mut self, info: SourcePortalInfo) {
        self.source_portal = Some(info);
    }

    pub fn process_portal_teleportation(
        &mut self,
        current_world: &World,
        entity: &dyn crate::entity::EntityBase,
        allowed_to_teleport: bool,
    ) -> bool {
        if self.inside_portal_this_tick {
            self.inside_portal_this_tick = false;
            if allowed_to_teleport {
                let transition_time = self
                    .portal_type
                    .get_portal_transition_time(current_world, entity);
                advance_portal_time(&mut self.portal_time, transition_time)
            } else {
                false
            }
        } else {
            self.decay_tick();
            false
        }
    }

    pub const fn decay_tick(&mut self) {
        self.portal_time = self.portal_time.saturating_sub(4);
    }

    #[must_use]
    pub const fn has_expired(&self) -> bool {
        self.portal_time == 0
    }
}

fn advance_portal_time(time: &mut u32, delay: u32) -> bool {
    let previous = *time;
    *time = time.wrapping_add(1);
    previous >= delay
}

#[cfg(test)]
mod processor_tests {
    use super::advance_portal_time;
    #[test]
    fn transition_compares_the_previous_contact_count() {
        let mut time = 0;
        for _ in 0..80 {
            assert!(!advance_portal_time(&mut time, 80));
        }
        assert!(advance_portal_time(&mut time, 80));
        time = 0;
        assert!(advance_portal_time(&mut time, 0));
        assert_eq!(time, 1);
    }
}
