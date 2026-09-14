use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use pumpkin_data::packet::serverbound::play::MOVE_PLAYER_STATUS_ONLY;
use pumpkin_macros::java_packet;
use pumpkin_util::version::JavaMinecraftVersion;

#[java_packet(MOVE_PLAYER_STATUS_ONLY)]
pub struct SSetPlayerGround {
    pub on_ground: bool,
    pub horizontal_collision: bool,
}

impl<'a> ServerPacket<'a> for SSetPlayerGround {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let flags = bytebuf.get_u8()?;
        Ok(Self {
            on_ground: flags & 1 != 0,
            horizontal_collision: flags & 2 != 0,
        })
    }
}

impl crate::ClientPacket for SSetPlayerGround {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_u8(u8::from(self.on_ground) | (u8::from(self.horizontal_collision) << 1))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_collision_is_not_on_ground() {
        for flags in 0u8..=3 {
            let mut bytes = vec![0; 0];
            bytes.push(flags);
            let packet =
                SSetPlayerGround::read(&mut bytes.as_slice(), &JavaMinecraftVersion::V_1_21_6)
                    .unwrap();
            assert_eq!(packet.on_ground, flags & 1 != 0);
            assert_eq!(packet.horizontal_collision, flags & 2 != 0);
        }
    }
}
