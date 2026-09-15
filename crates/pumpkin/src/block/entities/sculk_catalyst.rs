use super::BlockEntity;
use crate::{block::blocks::sculk::spreader::SculkSpreader, world::World};
use pumpkin_data::{
    Block, BlockId,
    advancement::Advancement,
    block_properties::SculkCatalystLikeProperties,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::{
    math::{position::BlockPos, vector2::Vector2, vector3::Vector3},
    random::RandomImpl,
};
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};
use std::sync::{Arc, Mutex, atomic::Ordering::Relaxed};

pub struct SculkCatalystBlockEntity {
    pub position: BlockPos,
    spreader: Mutex<SculkSpreader>,
}

impl BlockEntity for SculkCatalystBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }
    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        Self {
            position,
            spreader: Mutex::new(SculkSpreader::from_nbt(nbt)),
        }
    }
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.spreader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write_nbt(nbt);
    }
    fn tick(&self, world: &Arc<World>) {
        if world.get_block(&self.position).id != BlockId::SCULK_CATALYST {
            return;
        }
        {
            let mut spreader = self
                .spreader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if spreader.is_empty() {
                return;
            }
            spreader.tick(world, self.position);
        }
        self.mark_dirty(world);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SculkCatalystBlockEntity {
    pub const ID: &'static str = "minecraft:sculk_catalyst";
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            spreader: Mutex::new(SculkSpreader::default()),
        }
    }
    fn mark_dirty(&self, world: &World) {
        world
            .level
            .read_chunk_sync(&self.position.chunk_position(), |chunk| {
                chunk.mark_dirty(true)
            });
    }
    fn bloom(&self, world: &Arc<World>) {
        let state = world.get_block_state(&self.position);
        if state.id.to_block().id != BlockId::SCULK_CATALYST {
            return;
        }
        let mut props = SculkCatalystLikeProperties::from_state_id(state.id);
        props.bloom = true;
        world.set_block_state(
            &self.position,
            props.to_state_id(&Block::SCULK_CATALYST),
            BlockFlags::NOTIFY_ALL,
        );
        world.schedule_block_tick(
            &Block::SCULK_CATALYST,
            self.position,
            8,
            TickPriority::Normal,
        );
        world.spawn_particle(
            self.position.to_centered_f64() + Vector3::new(0.0, 0.65, 0.0),
            Vector3::new(0.2, 0.0, 0.2),
            0.0,
            2,
            Particle::SculkSoul,
        );
        let pitch = 0.6
            + world
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next_f32()
                * 0.4;
        world.play_sound_fine(
            Sound::BlockSculkCatalystBloom,
            SoundCategory::Blocks,
            &self.position.to_centered_f64(),
            2.0,
            pitch,
        );
    }
}

/// GameEventListener.DeliveryMode.BY_DISTANCE, separate from vibration filtering:
/// catalysts have no wool occlusion or travel delay. Clone entries before callbacks
/// so a bloom can safely update the same chunk's block-entity map.
pub(crate) fn dispatch_death(world: &Arc<World>, origin: Vector3<f64>, source: Option<i32>) {
    let Some(entity) = source.and_then(|id| world.get_entity_by_id(id)) else {
        return;
    };
    let Some(living) = entity.get_living_entity() else {
        return;
    };
    if living.experience_consumed.load(Relaxed) {
        return;
    }
    let min = BlockPos::floored(origin.x - 8.0, origin.y - 8.0, origin.z - 8.0).chunk_position();
    let max = BlockPos::floored(origin.x + 8.0, origin.y + 8.0, origin.z + 8.0).chunk_position();
    let mut listeners = Vec::new();
    for x in min.x..=max.x {
        for z in min.y..=max.y {
            if let Some(chunk) = world.block_entities.get(&Vector2::new(x, z)) {
                for block_entity in chunk.values() {
                    if !block_entity.as_any().is::<SculkCatalystBlockEntity>() {
                        continue;
                    }
                    let delta = block_entity.get_position().to_centered_f64() - origin;
                    let distance = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
                    if distance <= 64.0 {
                        listeners.push((distance, block_entity.clone()));
                    }
                }
            }
        }
    }
    listeners.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, block_entity) in listeners {
        let Some(catalyst) = block_entity
            .as_any()
            .downcast_ref::<SculkCatalystBlockEntity>()
        else {
            continue;
        };
        if world.get_block(&catalyst.position).id != BlockId::SCULK_CATALYST {
            continue;
        }
        if living.experience_consumed.swap(true, Relaxed) {
            break;
        }
        let now = world
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age;
        let killer = if now - living.last_damage_source_time.load(Relaxed) <= 40 {
            world.get_entity_by_id(living.last_damage_source_entity_id.load(Relaxed))
        } else {
            None
        };
        let reward = entity.get_experience_reward(killer.as_deref());
        if entity.should_drop_experience() && reward > 0 {
            let start = BlockPos::floored(origin.x, origin.y + 0.5, origin.z);
            catalyst
                .spreader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .add_cursors(start, reward);
            catalyst.mark_dirty(world);
            if let Some(attacker) = world.get_entity_by_id(living.last_hurt_by_mob_id.load(Relaxed))
                && let Some(player) = attacker.get_player()
            {
                player.trigger_advancement_criterion(
                    Advancement::ADVENTURE_KILL_MOB_NEAR_SCULK_CATALYST,
                    "kill_mob_near_sculk_catalyst",
                );
            }
        }
        catalyst.bloom(world);
        break;
    }
}
