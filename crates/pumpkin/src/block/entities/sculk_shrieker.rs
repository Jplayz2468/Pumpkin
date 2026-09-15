use super::BlockEntity;
use crate::block::blocks::sculk::vibration::{VibrationData, VibrationListener, VibrationUser};
use crate::world::World;
use pumpkin_data::game_event::GameEvent;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;
use std::sync::Mutex;

pub struct SculkShriekerBlockEntity {
    pub position: BlockPos,
    pub listener: Mutex<VibrationData>,
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
            listener: Mutex::new(VibrationData::from_nbt(nbt)),
            warning_level: Mutex::new(warning_level),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.listener
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);
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

    fn tick(&self, world: &Arc<World>) {
        let time = world
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age as u64;
        self.tick_vibration(world, time);
    }

    fn as_vibration_listener(&self) -> Option<&dyn VibrationListener> {
        Some(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SculkShriekerBlockEntity {
    pub const ID: &'static str = "minecraft:sculk_shrieker";
    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            listener: Mutex::new(VibrationData::default()),
            warning_level: Mutex::new(0),
        }
    }
}

impl VibrationListener for SculkShriekerBlockEntity {
    fn vibration_data(&self) -> &Mutex<VibrationData> {
        &self.listener
    }
    fn as_vibration_user(&self) -> &dyn VibrationUser {
        self
    }
}

impl VibrationUser for SculkShriekerBlockEntity {
    fn listener_radius(&self) -> i32 {
        8
    }
    fn listener_pos(&self) -> BlockPos {
        self.position
    }
    fn listenable_events(&self) -> pumpkin_data::tag::Tag {
        pumpkin_data::tag::GameEvent::MINECRAFT_SHRIEKER_CAN_LISTEN
    }
    fn can_receive_vibration(
        &self,
        world: &Arc<World>,
        _origin: &BlockPos,
        _event: GameEvent,
        source_entity: Option<i32>,
    ) -> bool {
        use pumpkin_data::block_properties::SculkShriekerLikeProperties;
        !SculkShriekerLikeProperties::from_state_id(world.get_block_state(&self.position).id)
            .shrieking
            && triggering_player(world, source_entity).is_some()
    }
    fn on_receive_vibration(
        &self,
        world: &Arc<World>,
        _origin: &BlockPos,
        _event: GameEvent,
        source_entity: Option<i32>,
        _distance: f32,
    ) {
        if let Some(player) = triggering_player(world, source_entity) {
            crate::block::blocks::sculk::sculk_shrieker::SculkShriekerBlock::try_activate(
                world,
                &self.position,
                &player,
            );
        }
    }
}

/// SculkShriekerBlockEntity.tryGetPlayer: direct players, controlling riders,
/// and the owner of projectiles can trigger a shriek.
fn triggering_player(
    world: &World,
    source: Option<i32>,
) -> Option<Arc<crate::entity::player::Player>> {
    let source = source?;
    if let Some(player) = world
        .players
        .load()
        .iter()
        .find(|p| p.living_entity.entity.entity_id == source)
        .cloned()
    {
        return Some(player);
    }
    let entity = world.get_entity_by_id(source)?;
    if let Some(owner) = entity.get_owner_id() {
        return world
            .players
            .load()
            .iter()
            .find(|p| p.living_entity.entity.entity_id == owner)
            .cloned();
    }
    let passengers = entity
        .get_entity()
        .passengers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let rider = passengers.first()?.get_entity().entity_id;
    world
        .players
        .load()
        .iter()
        .find(|p| p.living_entity.entity.entity_id == rider)
        .cloned()
}
