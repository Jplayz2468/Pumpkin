use super::BlockEntity;
use crate::{
    entity::{Entity, EntityBase, item::ItemEntity, player::Player},
    world::World,
};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::{Block, BlockDirection, block_properties::SuspiciousSandLikeProperties};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, Ordering},
};

pub struct BrushableBlockBlockEntity {
    pub position: BlockPos,
    pub item: Mutex<Option<ItemStack>>,
    pub hits: Mutex<i32>,
    resets_at: AtomicI64,
    cooldown_ends_at: AtomicI64,
    pub hit_direction: Mutex<Option<u8>>,
    pub loot_table: Mutex<Option<String>>,
    pub loot_table_seed: Mutex<i64>,
}

impl BlockEntity for BrushableBlockBlockEntity {
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
        let item = nbt
            .get_compound("item")
            .and_then(ItemStack::read_item_stack);
        let hits = 0;
        let hit_direction = nbt
            .get_int("hit_direction")
            .or_else(|| nbt.get_byte("hit_direction").map(i32::from))
            .or_else(|| nbt.get_byte("direction").map(i32::from))
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| BlockDirection::from_index(*value).is_some());
        let loot_table = nbt.get_string("LootTable").map(ToString::to_string);
        let loot_table_seed = nbt.get_long("LootTableSeed").unwrap_or(0);
        Self {
            position,
            item: Mutex::new(if loot_table.is_some() { None } else { item }),
            hits: Mutex::new(hits),
            resets_at: AtomicI64::new(0),
            cooldown_ends_at: AtomicI64::new(0),
            hit_direction: Mutex::new(hit_direction),
            loot_table: Mutex::new(loot_table),
            loot_table_seed: Mutex::new(loot_table_seed),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(loot_table) = self.loot_table.lock()
            && let Some(table) = loot_table.as_ref()
        {
            nbt.put_string("LootTable", table.clone());
            if let Ok(seed) = self.loot_table_seed.lock()
                && *seed != 0
            {
                nbt.put_long("LootTableSeed", *seed);
            }
        } else if let Ok(item) = self.item.lock()
            && let Some(it) = item.as_ref()
        {
            let mut it_nbt = NbtCompound::new();
            it.write_item_stack(&mut it_nbt);
            nbt.put_compound("item", it_nbt);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(item) = self.item.try_lock()
            && let Some(ref it) = *item
        {
            let mut it_nbt = NbtCompound::new();
            it.write_item_stack(&mut it_nbt);
            nbt.put_compound("item", it_nbt);
        }
        if let Ok(direction) = self.hit_direction.try_lock()
            && let Some(dir) = *direction
        {
            nbt.put_int("hit_direction", i32::from(dir));
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BrushableBlockBlockEntity {
    pub const ID: &'static str = "minecraft:brushable_block";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            item: Mutex::new(None),
            hits: Mutex::new(0),
            resets_at: AtomicI64::new(0),
            cooldown_ends_at: AtomicI64::new(0),
            hit_direction: Mutex::new(None),
            loot_table: Mutex::new(None),
            loot_table_seed: Mutex::new(0),
        }
    }
}

impl BrushableBlockBlockEntity {
    fn completion_state(hits: i32) -> u8 {
        match hits {
            0 => 0,
            1..=2 => 1,
            3..=5 => 2,
            _ => 3,
        }
    }

    fn unpack_loot(&self, world: &Arc<World>, player: &Player, brush: &ItemStack) {
        let table_key = self
            .loot_table
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(key) = table_key else {
            return;
        };
        let seed = *self
            .loot_table_seed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let item = pumpkin_data::loot_table::get_loot_table(&key).and_then(|table| {
            let context = crate::world::loot::LootContextParameters {
                position: Some(self.position.to_centered_f64()),
                this_entity: Some(&pumpkin_data::entity::EntityType::PLAYER),
                tool: Some(brush.clone()),
                luck: player
                    .living_entity
                    .get_attribute_value(&pumpkin_data::attributes::Attributes::LUCK)
                    as f32,
                ..Default::default()
            };
            crate::world::loot::generate_loot_with_context(table, seed, &context)
                .into_iter()
                .next()
        });
        *self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = item;
        if let Some(entity) = world.get_block_entity(&self.position) {
            world.update_block_entity(&entity);
        }
    }

    pub fn brush(
        &self,
        world: &Arc<World>,
        player: &Player,
        direction: BlockDirection,
        brush: &ItemStack,
    ) -> bool {
        self.hit_direction
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert(direction as u8);
        let time = world.get_world_age();
        self.resets_at.store(time + 40, Ordering::Relaxed);
        if time < self.cooldown_ends_at.load(Ordering::Relaxed) {
            return false;
        }
        self.cooldown_ends_at.store(time + 10, Ordering::Relaxed);
        self.unpack_loot(world, player, brush);
        let (previous, hits) = {
            let mut hits = self
                .hits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = Self::completion_state(*hits);
            *hits += 1;
            (previous, *hits)
        };
        let (block, state) = world.get_block_and_state(&self.position);
        if hits >= 10 {
            let item = self
                .item
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(mut item) = item.filter(|item| !item.is_empty()) {
                let direction = self
                    .hit_direction
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .and_then(BlockDirection::from_index)
                    .unwrap_or(BlockDirection::Up);
                let pos = self
                    .position
                    .offset(direction.to_offset())
                    .to_centered_f64()
                    .add_raw(0.0, 0.125, 0.0);
                let stack = item.split_off((world.rand_bounded_i32(21) + 10) as u8);
                let entity = ItemEntity::new_with_velocity(
                    Entity::new(world.clone(), pos, &pumpkin_data::entity::EntityType::ITEM),
                    stack,
                    pumpkin_util::math::vector3::Vector3::new(0.0, 0.0, 0.0),
                    0,
                );
                world.spawn_entity(Arc::new(entity));
            }
            world.sync_world_event(
                pumpkin_data::world::WorldEvent::ParticlesAndSoundBrushBlockComplete,
                self.position,
                i32::from(state.id.as_u16()),
            );
            let replacement = if block == &Block::SUSPICIOUS_SAND {
                &Block::SAND
            } else {
                &Block::GRAVEL
            };
            world.set_block_state(
                &self.position,
                replacement.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            return true;
        }
        world.schedule_block_tick(block, self.position, 2, TickPriority::Normal);
        let completion = Self::completion_state(hits);
        if previous != completion {
            let mut props = SuspiciousSandLikeProperties::from_state_id(state.id);
            props.dusted = completion;
            world.set_block_state(
                &self.position,
                props.to_state_id(block),
                BlockFlags::NOTIFY_ALL,
            );
        }
        if let Some(entity) = world.get_block_entity(&self.position) {
            world.update_block_entity(&entity);
        }
        false
    }

    pub fn check_reset(&self, world: &Arc<World>) {
        let time = world.get_world_age();
        let (previous, hits, retracted) = {
            let mut hits = self
                .hits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = Self::completion_state(*hits);
            let retracted = *hits != 0 && time >= self.resets_at.load(Ordering::Relaxed);
            if retracted {
                *hits = (*hits - 2).max(0);
            }
            (previous, *hits, retracted)
        };
        let (block, state) = world.get_block_and_state(&self.position);
        if retracted {
            let completion = Self::completion_state(hits);
            if previous != completion {
                let mut props = SuspiciousSandLikeProperties::from_state_id(state.id);
                props.dusted = completion;
                world.set_block_state(
                    &self.position,
                    props.to_state_id(block),
                    BlockFlags::NOTIFY_ALL,
                );
            }
            self.resets_at.store(time + 4, Ordering::Relaxed);
        }
        if hits == 0 {
            *self
                .hit_direction
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            self.resets_at.store(0, Ordering::Relaxed);
            self.cooldown_ends_at.store(0, Ordering::Relaxed);
        } else {
            world.schedule_block_tick(block, self.position, 2, TickPriority::Normal);
        }
        if retracted && let Some(entity) = world.get_block_entity(&self.position) {
            world.update_block_entity(&entity);
        }
    }
}
