use crate::block::entities::BlockEntity;
use crate::entity::EntityBase;
use crate::entity::ai::brain::{memory::MemoryValue, registry::MemoryModuleType};
use crate::world::World;
use crossbeam::atomic::AtomicCell;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockDirection, HorizontalFacingExt};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::any::Any;
use std::sync::{Arc, Mutex, Weak};

pub struct BellBlockEntity {
    pub position: BlockPos,
    pub last_side_hit: AtomicCell<Option<HorizontalFacing>>,
    pub ring_ticks: AtomicCell<i32>,
    pub ringing: AtomicCell<bool>,
    resonating: AtomicCell<bool>,
    resonate_time: AtomicCell<i32>,
    world: Mutex<Weak<World>>,
    last_ring_timestamp: AtomicCell<i64>,
    nearby_entities: Mutex<Option<Vec<Weak<dyn EntityBase>>>>,
}

impl BellBlockEntity {
    pub const ID: &'static str = "minecraft:bell";

    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            last_side_hit: AtomicCell::new(None),
            ring_ticks: AtomicCell::new(0),
            resonate_time: AtomicCell::new(0),
            resonating: AtomicCell::new(false),
            ringing: AtomicCell::new(false),
            world: Mutex::new(Weak::new()),
            last_ring_timestamp: AtomicCell::new(0),
            nearby_entities: Mutex::new(None),
        }
    }

    pub fn activate(&self, direction: HorizontalFacing) {
        self.last_side_hit.store(Some(direction));
        if self.ringing.load() {
            self.ring_ticks.store(0);
        } else {
            self.ringing.store(true);
        }
        let world = self.world.lock().unwrap().upgrade();
        if let Some(world) = world {
            world.add_synced_block_event(
                self.position,
                1,
                direction.to_block_direction().to_index(),
            );
        }
    }

    pub fn trigger_ring(&self, world: &Arc<World>, direction: u8) {
        self.update_entities(world);
        self.resonate_time.store(0);
        self.last_side_hit.store(
            BlockDirection::try_from(i32::from(direction))
                .ok()
                .and_then(|direction| direction.to_horizontal_facing()),
        );
        self.ring_ticks.store(0);
        self.ringing.store(true);
    }

    fn cached_entities(&self) -> Vec<Arc<dyn EntityBase>> {
        self.nearby_entities
            .lock()
            .unwrap()
            .as_ref()
            .map_or_else(Vec::new, |entities| {
                entities.iter().filter_map(Weak::upgrade).collect()
            })
    }

    fn within(&self, entity: &dyn EntityBase, radius: f64) -> bool {
        !entity.get_entity().is_removed()
            && entity
                .get_living_entity()
                .is_some_and(|living| living.health.load() > 0.0)
            && entity
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&self.position.to_centered_f64())
                < radius * radius
    }

    fn update_entities(&self, world: &Arc<World>) {
        let time = world.get_world_age();
        let needs_search = self.nearby_entities.lock().unwrap().is_none()
            || time > self.last_ring_timestamp.load() + 60;
        if needs_search {
            self.last_ring_timestamp.store(time);
            let corner = self.position.to_f64();
            let bounds = BoundingBox::new(
                corner.sub(&Vector3::new(48.0, 48.0, 48.0)),
                corner.add(&Vector3::new(49.0, 49.0, 49.0)),
            );
            let entities = world
                .get_all_at_box(&bounds)
                .into_iter()
                .filter(|entity| entity.get_living_entity().is_some() && !entity.is_spectator())
                .map(|entity| Arc::downgrade(&entity))
                .collect();
            *self.nearby_entities.lock().unwrap() = Some(entities);
        }
        for entity in self.cached_entities() {
            if self.within(entity.as_ref(), 32.0)
                && let Some(mob) = entity.get_mob()
                && let Some(brain) = mob.get_mob_entity().brain.lock().unwrap().as_mut()
                && brain
                    .memories()
                    .is_registered(MemoryModuleType::HeardBellTime)
            {
                brain
                    .memories_mut()
                    .set(MemoryModuleType::HeardBellTime, MemoryValue::Long(time));
            }
        }
    }

    pub fn raiders_hear_bell(&self) -> bool {
        self.cached_entities().iter().any(|entity| {
            self.within(entity.as_ref(), 32.0)
                && entity
                    .get_entity()
                    .entity_type
                    .is_tagged_with("#minecraft:raiders")
                    .unwrap_or(false)
        })
    }

    fn make_raiders_glow(&self) {
        for entity in self.cached_entities() {
            if self.within(entity.as_ref(), 48.0)
                && entity
                    .get_entity()
                    .entity_type
                    .is_tagged_with("#minecraft:raiders")
                    .unwrap_or(false)
                && let Some(living) = entity.get_living_entity()
            {
                living.add_effect(Effect {
                    effect_type: &StatusEffect::GLOWING,
                    duration: 60,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }
}

impl BlockEntity for BellBlockEntity {
    fn write_nbt(&self, _nbt: &mut NbtCompound) {}
    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self {
        Self::new(position)
    }
    fn set_world(&self, world: Weak<World>) {
        *self.world.lock().unwrap() = world;
    }

    fn tick(&self, world: &Arc<World>) {
        if self.ringing.load() {
            self.ring_ticks.fetch_add(1);
        }
        if self.ring_ticks.load() >= 50 {
            self.ringing.store(false);
            self.ring_ticks.store(0);
        }
        if self.ring_ticks.load() >= 5 && self.resonate_time.load() == 0 && self.raiders_hear_bell()
        {
            self.resonating.store(true);
            world.play_sound_fine(
                Sound::BlockBellResonate,
                SoundCategory::Blocks,
                &self.position.to_centered_f64(),
                1.0,
                1.0,
            );
        }
        if self.resonating.load() {
            if self.resonate_time.load() < 40 {
                self.resonate_time.fetch_add(1);
            } else {
                self.make_raiders_glow();
                self.resonating.store(false);
            }
        }
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
