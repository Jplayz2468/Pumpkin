//! ServerLevel sound recipient selection: full-precision position and strict radius.
use super::World;
use crate::entity::{Entity, EntityBase};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_protocol::{
    IdOr, SoundEvent,
    java::client::play::{CEntitySoundEffect, CSoundEffect},
};
use pumpkin_util::{math::vector3::Vector3, version::JavaMinecraftVersion};

fn range(sound: &IdOr<SoundEvent>, volume: f32) -> f64 {
    // All registered 26.2 SoundEvents have variable range. Direct component
    // events may carry their own fixed range, independently of their identifier.
    let fixed = match sound {
        IdOr::Value(sound) => sound.range,
        IdOr::Id(_) => None,
    };
    f64::from(fixed.unwrap_or(if volume > 1.0 {
        16.0_f32 * volume
    } else {
        16.0
    }))
}

fn in_range(source: Vector3<f64>, player: Vector3<f64>, radius: f64) -> bool {
    let x = source.x - player.x;
    let y = source.y - player.y;
    let z = source.z - player.z;
    x * x + y * y + z * z < radius * radius
}

impl World {
    pub(super) fn broadcast_sound(
        &self,
        packet: &CSoundEffect,
        source: Vector3<f64>,
        except: Option<uuid::Uuid>,
    ) {
        let radius = range(&packet.sound_event, packet.volume);
        let players = self.players.load();
        let recipients =
            Self::collect_java_recipients_by_version(players.iter().filter(|player| {
                Some(player.gameprofile.id) != except
                    && in_range(source, player.get_entity().pos.load(), radius)
            }));
        Self::broadcast_java_grouped(packet, recipients);
    }

    /// Entity-attached sounds follow the source after launch. Older clients without
    /// this packet receive the positional form at its current position.
    pub fn play_entity_sound(
        &self,
        entity: &Entity,
        sound: Sound,
        category: SoundCategory,
        volume: f32,
        pitch: f32,
    ) {
        let source = entity.pos.load();
        let sound = IdOr::Id(sound as u16);
        let radius = range(&sound, volume);
        let seed = self.next_sound_seed();
        let attached = CEntitySoundEffect::new(
            sound.clone(),
            category,
            entity.entity_id.into(),
            volume,
            pitch,
            seed,
        );
        let positional = CSoundEffect::new(sound, category, &source, volume, pitch, seed);
        let players = self.players.load();
        let recipients = Self::collect_java_recipients_by_version(
            players
                .iter()
                .filter(|player| in_range(source, player.get_entity().pos.load(), radius)),
        );
        for (version, clients) in recipients {
            let group = std::collections::BTreeMap::from([(version, clients)]);
            if version >= JavaMinecraftVersion::V_1_14 {
                Self::broadcast_java_grouped(&attached, group);
            } else {
                Self::broadcast_java_grouped(&positional, group);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_recipient_radius_and_component_ranges() {
        let generic = IdOr::Id(0);
        assert_eq!(range(&generic, 0.0), 16.0);
        assert_eq!(range(&generic, 1.25), 20.0);
        assert_eq!(range(&generic, f32::NAN), 16.0);
        let fixed = IdOr::Value(SoundEvent {
            sound_name: "minecraft:test".into(),
            range: Some(23.75),
        });
        assert_eq!(range(&fixed, 0.0), 23.75);
        assert_eq!(range(&fixed, 100.0), 23.75);
        let source = Vector3::new(-0.01, 64.0, 0.01);
        assert!(!in_range(source, source.add_raw(0.0, 16.0, 0.0), 16.0));
        assert!(in_range(source, source.add_raw(0.0, 15.999, 0.0), 16.0));
        assert!(!in_range(source, source.add_raw(12.0, 0.0, 12.0), 16.0));
        assert!(in_range(source, source.add_raw(-15.9, 0.0, 0.0), 16.0));
        assert!(!in_range(source, source, 0.0));
        // Filtering precedes packet coordinate quantization: use -0.01, not 0.
        assert!(!in_range(source, Vector3::new(15.995, 64.0, 0.01), 16.0));
    }
}
