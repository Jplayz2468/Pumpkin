//! Vanilla's vibration system (`world/level/gameevent/vibrations/VibrationSystem.java`).
//!
//! Sculk sensors and shriekers do not react to game events directly. An event first has
//! to reach a *listener* within range, survive a validity check and an occlusion test,
//! and then travel to the listener at one block per tick before it fires. This module is
//! that pipeline; the blocks themselves only implement [`VibrationUser`].
//!
//! ## One deliberate difference from vanilla
//!
//! Vanilla keeps a `GameEventListenerRegistry` per chunk section and dispatches through
//! it. That is a lookup optimisation, not a behaviour: the set of listeners it reaches is
//! exactly "those whose radius covers the event". [`dispatch`] computes the same set by
//! scanning the block entities in range, which is at most a few chunks because the
//! largest block listener radius is 16 blocks. Registering and unregistering
//! listeners as chunks and block entities come and go is a large amount of bookkeeping
//! for no observable difference, so it is not reproduced.

use pumpkin_world::chunk::io::Dirtiable;
use std::sync::Arc;

use pumpkin_data::game_event::GameEvent;
use pumpkin_data::tag::Tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::{entity::EntityBase, world::World};

/// `VibrationSystem.NO_VIBRATION_FREQUENCY`: an event no sensor reacts to.
pub const NO_VIBRATION_FREQUENCY: u8 = 0;

/// CalibratedSculkSensorBlockEntity.VibrationUser.getListenerRadius returns 16;
/// ordinary sensors and shriekers use 8.
pub const MAX_LISTENER_RADIUS: i32 = 16;

/// `VibrationSystem.VIBRATION_FREQUENCY_FOR_EVENT` (VibrationSystem.java:54-99).
///
/// This is the number a sculk sensor reports through a comparator, so the exact value per
/// event is observable in-game and worth having verbatim rather than approximated.
#[must_use]
pub fn vibration_frequency(event: GameEvent) -> u8 {
    match event {
        GameEvent::Step | GameEvent::Swim | GameEvent::Flap => 1,
        GameEvent::ProjectileLand
        | GameEvent::HitGround
        | GameEvent::Splash
        | GameEvent::Bounce => 2,
        GameEvent::ItemInteractFinish | GameEvent::ProjectileShoot | GameEvent::InstrumentPlay => 3,
        GameEvent::EntityAction | GameEvent::ElytraGlide | GameEvent::Unequip => 4,
        GameEvent::EntityDismount | GameEvent::Equip => 5,
        GameEvent::EntityInteract | GameEvent::Shear | GameEvent::EntityMount => 6,
        GameEvent::EntityDamage => 7,
        GameEvent::Drink | GameEvent::Eat => 8,
        GameEvent::ContainerClose
        | GameEvent::BlockClose
        | GameEvent::BlockDeactivate
        | GameEvent::BlockDetach => 9,
        GameEvent::ContainerOpen
        | GameEvent::BlockOpen
        | GameEvent::BlockActivate
        | GameEvent::BlockAttach
        | GameEvent::PrimeFuse
        | GameEvent::NoteBlockPlay => 10,
        GameEvent::BlockChange => 11,
        GameEvent::BlockDestroy | GameEvent::FluidPickup => 12,
        GameEvent::BlockPlace | GameEvent::FluidPlace => 13,
        GameEvent::EntityPlace | GameEvent::LightningStrike | GameEvent::Teleport => 14,
        GameEvent::EntityDie | GameEvent::Explode => 15,
        // The fifteen resonance events map to their own frequency, so an amethyst block
        // next to a calibrated sensor re-emits the frequency it resonated at.
        GameEvent::Resonate1 => 1,
        GameEvent::Resonate2 => 2,
        GameEvent::Resonate3 => 3,
        GameEvent::Resonate4 => 4,
        GameEvent::Resonate5 => 5,
        GameEvent::Resonate6 => 6,
        GameEvent::Resonate7 => 7,
        GameEvent::Resonate8 => 8,
        GameEvent::Resonate9 => 9,
        GameEvent::Resonate10 => 10,
        GameEvent::Resonate11 => 11,
        GameEvent::Resonate12 => 12,
        GameEvent::Resonate13 => 13,
        GameEvent::Resonate14 => 14,
        GameEvent::Resonate15 => 15,
        _ => NO_VIBRATION_FREQUENCY,
    }
}

