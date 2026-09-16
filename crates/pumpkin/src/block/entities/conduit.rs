use super::BlockEntity;
use crate::entity::{EntityBase, is_enemy_type};
use crate::world::World;
use pumpkin_data::{
    Block,
    damage::DamageType,
    effect::StatusEffect,
    fluid::Fluid,
    sound::{Sound, SoundCategory},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI64, Ordering},
};
use uuid::Uuid;

pub struct ConduitBlockEntity {
    pub position: BlockPos,
    pub active: AtomicBool,
    pub target: Mutex<Option<Uuid>>,
    next_ambient: AtomicI64,
}

impl ConduitBlockEntity {
    pub const ID: &'static str = "minecraft:conduit";
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            active: AtomicBool::new(false),
            target: Mutex::new(None),
            next_ambient: AtomicI64::new(0),
        }
    }

    fn frame_size(&self, world: &World) -> usize {
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let pos = self.position.offset(Vector3::new(x, y, z));
                    if !world.get_fluid(&pos).matches_type(&Fluid::WATER) {
                        return 0;
                    }
                }
            }
        }
        let mut size = 0;
        for x in -2_i32..=2 {
            for y in -2_i32..=2 {
                for z in -2_i32..=2 {
                    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
                    if (ax > 1 || ay > 1 || az > 1)
                        && ((x == 0 && (ay == 2 || az == 2))
                            || (y == 0 && (ax == 2 || az == 2))
                            || (z == 0 && (ax == 2 || ay == 2)))
                    {
                        let block = world.get_block(&self.position.offset(Vector3::new(x, y, z)));
                        if [
                            Block::PRISMARINE.id,
                            Block::PRISMARINE_BRICKS.id,
                            Block::SEA_LANTERN.id,
                            Block::DARK_PRISMARINE.id,
                        ]
                        .contains(&block.id)
                        {
                            size += 1;
                        }
                    }
                }
            }
        }
        size
    }

    fn close_to(&self, entity: &dyn EntityBase, range: f64) -> bool {
        self.position
            .to_f64()
            .squared_distance_to_vec(&entity.get_entity().block_pos.load().to_f64())
            < range * range
    }

    fn apply_effects(&self, world: &World, frame_size: usize) {
        let range = (frame_size / 7 * 16) as f64;
        let bounds = BoundingBox::from_block(&self.position)
            .expand_all(range)
            .expand_towards(
                0.0,
                f64::from(world.get_top_y() - world.get_bottom_y() + 1),
                0.0,
            );
        for player in world.get_players_at_box(&bounds) {
            if !player.get_entity().is_removed()
                && player.living_entity.health.load() > 0.0
                && self.close_to(player.as_ref(), range)
                && player.get_entity().is_in_water_or_rain()
            {
                player.add_effect(pumpkin_data::potion::Effect {
                    effect_type: &StatusEffect::CONDUIT_POWER,
                    duration: 260,
                    amplifier: 0,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }

    fn resolve(world: &World, uuid: Uuid) -> Option<Arc<dyn EntityBase>> {
        world
            .get_entity_by_uuid(uuid)
            .or_else(|| {
                world
                    .get_player_by_uuid(uuid)
                    .map(|player| player as Arc<dyn EntityBase>)
            })
            .filter(|entity| entity.get_living_entity().is_some())
    }

    fn update_target(&self, world: &Arc<World>, hunting: bool) {
        let previous = *self
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let target = if !hunting {
            None
        } else if let Some(uuid) = previous {
            Self::resolve(world, uuid).filter(|entity| {
                !entity.get_entity().is_removed()
                    && entity
                        .get_living_entity()
                        .is_some_and(|living| living.health.load() > 0.0)
                    && self.close_to(entity.as_ref(), 8.0)
            })
        } else {
            let bounds = BoundingBox::from_block(&self.position).expand_all(8.0);
            let candidates: Vec<_> = world
                .get_entities_at_box(&bounds)
                .into_iter()
                .filter(|entity| {
                    !entity.get_entity().is_removed()
                        && entity
                            .get_living_entity()
                            .is_some_and(|living| living.health.load() > 0.0)
                        && is_enemy_type(entity.get_entity().entity_type)
                        && entity.get_entity().is_in_water_or_rain()
                })
                .collect();
            if candidates.is_empty() {
                None
            } else {
                Some(candidates[world.rand_bounded_i32(candidates.len() as i32) as usize].clone())
            }
        };
        let uuid = target
            .as_ref()
            .map(|entity| entity.get_entity().entity_uuid);
        if let Some(target) = target {
            world.play_sound(
                Sound::BlockConduitAttackTarget,
                SoundCategory::Blocks,
                &target.get_entity().pos.load(),
            );
            target.damage_with_context(target.as_ref(), 4.0, DamageType::MAGIC, None, None, None);
        }
        if uuid != previous {
            *self
                .target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = uuid;
            if let Some(entity) = world.get_block_entity(&self.position) {
                world.update_block_entity(&entity);
            }
        }
    }
}

impl BlockEntity for ConduitBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }
    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        let mut entity = Self::new(position);
        *entity
            .target
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = nbt.get_uuid("Target");
        entity
    }
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(uuid) = *self
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_uuid("Target", uuid);
        }
    }
    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.write_nbt(&mut nbt);
        Some(nbt)
    }
    fn tick(&self, world: &Arc<World>) {
        let time = world.get_world_age();
        if time % 40 == 0 {
            let size = self.frame_size(world);
            let active = size >= 16;
            if active != self.active.swap(active, Ordering::Relaxed) {
                world.play_sound(
                    if active {
                        Sound::BlockConduitActivate
                    } else {
                        Sound::BlockConduitDeactivate
                    },
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
            if active {
                self.apply_effects(world, size);
                self.update_target(world, size >= 42);
            }
        }
        if self.active.load(Ordering::Relaxed) {
            if time % 80 == 0 {
                world.play_sound(
                    Sound::BlockConduitAmbient,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
            if time > self.next_ambient.load(Ordering::Relaxed) {
                self.next_ambient.store(
                    time + 60 + i64::from(world.rand_bounded_i32(40)),
                    Ordering::Relaxed,
                );
                world.play_sound(
                    Sound::BlockConduitAmbientShort,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
