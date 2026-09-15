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
//! largest listener radius in the game is 8 blocks. Registering and unregistering
//! listeners as chunks and block entities come and go is a large amount of bookkeeping
//! for no observable difference, so it is not reproduced.

use std::sync::Arc;

use pumpkin_data::game_event::GameEvent;
use pumpkin_data::tag::Tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::world::World;

/// `VibrationSystem.NO_VIBRATION_FREQUENCY`: an event no sensor reacts to.
pub const NO_VIBRATION_FREQUENCY: u8 = 0;

/// The largest `getListenerRadius` any vibration user reports. Both the sculk sensor
/// (SculkSensorBlockEntity.VibrationUser.LISTENER_RANGE) and the shrieker use 8, and
/// [`dispatch`] uses this to bound its search.
pub const MAX_LISTENER_RADIUS: i32 = 8;

/// `VibrationSystem.VIBRATION_FREQUENCY_FOR_EVENT` (VibrationSystem.java:54-99).
///
/// This is the number a sculk sensor reports through a comparator, so the exact value per
/// event is observable in-game and worth having verbatim rather than approximated.
#[must_use]
pub fn vibration_frequency(event: GameEvent) -> u8 {
    match event {
        GameEvent::Step | GameEvent::Swim | GameEvent::Flap => 1,
        GameEvent::ProjectileLand | GameEvent::HitGround | GameEvent::Splash
        | GameEvent::Bounce => 2,
        GameEvent::ItemInteractFinish | GameEvent::ProjectileShoot
        | GameEvent::InstrumentPlay => 3,
        GameEvent::EntityAction | GameEvent::ElytraGlide | GameEvent::Unequip => 4,
        GameEvent::EntityDismount | GameEvent::Equip => 5,
        GameEvent::EntityInteract | GameEvent::Shear | GameEvent::EntityMount => 6,
        GameEvent::EntityDamage => 7,
        GameEvent::Drink | GameEvent::Eat => 8,
        GameEvent::ContainerClose | GameEvent::BlockClose | GameEvent::BlockDeactivate
        | GameEvent::BlockDetach => 9,
        GameEvent::ContainerOpen | GameEvent::BlockOpen | GameEvent::BlockActivate
        | GameEvent::BlockAttach | GameEvent::PrimeFuse | GameEvent::NoteBlockPlay => 10,
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
    fn can_receive_vibration(&self, world: &Arc<World>, origin: &BlockPos, event: GameEvent)
    -> bool;

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

impl VibrationData {
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
        // A listener already carrying a vibration ignores everything until it lands.
        if self.current.is_some() {
            return false;
        }
        if !is_valid_vibration(world, user, event, source_entity) {
            return false;
        }

        let dest_block = user.listener_pos();
        let dest = Vector3::new(
            f64::from(dest_block.0.x) + 0.5,
            f64::from(dest_block.0.y) + 0.5,
            f64::from(dest_block.0.z) + 0.5,
        );

        let origin_block = BlockPos::floored(origin.x, origin.y, origin.z);
        if !user.can_receive_vibration(world, &origin_block, event) {
            return false;
        }
        if is_occluded(world, origin, dest) {
            return false;
        }

        let distance = (origin.squared_distance_to_vec(&dest)).sqrt() as f32;
        self.add_candidate(
            VibrationInfo {
                event,
                distance,
                origin,
                source_entity,
            },
            game_time,
        );
        true
    }

    /// `VibrationSelector.addCandidate`: a new event replaces the stored one only if it
    /// is strictly better -- higher frequency wins, and on a tie the closer one does.
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
                    new_frequency > old_frequency
                        || (new_frequency == old_frequency
                            && candidate.distance < current.distance)
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

    /// Offer a game event to this listener. `VibrationSystem.Listener.handleGameEvent`.
    fn handle_vibration(
        &self,
        world: &Arc<World>,
        event: GameEvent,
        origin: Vector3<f64>,
        source_entity: Option<i32>,
        game_time: u64,
    ) {
        let mut data = self
            .vibration_data()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        data.handle_game_event(
            world,
            self.as_vibration_user(),
            event,
            origin,
            source_entity,
            game_time,
        );
    }

    /// Advance this listener one tick, delivering a vibration if one arrives.
    /// `VibrationSystem.Ticker.tick`.
    fn tick_vibration(&self, world: &Arc<World>, game_time: u64) {
        // The lock is released before delivery: `on_receive_vibration` changes block
        // state and emits its own game events, which come back through this listener.
        let landed = {
            let mut data = self
                .vibration_data()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            data.tick(self.as_vibration_user(), game_time)
        };
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
            landed.source_entity,
            (f64::from(distance_sq).sqrt()) as f32,
        );
    }
}