/// `VibrationSystem.getResonanceEventByFrequency` (VibrationSystem.java:35-50).
#[must_use]
pub const fn resonance_event_for_frequency(frequency: u8) -> Option<GameEvent> {
    Some(match frequency {
        1 => GameEvent::Resonate1,
        2 => GameEvent::Resonate2,
        3 => GameEvent::Resonate3,
        4 => GameEvent::Resonate4,
        5 => GameEvent::Resonate5,
        6 => GameEvent::Resonate6,
        7 => GameEvent::Resonate7,
        8 => GameEvent::Resonate8,
        9 => GameEvent::Resonate9,
        10 => GameEvent::Resonate10,
        11 => GameEvent::Resonate11,
        12 => GameEvent::Resonate12,
        13 => GameEvent::Resonate13,
        14 => GameEvent::Resonate14,
        15 => GameEvent::Resonate15,
        _ => return None,
    })
}

/// `VibrationSystem.getRedstoneStrengthForDistance` (VibrationSystem.java:120): the
/// redstone level a sensor outputs falls off linearly with distance, never below 1.
#[must_use]
pub fn redstone_strength_for_distance(distance: f32, listener_radius: i32) -> u8 {
    let power_scale = 15.0 / f64::from(listener_radius);
    let strength = 15 - (power_scale * f64::from(distance)).floor() as i32;
    strength.max(1) as u8
}

/// Whether a game event belongs to a tag.
///
/// `GameEvent` has no `Taggable` impl -- unlike blocks and items it is not a registry
/// object here -- but a `Tag` is just its member names, and `GameEvent::name` returns the
/// same unqualified form the tag lists ("step", "block_open").
#[must_use]
pub fn event_in_tag(event: GameEvent, tag: &Tag) -> bool {
    tag.0.contains(&event.name())
}

/// One vibration in flight. `VibrationInfo`.
#[derive(Clone)]
pub struct VibrationInfo {
    pub event: GameEvent,
    pub distance: f32,
    pub origin: Vector3<f64>,
    /// Entity id of whatever caused the event, if anything did.
    pub source_entity: Option<i32>,
    pub source_uuid: Option<uuid::Uuid>,
    pub projectile_owner_uuid: Option<uuid::Uuid>,
}

/// `VibrationSystem.Data` plus the `VibrationSelector` it owns.
///
/// The selector exists because several events can arrive in the same tick and only one
/// vibration may travel at a time: candidates accumulate, and the winner is not chosen
/// until a later tick than the one it arrived on (VibrationSelector.chosenCandidate).
#[derive(Default)]
pub struct VibrationData {
    current: Option<VibrationInfo>,
    travel_time: i32,
    /// The best candidate so far, with the tick it was added on.
    candidate: Option<(VibrationInfo, u64)>,
    reload_particle: bool,
}

/// What a block must provide to listen for vibrations. `VibrationSystem.User`.
pub trait VibrationUser {
    /// `getListenerRadius`.
    fn listener_radius(&self) -> i32;

    /// Where the listener sits; `getPositionSource` resolved to a block.
    fn listener_pos(&self) -> BlockPos;

    /// `getListenableEvents`; defaults to `#minecraft:vibrations` as vanilla does.
    fn listenable_events(&self) -> Tag {
        pumpkin_data::tag::GameEvent::MINECRAFT_VIBRATIONS
    }

    /// `canReceiveVibration`: a last check once the event is known to be in range.
    fn can_receive_vibration(
        &self,
        world: &Arc<World>,
        origin: &BlockPos,
        event: GameEvent,
        source_entity: Option<i32>,
    ) -> bool;

    /// `onReceiveVibration`: the vibration has arrived. This is where the block acts.
    fn on_receive_vibration(
        &self,
        world: &Arc<World>,
        origin: &BlockPos,
        event: GameEvent,
        source_entity: Option<i32>,
        distance: f32,
    );

    /// `calculateTravelTimeInTicks`: one tick per block, floored.
    fn calculate_travel_time(&self, distance: f32) -> i32 {
        distance.floor() as i32
    }
}

impl VibrationInfo {
    fn new(
        world: &World,
        event: GameEvent,
        distance: f32,
        origin: Vector3<f64>,
        source_entity: Option<i32>,
    ) -> Self {
        let source = source_entity.and_then(|id| world.get_entity_by_id(id));
        Self::from_entity(world, event, distance, origin, source.as_deref())
    }

    fn from_entity(
        _world: &World,
        event: GameEvent,
        distance: f32,
        origin: Vector3<f64>,
        source: Option<&dyn EntityBase>,
    ) -> Self {
        let source_entity = source.map(|entity| entity.get_entity().entity_id);
        let source_uuid = source.as_ref().map(|e| e.get_entity().entity_uuid);
        let projectile_owner_uuid = source
            .filter(|entity| {
                crate::entity::projectile::is_projectile(entity.get_entity().entity_type)
            })
            .and_then(|e| e.get_projectile_owner())
            .map(|e| e.get_entity().entity_uuid);
        Self {
            event,
            distance,
            origin,
            source_entity,
            source_uuid,
            projectile_owner_uuid,
        }
    }

