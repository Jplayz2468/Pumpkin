use pumpkin_data::packet::clientbound::play::INITIALIZE_BORDER;
use pumpkin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use crate::{VarInt, codec::var_long::VarLong};
use pumpkin_util::version::JavaMinecraftVersion;

/// Fully initializes the world border for the client.
///
/// This packet is sent when a player joins the world or changes dimensions.
/// It synchronizes the current position, size, and all warning parameters
/// to ensure the client-side visual barrier matches the server's authority.
#[java_packet(INITIALIZE_BORDER)]
pub struct CInitializeWorldBorder {
    /// The X coordinate of the center of the world border.
    pub x: f64,
    /// The Z coordinate of the center of the world border.
    pub z: f64,
    /// The diameter the border is moving from.
    pub old_diameter: f64,
    /// The diameter the border is moving toward.
    pub new_diameter: f64,
    /// The time (in milliseconds) it will take to reach `new_diameter`.
    pub speed: VarLong,
    /// The maximum distance a player can be teleported by a portal
    /// before the border prevents the teleport.
    pub portal_teleport_boundary: VarInt,
    /// Distance in blocks from the border where the screen starts to tint red.
    pub warning_blocks: VarInt,
    /// Time in ticks that a player must be on a collision course with
    /// the border before the warning tint appears.
    pub warning_time: VarInt,
}

impl CInitializeWorldBorder {
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        x: f64,
        z: f64,
        old_diameter: f64,
        new_diameter: f64,
        speed: VarLong,
        portal_teleport_boundary: VarInt,
        warning_blocks: VarInt,
        warning_time: VarInt,
    ) -> Self {
        Self {
            x,
            z,
            old_diameter,
            new_diameter,
            speed,
            portal_teleport_boundary,
            warning_blocks,
            warning_time,
        }
    }
}

impl ClientPacket for CInitializeWorldBorder {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_f64_be(self.x)?;
        write.write_f64_be(self.z)?;
        write.write_f64_be(self.old_diameter)?;
        write.write_f64_be(self.new_diameter)?;
        // The public packet field retains milliseconds for compatibility with plugins.
        let duration = if *version >= JavaMinecraftVersion::V_1_21_11 {
            VarLong(self.speed.0 / 50)
        } else {
            self.speed
        };
        write.write_var_long(&duration)?;
        write.write_var_int(&self.portal_teleport_boundary)?;
        write.write_var_int(&self.warning_blocks)?;
        let warning = if *version < JavaMinecraftVersion::V_1_21_11 {
            VarInt(self.warning_time.0 / 20)
        } else {
            self.warning_time
        };
        write.write_var_int(&warning)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java::client::play::{CSetBorderLerpSize, CSetBorderWarningDelay};
    #[test]
    fn border_packets_convert_legacy_milliseconds_and_seconds() {
        for (version, duration, warning) in [
            (JavaMinecraftVersion::V_1_21_9, 5000, 15),
            (JavaMinecraftVersion::V_1_21_11, 100, 300),
            (JavaMinecraftVersion::V_26_2, 100, 300),
        ] {
            let mut bytes = Vec::new();
            CInitializeWorldBorder::new(
                0.0,
                0.0,
                100.0,
                20.0,
                5000_i64.into(),
                29999984.into(),
                5.into(),
                300.into(),
            )
            .write_packet_data(&mut bytes, &version)
            .unwrap();
            let mut cursor = std::io::Cursor::new(bytes);
            cursor.set_position(32);
            assert_eq!(VarLong::decode(&mut cursor).unwrap().0, duration);
            assert_eq!(VarInt::decode(&mut cursor).unwrap().0, 29999984);
            assert_eq!(VarInt::decode(&mut cursor).unwrap().0, 5);
            assert_eq!(VarInt::decode(&mut cursor).unwrap().0, warning);
            let mut bytes = Vec::new();
            CSetBorderLerpSize::new(100.0, 20.0, 5000_i64.into())
                .write_packet_data(&mut bytes, &version)
                .unwrap();
            let mut cursor = std::io::Cursor::new(bytes);
            cursor.set_position(16);
            assert_eq!(VarLong::decode(&mut cursor).unwrap().0, duration);
            let mut bytes = Vec::new();
            CSetBorderWarningDelay::new(300.into())
                .write_packet_data(&mut bytes, &version)
                .unwrap();
            assert_eq!(
                VarInt::decode(&mut std::io::Cursor::new(bytes)).unwrap().0,
                warning
            );
        }
    }
}
