use pumpkin_data::packet::clientbound::play::SET_BORDER_WARNING_DELAY;
use pumpkin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use pumpkin_util::version::JavaMinecraftVersion;

#[java_packet(SET_BORDER_WARNING_DELAY)]
pub struct CSetBorderWarningDelay {
    pub warning_time: VarInt,
}

impl CSetBorderWarningDelay {
    #[must_use]
    pub const fn new(warning_time: VarInt) -> Self {
        Self { warning_time }
    }
}

impl ClientPacket for CSetBorderWarningDelay {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        let warning = if *version < JavaMinecraftVersion::V_1_21_11 {
            VarInt(self.warning_time.0 / 20)
        } else {
            self.warning_time
        };
        write.write_var_int(&warning)?;
        Ok(())
    }
}