    fn read_nbt(nbt: &pumpkin_nbt::compound::NbtCompound) -> Option<Self> {
        use pumpkin_nbt::tag::NbtTag;
        let event = GameEvent::from_name(nbt.get_string("game_event")?)?;
        let distance = nbt.get_float("distance")?;
        if !distance.is_finite() || distance < 0.0 {
            return None;
        }
        let coords = nbt.get_list("pos")?;
        let [NbtTag::Double(x), NbtTag::Double(y), NbtTag::Double(z)] = coords else {
            return None;
        };
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return None;
        }
        Some(Self {
            event,
            distance,
            origin: Vector3::new(*x, *y, *z),
            source_entity: None,
            source_uuid: nbt.get_uuid("source"),
            projectile_owner_uuid: nbt.get_uuid("projectile_owner"),
        })
    }

    fn write_nbt(&self) -> pumpkin_nbt::compound::NbtCompound {
        use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
        let mut nbt = NbtCompound::new();
        nbt.put_string("game_event", format!("minecraft:{}", self.event.name()));
        nbt.put_float("distance", self.distance);
        nbt.put_list(
            "pos",
            vec![
                NbtTag::Double(self.origin.x),
                NbtTag::Double(self.origin.y),
                NbtTag::Double(self.origin.z),
            ],
        );
        if let Some(id) = self.source_uuid {
            nbt.put_uuid("source", id);
        }
        if let Some(id) = self.projectile_owner_uuid {
            nbt.put_uuid("projectile_owner", id);
        }
        nbt
    }

    fn resolve_source(&self, world: &World) -> Option<i32> {
        let resolve = |uuid| {
            world
                .get_entity_by_uuid(uuid)
                .map(|e| e.get_entity().entity_id)
                .or_else(|| {
                    world
                        .get_player_by_uuid(uuid)
                        .map(|p| p.living_entity.entity.entity_id)
                })
        };
        self.source_entity
            .filter(|id| world.get_entity_by_id(*id).is_some())
            .or_else(|| self.source_uuid.and_then(resolve))
            .or_else(|| self.projectile_owner_uuid.and_then(resolve))
    }
}

