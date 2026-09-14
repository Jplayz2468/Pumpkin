use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use pumpkin_data::packet::serverbound::play::MOVE_PLAYER_ROT;
use pumpkin_macros::java_packet;
use pumpkin_util::version::JavaMinecraftVersion;

#[java_packet(MOVE_PLAYER_ROT)]
pub struct SPlayerRotation {
    pub yaw: f32,
    pub pitch: f32,
    pub ground: bool,
    pub horizontal_collision: bool,
}

impl<'a> ServerPacket<'a> for SPlayerRotation {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let yaw = bytebuf.get_f32_be()?;
        let pitch = bytebuf.get_f32_be()?;
        let flags = bytebuf.get_u8()?;
        Ok(Self {
            yaw,
            pitch,
            ground: flags & 1 != 0,
            horizontal_collision: flags & 2 != 0,
        })
    }
}

impl crate::ClientPacket for SPlayerRotation {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_f32_be(self.yaw)?;
        write.write_f32_be(self.pitch)?;
        write.write_u8(u8::from(self.ground) | (u8::from(self.horizontal_collision) << 1))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_collision_is_not_on_ground() {
        for flags in 0u8..=3 {
            let mut bytes = vec![0; 8];
            bytes.push(flags);
            let packet =
                SPlayerRotation::read(&mut bytes.as_slice(), &JavaMinecraftVersion::V_1_21_6)
                    .unwrap();
            assert_eq!(packet.ground, flags & 1 != 0);
            assert_eq!(packet.horizontal_collision, flags & 2 != 0);
        }
    }
}
