use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
};

use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;

use super::BlockEntity;

// TODO: component-form CustomName and LastOutput persistence.
pub struct CommandBlockEntity {
    pub position: BlockPos,
    pub powered: AtomicBool,
    pub condition_met: AtomicBool,
    pub auto: AtomicBool,
    pub dirty: AtomicBool,
    pub command: StdMutex<String>,
    pub last_output: StdMutex<String>,
    pub track_output: AtomicBool,
    pub success_count: AtomicU32,
    pub last_execution: AtomicI64,
    pub update_last_execution: AtomicBool,
}

impl CommandBlockEntity {
    pub const ID: &'static str = "minecraft:command_block";
    #[must_use]
    pub const fn new(position: BlockPos, track_output: bool, is_chain: bool) -> Self {
        Self {
            position,
            powered: AtomicBool::new(false),
            condition_met: AtomicBool::new(false),
            auto: AtomicBool::new(is_chain),
            dirty: AtomicBool::new(false),
            command: StdMutex::new(String::new()),
            last_output: StdMutex::new(String::new()),
            track_output: AtomicBool::new(track_output),
            success_count: AtomicU32::new(0),
            last_execution: AtomicI64::new(-1),
            update_last_execution: AtomicBool::new(true),
        }
    }

    fn write_sync_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("auto", self.auto.load(Ordering::SeqCst));
        nbt.put_string(
            "Command",
            self.command
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .to_string(),
        );
        nbt.put_bool("conditionMet", self.condition_met.load(Ordering::SeqCst));
        nbt.put_string(
            "LastOutput",
            self.last_output
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .to_string(),
        );
        nbt.put_bool("powered", self.powered.load(Ordering::SeqCst));
        nbt.put_bool("TrackOutput", self.track_output.load(Ordering::SeqCst));
        let update_last = self.update_last_execution.load(Ordering::Relaxed);
        nbt.put_bool("UpdateLastExecution", update_last);
        let last = self.last_execution.load(Ordering::Relaxed);
        if update_last && last != -1 {
            nbt.put_long("LastExecution", last);
        }
        nbt.put_int(
            "SuccessCount",
            self.success_count.load(Ordering::SeqCst).cast_signed(),
        );
    }
}

impl BlockEntity for CommandBlockEntity {
    fn set_block_state(&mut self, state: pumpkin_data::BlockStateId) {
        self.auto.store(
            state.to_block() == &pumpkin_data::Block::CHAIN_COMMAND_BLOCK,
            Ordering::Relaxed,
        );
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let condition_met = AtomicBool::new(nbt.get_bool("conditionMet").unwrap_or(false));
        let auto = AtomicBool::new(nbt.get_bool("auto").unwrap_or(false));
        let powered = AtomicBool::new(nbt.get_bool("powered").unwrap_or(false));
        let command = StdMutex::new(nbt.get_string("Command").unwrap_or("").to_string());
        let last_output = StdMutex::new(nbt.get_string("LastOutput").unwrap_or("").to_string());
        let track_output = AtomicBool::new(nbt.get_bool("TrackOutput").unwrap_or(true));
        let success_count =
            AtomicU32::new(nbt.get_int("SuccessCount").unwrap_or(0).cast_unsigned());

        Self {
            position,
            condition_met,
            auto,
            powered,
            command,
            last_output,
            track_output,
            success_count,
            update_last_execution: AtomicBool::new(
                nbt.get_bool("UpdateLastExecution").unwrap_or(true),
            ),
            last_execution: AtomicI64::new(
                if nbt.get_bool("UpdateLastExecution").unwrap_or(true) {
                    nbt.get_long("LastExecution").unwrap_or(-1)
                } else {
                    -1
                },
            ),
            dirty: AtomicBool::new(false),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_sync_nbt(nbt);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.write_sync_nbt(&mut nbt);
        Some(nbt)
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn execution_tracking_defaults_and_disabled_reload_match_source() {
        let pos = BlockPos::new(0, 0, 0);
        let mut nbt = NbtCompound::new();
        let default = CommandBlockEntity::from_nbt(&nbt, pos);
        assert!(default.track_output.load(Ordering::Relaxed));
        assert!(default.update_last_execution.load(Ordering::Relaxed));
        assert_eq!(default.last_execution.load(Ordering::Relaxed), -1);
        nbt.put_long("LastExecution", 42);
        let enabled = CommandBlockEntity::from_nbt(&nbt, pos);
        let mut saved = NbtCompound::new();
        enabled.write_nbt(&mut saved);
        assert_eq!(saved.get_long("LastExecution"), Some(42));
        nbt.put_bool("UpdateLastExecution", false);
        let disabled = CommandBlockEntity::from_nbt(&nbt, pos);
        assert_eq!(disabled.last_execution.load(Ordering::Relaxed), -1);
        let mut saved = NbtCompound::new();
        disabled.write_nbt(&mut saved);
        assert_eq!(saved.get_long("LastExecution"), None);
    }
}