impl VibrationData {
    /// VibrationSystem.Data.CODEC and VibrationSelector.CODEC: retain both a
    /// travelling vibration and the pending candidate across save/reload.
    pub fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound) -> Self {
        let Some(listener) = nbt.get_compound("listener") else {
            return Self::default();
        };
        let candidate = listener.get_compound("selector").and_then(|selector| {
            let tick = selector.get_long("tick")?;
            if tick < 0 {
                return None;
            }
            Some((
                VibrationInfo::read_nbt(selector.get_compound("event")?)?,
                tick as u64,
            ))
        });
        Self {
            current: listener
                .get_compound("event")
                .and_then(VibrationInfo::read_nbt),
            travel_time: listener.get_int("event_delay").unwrap_or(0).max(0),
            candidate,
            reload_particle: true,
        }
    }

    pub fn write_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        use pumpkin_nbt::compound::NbtCompound;
        let mut listener = NbtCompound::new();
        if let Some(current) = &self.current {
            listener.put_compound("event", current.write_nbt());
        }
        listener.put_int("event_delay", self.travel_time.max(0));
        let mut selector = NbtCompound::new();
        if let Some((candidate, time)) = &self.candidate {
            selector.put_compound("event", candidate.write_nbt());
            selector.put_long("tick", *time as i64);
        } else {
            selector.put_long("tick", -1);
        }
        listener.put_compound("selector", selector);
        nbt.put_compound("listener", listener);
    }

    /// `VibrationSystem.Listener.handleGameEvent` (VibrationSystem.java:211).
    ///
    /// Returns whether the event was accepted as a candidate.
    pub fn handle_game_event(
        &mut self,
        world: &Arc<World>,
        user: &dyn VibrationUser,
        event: GameEvent,
        origin: Vector3<f64>,
        source_entity: Option<i32>,
        game_time: u64,
    ) -> bool {
        let source = source_entity.and_then(|id| world.get_entity_by_id(id));
        self.handle_game_event_from_entity(world, user, event, origin, source.as_deref(), game_time)
    }

    fn handle_game_event_from_entity(
        &mut self,
        world: &Arc<World>,
        user: &dyn VibrationUser,
        event: GameEvent,
        origin: Vector3<f64>,
        source: Option<&dyn EntityBase>,
        game_time: u64,
    ) -> bool {
        let source_entity = source.map(|entity| entity.get_entity().entity_id);
        // A listener already carrying a vibration ignores everything until it lands.
        if self.current.is_some() {
            return false;
        }
        if !is_valid_vibration(user, event, source) {
            return false;
        }

        let dest_block = user.listener_pos();
        let dest = Vector3::new(
            f64::from(dest_block.0.x) + 0.5,
            f64::from(dest_block.0.y) + 0.5,
            f64::from(dest_block.0.z) + 0.5,
        );

        let origin_block = BlockPos::floored(origin.x, origin.y, origin.z);
        if !user.can_receive_vibration(world, &origin_block, event, source_entity) {
            return false;
        }
        if is_occluded(world, origin, dest) {
            return false;
        }

        let distance = (origin.squared_distance_to_vec(&dest)).sqrt() as f32;
        self.add_candidate(
            VibrationInfo::from_entity(world, event, distance, origin, source),
            game_time,
        );
        true
    }

    /// `VibrationSelector.addCandidate`: a new event replaces the stored one only if it
    /// is strictly better -- nearer wins, and on equal distance the higher frequency does.
    fn add_candidate(&mut self, candidate: VibrationInfo, game_time: u64) {
        let replace = match &self.candidate {
            None => true,
            Some((current, current_tick)) => {
                if *current_tick != game_time {
                    // The stored candidate belongs to an earlier tick and is about to be
                    // chosen; leave it alone.
                    false
                } else {
                    let new_frequency = vibration_frequency(candidate.event);
                    let old_frequency = vibration_frequency(current.event);
                    candidate.distance < current.distance
                        || (candidate.distance == current.distance && new_frequency > old_frequency)
                }
            }
        };
        if replace {
            self.candidate = Some((candidate, game_time));
        }
    }

    /// `VibrationSystem.Ticker.tick` (VibrationSystem.java:275). Call once per tick from
    /// the owning block entity.
    ///
    /// Returns the vibration to deliver, if one landed this tick. Delivery is handed back
    /// rather than performed here so the caller can release the lock on this data first:
    /// `on_receive_vibration` sets block state and emits further game events, which can
    /// re-enter this listener.
    pub fn tick(&mut self, user: &dyn VibrationUser, game_time: u64) -> Option<VibrationInfo> {
        if self.current.is_none() {
            self.try_select_and_schedule(user, game_time);
        }

        if self.current.is_some() {
            self.travel_time -= 1;
            if self.travel_time <= 0 {
                return self.current.take();
            }
        }
        None
    }

    /// `trySelectAndScheduleVibration` (VibrationSystem.java:301). A candidate is only
    /// eligible on a *later* tick than it arrived, which is what lets several events in
    /// one tick compete before one is picked.
    fn try_select_and_schedule(&mut self, user: &dyn VibrationUser, game_time: u64) {
        let Some((_, added_tick)) = &self.candidate else {
            return;
        };
        if *added_tick >= game_time {
            return;
        }
        let (chosen, _) = self.candidate.take().expect("checked above");
        self.travel_time = user.calculate_travel_time(chosen.distance);
        self.current = Some(chosen);
    }

    /// Whether a vibration is currently travelling to this listener.
    #[must_use]
    pub const fn has_vibration_in_flight(&self) -> bool {
        self.current.is_some()
    }
}

/// A block entity that listens for vibrations.
///
/// Object-safe so `dyn BlockEntity` can hand one out. Implementors supply the
/// [`VibrationUser`] behaviour and somewhere to keep the [`VibrationData`]; the two
/// driving methods below are the whole contract and are the same for every listener.
pub trait VibrationListener: VibrationUser + Send + Sync {
    fn vibration_data(&self) -> &std::sync::Mutex<VibrationData>;

    /// Self as a [`VibrationUser`]. Written out rather than relying on an upcast so the
    /// default methods below stay callable on `dyn VibrationListener`; every implementor
    /// returns `self`.
    fn as_vibration_user(&self) -> &dyn VibrationUser;

    /// SculkSensorBlock.stepOn uses Listener.forceScheduleVibration, which
    /// bypasses validity and occlusion checks while retaining selector ordering.
    fn force_vibration(
        &self,
        world: &World,
        event: GameEvent,
        origin: Vector3<f64>,
        source_entity: Option<i32>,
        game_time: u64,
    ) {
        let distance = origin
            .squared_distance_to_vec(&self.listener_pos().to_centered_f64())
            .sqrt() as f32;
        self.vibration_data()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .add_candidate(
                VibrationInfo::new(world, event, distance, origin, source_entity),
                game_time,
            );
    }

    /// Offer a game event to this listener. `VibrationSystem.Listener.handleGameEvent`.
    fn handle_vibration(
        &self,
        world: &Arc<World>,
        event: GameEvent,
        origin: Vector3<f64>,
        source_entity: Option<i32>,
        game_time: u64,
    ) {
        let source = source_entity.and_then(|id| world.get_entity_by_id(id));
        self.handle_vibration_from_entity(world, event, origin, source.as_deref(), game_time);
    }

