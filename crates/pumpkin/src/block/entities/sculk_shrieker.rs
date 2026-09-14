use super::BlockEntity;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::sync::Mutex;

pub struct SculkShriekerBlockEntity {
    pub position: BlockPos,
    pub warning_level: Mutex<i32>,
}

impl BlockEntity for SculkShriekerBlockEntity {
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
        let warning_level = nbt.get_int("warning_level").unwrap_or(0);
        Self {
            position,
            warning_level: Mutex::new(warning_level),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(warning_level) = self.warning_level.lock() {
            nbt.put_int("warning_level", *warning_level);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_int("warning_level", *self.warning_level.try_lock().ok()?);
        Some(nbt)
    }

    fn on_block_replaced_with_state(
        self: std::sync::Arc<Self>,
        world: &std::sync::Arc<crate::world::World>,
        position: &BlockPos,
        old_state: pumpkin_data::BlockStateId,
    ) {
        let props =
            pumpkin_data::block_properties::SculkShriekerLikeProperties::from_state_id(old_state);
        if props.shrieking {
            let warning = *self
                .warning_level
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            crate::block::blocks::sculk::sculk_shrieker::SculkShriekerBlock::respond_with_warning(
                world,
                position,
                props.can_summon,
                warning,
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SculkShriekerBlockEntity {
    pub const ID: &'static str = "minecraft:sculk_shrieker";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            warning_level: Mutex::new(0),
        }
    }
}