/// `VibrationSystem.User.isValidVibration` (VibrationSystem.java:414).
fn is_valid_vibration(
    world: &Arc<World>,
    user: &dyn VibrationUser,
    event: GameEvent,
    source_entity: Option<i32>,
) -> bool {
    if !event_in_tag(event, &user.listenable_events()) {
        return false;
    }

    let Some(entity_id) = source_entity else {
        return true;
    };
    let Some(entity) = world.get_entity_by_id(entity_id) else {
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
        && event_in_tag(event, &pumpkin_data::tag::GameEvent::MINECRAFT_IGNORE_VIBRATIONS_SNEAKING)
    {
        return false;
    }

    // Wool armour (and a warden's own footsteps) dampen vibrations entirely.
    !entity_dampens_vibrations(entity.as_ref())
}

/// `Entity.dampensVibrations`. Only the wool-wearing case and the warden apply.
fn entity_dampens_vibrations(entity: &dyn crate::entity::EntityBase) -> bool {
    use pumpkin_data::data_component_impl::EquipmentSlot;
    use pumpkin_data::tag::{Item, Taggable};

    if entity.get_entity().entity_type == &pumpkin_data::entity::EntityType::WARDEN {
        return true;
    }
    let Some(living) = entity.get_living_entity() else {
        return false;
    };
    let equipment = living
        .entity_equipment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    equipment
        .equipment
        .iter()
        .any(|(slot, stack)| *slot != EquipmentSlot::MAIN_HAND
            && *slot != EquipmentSlot::OFF_HAND
            && stack.item.has_tag(&Item::MINECRAFT_WOOL_CARPETS))
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

    let delta = Vector3::new(to.x - from.x, to.y - from.y, to.z - from.z);
    let length = (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt();
    if length < 1.0E-7 {
        return false;
    }
    // One sample per half block is enough to never skip a unit cube on the way.
    let steps = (length * 2.0).ceil() as i32;
    for step in 0..=steps {
        let t = f64::from(step) / f64::from(steps);
        let x = from.x + delta.x * t;
        let y = from.y + delta.y * t;
        let z = from.z + delta.z * t;
        let pos = BlockPos::floored(x, y, z);
        if world
            .get_block(&pos)
            .has_tag(&BlockTag::MINECRAFT_OCCLUDES_VIBRATION_SIGNALS)
        {
            return true;
        }
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
    // An event no sensor has a frequency for can never produce a vibration, so skip the
    // whole search rather than waking every nearby block entity for it.
    if vibration_frequency(event) == NO_VIBRATION_FREQUENCY
        && !event_in_tag(event, &pumpkin_data::tag::GameEvent::MINECRAFT_SHRIEKER_CAN_LISTEN)
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
                listener.handle_vibration(world, event, origin, source_entity, game_time);
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

    /// `VibrationSelector.addCandidate`: within one tick the strongest event wins, and
    /// distance only breaks a tie.
    #[test]
    fn the_strongest_candidate_in_a_tick_wins() {
        let at = |event, distance| VibrationInfo {
            event,
            distance,
            origin: Vector3::new(0.0, 0.0, 0.0),
            source_entity: None,
        };

        let mut data = VibrationData::default();
        data.add_candidate(at(GameEvent::Step, 1.0), 100); // frequency 1
        data.add_candidate(at(GameEvent::EntityDie, 7.0), 100); // frequency 15
        assert_eq!(
            data.candidate.as_ref().map(|(c, _)| c.event),
            Some(GameEvent::EntityDie),
            "a louder event must win even from further away"
        );

        // A quieter one arriving afterwards must not displace it.
        data.add_candidate(at(GameEvent::Step, 0.5), 100);
        assert_eq!(
            data.candidate.as_ref().map(|(c, _)| c.event),
            Some(GameEvent::EntityDie)
        );
    }

    #[test]
    fn equal_frequencies_are_broken_by_distance() {
        let at = |distance| VibrationInfo {
            event: GameEvent::Step,
            distance,
            origin: Vector3::new(0.0, 0.0, 0.0),
            source_entity: None,
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
            fn can_receive_vibration(&self, _: &Arc<World>, _: &BlockPos, _: GameEvent) -> bool {
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