    fn handle_vibration_from_entity(
        &self,
        world: &Arc<World>,
        event: GameEvent,
        origin: Vector3<f64>,
        source: Option<&dyn EntityBase>,
        game_time: u64,
    ) {
        self.vibration_data()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .handle_game_event_from_entity(
                world,
                self.as_vibration_user(),
                event,
                origin,
                source,
                game_time,
            );
    }

    /// Advance this listener one tick, delivering a vibration if one arrives.
    /// `VibrationSystem.Ticker.tick`.
    fn tick_vibration(&self, world: &Arc<World>, game_time: u64) {
        // The lock is released before delivery: `on_receive_vibration` changes block
        // state and emits its own game events, which come back through this listener.
        let (landed, changed) = {
            let mut data = self
                .vibration_data()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut changed = false;
            if data.current.is_none() {
                data.try_select_and_schedule(self.as_vibration_user(), game_time);
                if let Some(current) = &data.current {
                    // New vibrations get one initial particle broadcast. Only a
                    // vibration restored from NBT retries its particle on later ticks.
                    send_vibration_particle(
                        world,
                        current.origin,
                        self.listener_pos(),
                        data.travel_time,
                    );
                    changed = true;
                }
            }
            if data.reload_particle {
                if let Some(current) = &data.current {
                    let initial = self.calculate_travel_time(current.distance);
                    let alpha = if initial > 0 {
                        1.0 - f64::from(data.travel_time) / f64::from(initial)
                    } else {
                        0.0
                    };
                    let origin = current.origin
                        + (self.listener_pos().to_centered_f64() - current.origin) * alpha;
                    data.reload_particle = !send_vibration_particle(
                        world,
                        origin,
                        self.listener_pos(),
                        data.travel_time,
                    );
                } else {
                    data.reload_particle = false;
                }
            }
            let landed = if data.current.is_some() {
                let was_travelling = data.travel_time > 0;
                data.travel_time = (data.travel_time - 1).max(0);
                if data.travel_time == 0 {
                    if adjacent_chunks_tick(world, self.listener_pos()) {
                        changed = true;
                        data.current.take()
                    } else {
                        None
                    }
                } else {
                    changed |= was_travelling;
                    None
                }
            } else {
                None
            };
            (landed, changed)
        };
        if changed {
            // VibrationUser.onDataChanged -> BlockEntity.setChanged. This marks the
            // chunk for saving without sending listener internals to the client.
            world
                .level
                .read_chunk_sync(&self.listener_pos().chunk_position(), |chunk| {
                    chunk.mark_dirty(true)
                });
        }
        let Some(landed) = landed else {
            return;
        };

        let origin = BlockPos::floored(landed.origin.x, landed.origin.y, landed.origin.z);
        let dest = self.listener_pos();
        // `Listener.distanceBetweenInBlocks` measures block to block, not from the precise
        // event position, so a sensor's redstone output only steps in whole blocks.
        let distance_sq = (dest.0.x - origin.0.x).pow(2)
            + (dest.0.y - origin.0.y).pow(2)
            + (dest.0.z - origin.0.z).pow(2);
        self.on_receive_vibration(
            world,
            &origin,
            landed.event,
            landed.resolve_source(world),
            (f64::from(distance_sq).sqrt()) as f32,
        );
    }
}

fn adjacent_chunks_tick(world: &World, pos: BlockPos) -> bool {
    let chunk = pos.chunk_position();
    let active = world
        .active_chunks
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for x in chunk.x - 1..=chunk.x + 1 {
        for z in chunk.y - 1..=chunk.y + 1 {
            let pos = pumpkin_util::math::vector2::Vector2::new(x, z);
            if !active.contains(&pos) || !world.level.is_chunk_loaded(&pos) {
                return false;
            }
        }
    }
    true
}

/// VibrationParticleOption.STREAM_CODEC: block position source (registry id 0),
/// packed destination, then travel ticks. Send only to nearby Java players.
fn send_vibration_particle(
    world: &World,
    origin: Vector3<f64>,
    destination: BlockPos,
    ticks: i32,
) -> bool {
    use pumpkin_protocol::{
        codec::var_int::VarInt, java::client::play::CParticle, ser::NetworkWriteExt,
    };
    let mut bytes = Vec::new();
    if bytes.write_var_int(&VarInt(0)).is_err()
        || bytes.write_i64_be(destination.as_long()).is_err()
        || bytes.write_var_int(&VarInt(ticks.max(0))).is_err()
    {
        return false;
    }
    let packet = CParticle::new(
        false,
        false,
        origin,
        Vector3::new(0.0, 0.0, 0.0),
        0.0,
        1,
        VarInt(pumpkin_data::particle::Particle::Vibration.to_id() as i32),
        &bytes,
    );
    let recipients = world.get_nearby_players(origin, 32.0);
    let mut sent = false;
    for player in &recipients {
        if let crate::net::ClientPlatform::Java(client) = player.client.as_ref()
            && let Ok(data) = client.serialize_packet(&packet)
        {
            client.try_enqueue_packet(data);
            sent = true;
        }
    }
    sent
}

/// `VibrationSystem.User.isValidVibration` (VibrationSystem.java:414).
fn is_valid_vibration(
    user: &dyn VibrationUser,
    event: GameEvent,
    source: Option<&dyn EntityBase>,
) -> bool {
    if !event_in_tag(event, &user.listenable_events()) {
        return false;
    }
    let Some(entity) = source else {
        return true;
    };

    // A spectator makes no sound at all.
    if entity
        .get_player()
        .is_some_and(crate::entity::player::Player::is_spectator)
    {
        return false;
    }

    // "Stepping carefully" is sneaking: it hides the events in
    // #ignore_vibrations_sneaking, which is why you can walk past a sensor crouched.
    let stepping_carefully = entity
        .get_entity()
        .sneaking
        .load(std::sync::atomic::Ordering::Relaxed);
    if stepping_carefully
        && event_in_tag(
            event,
            &pumpkin_data::tag::GameEvent::MINECRAFT_IGNORE_VIBRATIONS_SNEAKING,
        )
    {
        return false;
    }

    // Dropped wool items and a warden's own footsteps dampen vibrations.
    !entity_dampens_vibrations(entity)
}

/// Entity.java:1555, ItemEntity.java:85 and Warden.java:195. Equipped wool does
/// not silence living entities; dropped items in #dampens_vibrations are silent.
fn entity_dampens_vibrations(entity: &dyn crate::entity::EntityBase) -> bool {
    use pumpkin_data::tag::{Item, Taggable};
    entity.get_entity().entity_type == &pumpkin_data::entity::EntityType::WARDEN
        || entity.get_item_entity().is_some_and(|item| {
            item.get_item_stack()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .item
                .has_tag(&Item::MINECRAFT_DAMPENS_VIBRATIONS)
        })
}

/// `VibrationSystem.Listener.isOccluded` (VibrationSystem.java:263).
///
/// A vibration is blocked only when it is blocked from *every* direction: the check nudges
/// the source a hair along each of the six faces and reports occluded only if all six rays
/// hit a `#occludes_vibration_signals` block. That is why a wool block beside the source
/// does not hide it, but wool fully surrounding it does.
fn is_occluded(world: &Arc<World>, origin: Vector3<f64>, dest: Vector3<f64>) -> bool {
    use pumpkin_data::BlockDirection;

    let from = Vector3::new(
        origin.x.floor() + 0.5,
        origin.y.floor() + 0.5,
        origin.z.floor() + 0.5,
    );
    let to = Vector3::new(
        dest.x.floor() + 0.5,
        dest.y.floor() + 0.5,
        dest.z.floor() + 0.5,
    );

    const NUDGE: f64 = 1.0E-5;
    for direction in BlockDirection::all() {
        let offset = direction.to_offset();
        let nudged = Vector3::new(
            from.x + f64::from(offset.x) * NUDGE,
            from.y + f64::from(offset.y) * NUDGE,
            from.z + f64::from(offset.z) * NUDGE,
        );
        if !ray_hits_occluding_block(world, nudged, to) {
            return false;
        }
    }
    true
}

/// `Level.isBlockInLine` restricted to `#occludes_vibration_signals`: walks the blocks
/// between the two points and reports whether any of them blocks vibrations.
fn ray_hits_occluding_block(world: &Arc<World>, from: Vector3<f64>, to: Vector3<f64>) -> bool {
    use pumpkin_data::tag::{Block as BlockTag, Taggable};

    // BlockGetter.traverseBlocks: traverse every intersected voxel. Sampling a
    // ray every half block can miss arbitrarily short corner intersections.
    let start = from + (from - to) * 1.0E-7;
    let end = to + (to - from) * 1.0E-7;
    let delta = end - start;
    let mut pos = BlockPos::floored(start.x, start.y, start.z);
    let step = [
        delta.x.signum() as i32,
        delta.y.signum() as i32,
        delta.z.signum() as i32,
    ];
    let d = [delta.x, delta.y, delta.z];
    let start_axes = [start.x, start.y, start.z];
    let mut next = [f64::INFINITY; 3];
    let mut stride = [f64::INFINITY; 3];
    for axis in 0..3 {
        if d[axis] != 0.0 {
            stride[axis] = 1.0 / d[axis].abs();
            let fraction = start_axes[axis] - start_axes[axis].floor();
            next[axis] = stride[axis]
                * if step[axis] > 0 {
                    1.0 - fraction
                } else {
                    fraction
                };
        }
    }
    loop {
        if world
            .get_block(&pos)
            .has_tag(&BlockTag::MINECRAFT_OCCLUDES_VIBRATION_SIGNALS)
        {
            return true;
        }
        // Strict comparisons reproduce vanilla's Z/Y/X tie-breaking.
        let axis = if next[0] < next[1] {
            if next[0] < next[2] { 0 } else { 2 }
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        if next[axis] > 1.0 {
            break;
        }
        match axis {
            0 => pos.0.x += step[0],
            1 => pos.0.y += step[1],
            _ => pos.0.z += step[2],
        }
        next[axis] += stride[axis];
    }
    false
}

/// Deliver a game event to every vibration listener in range.
///
/// Called from `World::emit_game_event`, which is the single point every game event in
/// the server passes through.
pub fn dispatch(
    world: &Arc<World>,
    event: GameEvent,
    origin: Vector3<f64>,
    source_entity: Option<i32>,
) {
    let source = source_entity.and_then(|id| world.get_entity_by_id(id));
    dispatch_from_entity(world, event, origin, source.as_deref());
}

pub fn dispatch_from_entity(
    world: &Arc<World>,
    event: GameEvent,
    origin: Vector3<f64>,
    source: Option<&dyn EntityBase>,
) {
    // An event no sensor has a frequency for can never produce a vibration, so skip the
    // whole search rather than waking every nearby block entity for it.
    if vibration_frequency(event) == NO_VIBRATION_FREQUENCY
        && !event_in_tag(
            event,
            &pumpkin_data::tag::GameEvent::MINECRAFT_SHRIEKER_CAN_LISTEN,
        )
    {
        return;
    }

    let game_time = world
        .level_time
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .world_age as u64;
    let radius = f64::from(MAX_LISTENER_RADIUS);
    let min = BlockPos::floored(origin.x - radius, origin.y - radius, origin.z - radius);
    let max = BlockPos::floored(origin.x + radius, origin.y + radius, origin.z + radius);

    let min_chunk = min.chunk_and_chunk_relative_position().0;
    let max_chunk = max.chunk_and_chunk_relative_position().0;

    for chunk_x in min_chunk.x..=max_chunk.x {
        for chunk_z in min_chunk.y..=max_chunk.y {
            let Some(chunk) = world
                .block_entities
                .get(&pumpkin_util::math::vector2::Vector2::new(chunk_x, chunk_z))
            else {
                continue;
            };
            for block_entity in chunk.values() {
                let pos = block_entity.get_position();
                if pos.0.x < min.0.x
                    || pos.0.x > max.0.x
                    || pos.0.y < min.0.y
                    || pos.0.y > max.0.y
                    || pos.0.z < min.0.z
                    || pos.0.z > max.0.z
                {
                    continue;
                }
                let Some(listener) = block_entity.as_vibration_listener() else {
                    continue;
                };
                // The listener's own radius, not the search bound, decides range.
                let radius = f64::from(listener.listener_radius());
                let dx = f64::from(pos.0.x) + 0.5 - origin.x;
                let dy = f64::from(pos.0.y) + 0.5 - origin.y;
                let dz = f64::from(pos.0.z) + 0.5 - origin.z;
                if dx * dx + dy * dy + dz * dz > radius * radius {
                    continue;
                }
                listener.handle_vibration_from_entity(world, event, origin, source, game_time);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frequency table is what a comparator on a sculk sensor reads out, so these
    /// numbers are directly observable and worth pinning verbatim against
    /// VibrationSystem.java:54-99.
    #[test]
    fn frequencies_match_the_vanilla_table() {
        assert_eq!(vibration_frequency(GameEvent::Step), 1);
        assert_eq!(vibration_frequency(GameEvent::Splash), 2);
        assert_eq!(vibration_frequency(GameEvent::ProjectileShoot), 3);
        assert_eq!(vibration_frequency(GameEvent::EntityAction), 4);
        assert_eq!(vibration_frequency(GameEvent::Equip), 5);
        assert_eq!(vibration_frequency(GameEvent::EntityInteract), 6);
        assert_eq!(vibration_frequency(GameEvent::EntityDamage), 7);
        assert_eq!(vibration_frequency(GameEvent::Eat), 8);
        assert_eq!(vibration_frequency(GameEvent::BlockClose), 9);
        assert_eq!(vibration_frequency(GameEvent::BlockOpen), 10);
        assert_eq!(vibration_frequency(GameEvent::BlockChange), 11);
        assert_eq!(vibration_frequency(GameEvent::BlockDestroy), 12);
        assert_eq!(vibration_frequency(GameEvent::BlockPlace), 13);
        assert_eq!(vibration_frequency(GameEvent::Teleport), 14);
        assert_eq!(vibration_frequency(GameEvent::EntityDie), 15);
    }

    #[test]
    fn events_outside_the_table_produce_no_vibration() {
        // Not every game event is a vibration; these must stay silent or a sensor would
        // fire on things vanilla ignores.
        assert_eq!(
            vibration_frequency(GameEvent::SculkSensorTendrilsClicking),
            NO_VIBRATION_FREQUENCY
        );
    }

    #[test]
    fn resonance_events_carry_their_own_frequency() {
        // An amethyst block beside a calibrated sensor re-emits at the frequency it
        // resonated at, so the round trip has to be exact for all fifteen.
        for frequency in 1..=15u8 {
            let event = resonance_event_for_frequency(frequency)
                .expect("every frequency 1..=15 has a resonance event");
            assert_eq!(vibration_frequency(event), frequency);
        }
        assert!(resonance_event_for_frequency(0).is_none());
        assert!(resonance_event_for_frequency(16).is_none());
    }

    /// `getRedstoneStrengthForDistance` (VibrationSystem.java:120) with the sensor's
    /// radius of 8: adjacent is full strength, and the far edge still reads 1.
    #[test]
    fn redstone_strength_falls_off_with_distance() {
        assert_eq!(redstone_strength_for_distance(0.0, 8), 15);
        assert_eq!(redstone_strength_for_distance(1.0, 8), 14);
        assert_eq!(redstone_strength_for_distance(4.0, 8), 8);
        assert_eq!(redstone_strength_for_distance(8.0, 8), 1);
        // Never zero, however far away -- a received vibration always powers the sensor.
        assert_eq!(redstone_strength_for_distance(100.0, 8), 1);
    }

    /// VibrationSelector.java:46: distance takes priority; frequency breaks a tie.
    #[test]
    fn the_nearest_candidate_in_a_tick_wins() {
        let at = |event, distance| VibrationInfo {
            event,
            distance,
            origin: Vector3::new(0.0, 0.0, 0.0),
            source_entity: None,
            source_uuid: None,
            projectile_owner_uuid: None,
        };

        let mut data = VibrationData::default();
        data.add_candidate(at(GameEvent::Step, 1.0), 100); // frequency 1
        data.add_candidate(at(GameEvent::EntityDie, 7.0), 100); // frequency 15
        assert_eq!(
            data.candidate.as_ref().map(|(c, _)| c.event),
            Some(GameEvent::Step),
            "distance takes priority over frequency"
        );

        // An even nearer candidate replaces it regardless of frequency.
        data.add_candidate(at(GameEvent::Step, 0.5), 100);
        assert_eq!(
            data.candidate.as_ref().map(|(c, _)| c.event),
            Some(GameEvent::Step)
        );
    }

    #[test]
    fn equal_frequencies_are_broken_by_distance() {
        let at = |distance| VibrationInfo {
            event: GameEvent::Step,
            distance,
            origin: Vector3::new(0.0, 0.0, 0.0),
            source_entity: None,
            source_uuid: None,
            projectile_owner_uuid: None,
        };
        let mut data = VibrationData::default();
        data.add_candidate(at(5.0), 100);
        data.add_candidate(at(2.0), 100);
        assert_eq!(data.candidate.as_ref().map(|(c, _)| c.distance), Some(2.0));
        data.add_candidate(at(9.0), 100);
        assert_eq!(
            data.candidate.as_ref().map(|(c, _)| c.distance),
            Some(2.0),
            "a further event of the same frequency must not displace a closer one"
        );
    }

    /// A candidate is only eligible on a later tick than it arrived on. That one-tick
    /// wait is what gives competing events in the same tick a chance to be compared.
    #[test]
    fn a_candidate_is_not_chosen_in_its_own_tick() {
        struct Dummy;
        impl VibrationUser for Dummy {
            fn listener_radius(&self) -> i32 {
                8
            }
            fn listener_pos(&self) -> BlockPos {
                BlockPos::new(0, 0, 0)
            }
            fn can_receive_vibration(
                &self,
                _: &Arc<World>,
                _: &BlockPos,
                _: GameEvent,
                _: Option<i32>,
            ) -> bool {
                true
            }
            fn on_receive_vibration(
                &self,
                _: &Arc<World>,
                _: &BlockPos,
                _: GameEvent,
                _: Option<i32>,
                _: f32,
            ) {
            }
        }

        let mut data = VibrationData::default();
        data.add_candidate(
            VibrationInfo {
                event: GameEvent::Step,
                distance: 3.0,
                origin: Vector3::new(0.0, 0.0, 0.0),
                source_entity: None,
                source_uuid: None,
                projectile_owner_uuid: None,
            },
            100,
        );

        data.try_select_and_schedule(&Dummy, 100);
        assert!(
            !data.has_vibration_in_flight(),
            "the arrival tick itself must not select the candidate"
        );

        data.try_select_and_schedule(&Dummy, 101);
        assert!(data.has_vibration_in_flight(), "the next tick selects it");
        assert_eq!(data.travel_time, 3, "one tick of travel per block, floored");
    }
}
