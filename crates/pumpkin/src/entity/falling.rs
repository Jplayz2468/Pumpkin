use crate::{
    block::{
        CanPlaceAtArgs,
        blocks::{anvil::AnvilBlock, falling::FallingBlock},
    },
    entity::{Entity, EntityBase, item::ItemEntity, living::LivingEntity},
    server::Server,
    world::World,
};
use crossbeam::atomic::AtomicCell;
use pumpkin_data::{
    Block, BlockId, BlockStateId,
    damage::DamageType,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    tag::{self, Taggable},
    world::WorldEvent,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::{
    math::position::BlockPos,
    random::{RandomImpl, legacy_rand::LegacyRand},
};
use pumpkin_world::world::BlockFlags;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

pub struct FallingEntity {
    entity: Entity,
    block_state_id: AtomicCell<BlockStateId>,
    time: AtomicI32,
    drop_item: AtomicBool,
    cancel_drop: AtomicBool,
    hurt_entities: AtomicBool,
    fall_damage_amount: AtomicCell<f32>,
    fall_damage_max: AtomicI32,
    fall_distance: Arc<AtomicCell<f64>>,
    block_data: Mutex<Option<NbtCompound>>,
    random: Mutex<LegacyRand>,
}

impl FallingEntity {
    pub fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        let fall_distance = Arc::clone(&entity.fall_distance);
        Self {
            entity,
            block_state_id: AtomicCell::new(block_state_id),
            time: AtomicI32::new(0),
            drop_item: AtomicBool::new(true),
            cancel_drop: AtomicBool::new(false),
            hurt_entities: AtomicBool::new(false),
            fall_damage_amount: AtomicCell::new(0.0),
            fall_damage_max: AtomicI32::new(40),
            fall_distance,
            block_data: Mutex::new(None),
            random: Mutex::new(LegacyRand::from_seed(rand::random())),
        }
    }

    pub fn set_hurts_entities(&self, amount: f32, max: i32) {
        self.hurt_entities.store(true, Ordering::Relaxed);
        self.fall_damage_amount.store(amount);
        self.fall_damage_max.store(max, Ordering::Relaxed);
    }

    pub fn check_fall_distance_accumulation(&self) {
        if self.entity.velocity.load().y > -0.5 {
            self.fall_distance.store(self.fall_distance.load().min(1.0));
        }
    }

    pub fn reset_fall_distance(&self) {
        self.fall_distance.store(0.0);
    }

    fn with_waterlogged(state: BlockStateId, waterlogged: bool) -> BlockStateId {
        let block = state.to_block();
        let Some(props) = block.properties(state) else {
            return state;
        };
        let mut properties = props.to_props();
        for (name, value) in &mut properties {
            if *name == "waterlogged" {
                *value = if waterlogged { "true" } else { "false" };
            }
        }
        block
            .state_from_properties(&properties)
            .map_or(state, |state| state.id)
    }

    pub fn replace_spawn(
        world: &Arc<World>,
        position: BlockPos,
        block_state: BlockStateId,
    ) -> Arc<Self> {
        let falling_state = Self::with_waterlogged(block_state, false);
        let entity = Entity::new(
            world.clone(),
            position.0.to_f64().add_raw(0.5, 0.0, 0.5),
            &EntityType::FALLING_BLOCK,
        );
        entity
            .data
            .store(i32::from(falling_state.as_u16()), Ordering::Relaxed);
        let falling = Arc::new(Self::new(entity, falling_state));
        let block = block_state.to_block();
        if block.has_tag(&tag::Block::MINECRAFT_ANVIL) {
            falling.hurt_entities.store(true, Ordering::Relaxed);
            falling.fall_damage_amount.store(2.0);
        }
        if matches!(
            block.id,
            BlockId::SUSPICIOUS_SAND | BlockId::SUSPICIOUS_GRAVEL
        ) {
            falling.cancel_drop.store(true, Ordering::Relaxed);
        }
        world.set_block_state(
            &position,
            World::fluid_state_from_block_state(block_state)
                .1
                .block_state_id,
            BlockFlags::NOTIFY_ALL,
        );
        world.spawn_entity(falling.clone());
        falling
    }

    fn drop_block_item(&self, world: &Arc<World>) {
        if !self.drop_item.load(Ordering::Relaxed)
            || !world.level_info.load().game_rules.entity_drops
        {
            return;
        }
        let Some(item) = Item::from_id(self.block_state_id.load().to_block().item_id) else {
            return;
        };
        let stack = ItemStack::new(1, item);
        if stack.is_empty() {
            return;
        }
        world.spawn_entity(Arc::new(ItemEntity::new(
            Entity::new(world.clone(), self.entity.pos.load(), &EntityType::ITEM),
            stack,
        )));
    }

    fn broken_after_fall(&self, world: &Arc<World>, pos: &BlockPos) {
        let state = self.block_state_id.load();
        if state.to_block().has_tag(&tag::Block::MINECRAFT_ANVIL) && !self.entity.is_silent() {
            world.sync_world_event(WorldEvent::SoundAnvilBroken, *pos, 0);
        } else if !self.entity.is_silent()
            && matches!(
                state.to_block().id,
                BlockId::POINTED_DRIPSTONE | BlockId::SULFUR_SPIKE
            )
        {
            world.sync_world_event(
                if state.to_block() == &Block::SULFUR_SPIKE {
                    WorldEvent::SoundSulfurSpikeLand
                } else {
                    WorldEvent::SoundPointedDripstoneLand
                },
                *pos,
                0,
            );
        } else if matches!(
            state.to_block().id,
            BlockId::SUSPICIOUS_SAND | BlockId::SUSPICIOUS_GRAVEL
        ) {
            let bounds = self.entity.bounding_box.load();
            let center = (bounds.min + bounds.max) * 0.5;
            world.sync_world_event(
                WorldEvent::ParticlesDestroyBlock,
                BlockPos::floored(center.x, center.y, center.z),
                i32::from(state.as_u16()),
            );
            world.emit_game_event_from_entity("block_destroy", center, Some(self), None);
        }
    }

    pub(crate) fn hurt_on_landing(&self, world: &Arc<World>, distance: f64) {
        if !self.hurt_entities.load(Ordering::Relaxed) {
            return;
        }
        let distance = (distance - 1.0).ceil() as i32;
        if distance < 0 {
            return;
        }
        let state = self.block_state_id.load();
        let is_anvil = state.to_block().has_tag(&tag::Block::MINECRAFT_ANVIL);
        let damage = ((distance as f32 * self.fall_damage_amount.load()).floor())
            .min(self.fall_damage_max.load(Ordering::Relaxed) as f32);
        let damage_type = if is_anvil {
            DamageType::FALLING_ANVIL
        } else if matches!(
            state.to_block().id,
            BlockId::POINTED_DRIPSTONE | BlockId::SULFUR_SPIKE
        ) {
            DamageType::FALLING_STALACTITE
        } else {
            DamageType::FALLING_BLOCK
        };
        for target in world.get_all_at_box(&self.entity.bounding_box.load()) {
            if target.get_entity().entity_uuid == self.entity.entity_uuid
                || target.get_living_entity().is_none()
                || !target.get_entity().is_alive()
                || target.get_player().is_some_and(|player| {
                    matches!(
                        player.gamemode.load(),
                        pumpkin_util::GameMode::Creative | pumpkin_util::GameMode::Spectator
                    )
                })
            {
                continue;
            }
            target.damage_with_context(
                target.as_ref(),
                damage,
                damage_type,
                Some(self.entity.pos.load()),
                Some(self),
                Some(self),
            );
        }
        if is_anvil
            && damage > 0.0
            && self
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next_f32()
                < 0.05 + distance as f32 * 0.05
        {
            if let Some(next) = AnvilBlock::damage(state) {
                self.block_state_id.store(next);
                self.entity
                    .data
                    .store(i32::from(next.as_u16()), Ordering::Relaxed);
            } else {
                self.cancel_drop.store(true, Ordering::Relaxed);
            }
        }
    }

    fn land(&self, world: &Arc<World>, pos: BlockPos, in_water: bool) {
        let state = self.block_state_id.load();
        let block = state.to_block();
        let current = world.get_block_state(&pos);
        if current.id.to_block() == &Block::MOVING_PISTON {
            return;
        }
        if self.cancel_drop.load(Ordering::Relaxed) {
            self.entity.remove();
            self.broken_after_fall(world, &pos);
            return;
        }
        let may_replace = current.replaceable()
            && (current.id.to_block() != &Block::SNOW
                || current
                    .id
                    .to_block()
                    .properties(current.id)
                    .is_some_and(|props| {
                        props
                            .to_props()
                            .iter()
                            .any(|(name, value)| *name == "layers" && *value == "1")
                    }));
        let below = world.get_block_state(&pos.down());
        let concrete = block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS);
        let would_continue_falling =
            FallingBlock::can_fall_through(below, below.id.to_block()) && !(concrete && in_water);
        let would_survive =
            world
                .block_registry
                .get_pumpkin_block(block.id)
                .is_none_or(|behaviour| {
                    behaviour.can_place_at(CanPlaceAtArgs {
                        server: None,
                        world: Some(world),
                        block_accessor: world.as_ref(),
                        block,
                        state: state.to_state(),
                        position: &pos,
                        direction: None,
                        player: None,
                        use_item_on: None,
                    })
                })
                && !would_continue_falling;
        if may_replace && would_survive && world.is_in_build_limit(pos) {
            let (fluid, fluid_state) = World::fluid_state_from_block_state(current.id);
            let mut placed = if fluid.has_tag(&tag::Fluid::MINECRAFT_WATER) && fluid_state.is_source
            {
                Self::with_waterlogged(state, true)
            } else {
                state
            };
            let solidifies = concrete
                && (FallingBlock::can_solidify(current)
                    || FallingBlock::touches_liquid(world.as_ref(), &pos));
            world.set_block_state(&pos, placed, BlockFlags::NOTIFY_ALL);
            self.entity.remove();
            if solidifies
                && let Some(name) = block.name.strip_suffix("_powder")
                && let Some(concrete) = Block::from_name(name)
            {
                placed = concrete.default_state.id;
                world.set_block_state(&pos, placed, BlockFlags::NOTIFY_ALL);
            }
            if block.has_tag(&tag::Block::MINECRAFT_ANVIL) && !self.entity.is_silent() {
                world.sync_world_event(WorldEvent::SoundAnvilLand, pos, 0);
            }
            let data = self
                .block_data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(data) = data
                && let Some(entity) = world.get_block_entity(&pos)
            {
                let mut merged = NbtCompound::new();
                entity.write_nbt(&mut merged);
                merged.extend(data);
                merged.put_string("id", entity.resource_location().to_string());
                merged.put_int("x", pos.0.x);
                merged.put_int("y", pos.0.y);
                merged.put_int("z", pos.0.z);
                if let Some(reloaded) = crate::block::entities::block_entity_from_nbt(&merged) {
                    world.add_block_entity(reloaded.clone());
                    world.update_block_entity(&reloaded);
                }
            }
        } else {
            self.entity.remove();
            if self.drop_item.load(Ordering::Relaxed)
                && world.level_info.load().game_rules.entity_drops
            {
                self.broken_after_fall(world, &pos);
                self.drop_block_item(world);
            }
        }
    }
}

impl EntityBase for FallingEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        if self.block_state_id.load().to_state().is_air() {
            self.entity.remove();
            return;
        }
        let time = self.time.fetch_add(1, Ordering::Relaxed) + 1;
        let before = self.entity.pos.load();
        let mut velocity = self.entity.velocity.load();
        velocity.y -= self.get_gravity();
        self.entity.velocity.store(velocity);
        self.entity.move_entity(caller, velocity);
        self.entity.tick_block_collisions(caller);
        let world = self.entity.world.load_full();
        if self.entity.is_alive() {
            let mut pos = self.entity.block_pos.load();
            let concrete = self
                .block_state_id
                .load()
                .to_block()
                .has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS);
            let mut in_water =
                concrete && world.get_fluid(&pos).has_tag(&tag::Fluid::MINECRAFT_WATER);
            if concrete
                && self.entity.velocity.load().length_squared() > 1.0
                && let Some(hit) = world.raycast_falling_block(before, self.entity.pos.load())
                && world.get_fluid(&hit).has_tag(&tag::Fluid::MINECRAFT_WATER)
            {
                pos = hit;
                in_water = true;
            }
            if self.entity.on_ground.load(Ordering::Relaxed) || in_water {
                self.entity
                    .velocity
                    .store(self.entity.velocity.load().multiply(0.7, -0.5, 0.7));
                self.land(&world, pos, in_water);
            } else if time > 600
                || (time > 100 && (pos.0.y <= world.get_bottom_y() || pos.0.y > world.get_top_y()))
            {
                self.drop_block_item(&world);
                self.entity.remove();
            }
        }
        self.entity
            .velocity
            .store(self.entity.velocity.load() * 0.98);
        if self.entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            self.entity.send_pos_rot();
            self.entity.send_velocity();
        }
    }

    fn init_data_tracker(&self) {
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::falling_block::START_POS,
            self.entity.block_pos.load(),
        );
    }
    fn get_entity(&self) -> &Entity {
        &self.entity
    }
    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }
    fn can_hit(&self) -> bool {
        self.entity.is_alive()
    }
    fn damage(&self, _caller: &dyn EntityBase, _amount: f32, _damage_type: DamageType) -> bool {
        false
    }
    fn get_default_gravity(&self) -> f64 {
        0.04
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let state = self.block_state_id.load();
        let block = state.to_block();
        let mut state_nbt = NbtCompound::new();
        state_nbt.put_string("Name", format!("minecraft:{}", block.name));
        if let Some(properties) = block.properties(state) {
            let mut values = NbtCompound::new();
            for (name, value) in properties.to_props() {
                values.put_string(name, value.to_string());
            }
            state_nbt.put_compound("Properties", values);
        }
        nbt.put_compound("BlockState", state_nbt);
        nbt.put_int("Time", self.time.load(Ordering::Relaxed));
        nbt.put_bool("DropItem", self.drop_item.load(Ordering::Relaxed));
        nbt.put_bool("CancelDrop", self.cancel_drop.load(Ordering::Relaxed));
        nbt.put_bool("HurtEntities", self.hurt_entities.load(Ordering::Relaxed));
        nbt.put_float("FallHurtAmount", self.fall_damage_amount.load());
        nbt.put_int("FallHurtMax", self.fall_damage_max.load(Ordering::Relaxed));
        if let Some(data) = self
            .block_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            nbt.put_compound("TileEntityData", data.clone());
        }
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        let state_nbt = nbt.get_compound("BlockState");
        let block = state_nbt
            .and_then(|state| state.get_string("Name"))
            .and_then(Block::from_name)
            .unwrap_or(&Block::SAND);
        let state = if let Some(props) = block.properties(block.default_state.id) {
            let mut properties = props.to_props();
            if let Some(values) = state_nbt.and_then(|state| state.get_compound("Properties")) {
                for (name, value) in &mut properties {
                    if let Some(saved) = values.get_string(name) {
                        *value = saved;
                    }
                }
            }
            block
                .state_from_properties(&properties)
                .map_or(block.default_state.id, |state| state.id)
        } else {
            block.default_state.id
        };
        self.block_state_id.store(state);
        self.entity
            .data
            .store(i32::from(state.as_u16()), Ordering::Relaxed);
        self.time
            .store(nbt.get_int("Time").unwrap_or(0), Ordering::Relaxed);
        self.drop_item
            .store(nbt.get_bool("DropItem").unwrap_or(true), Ordering::Relaxed);
        self.cancel_drop.store(
            nbt.get_bool("CancelDrop").unwrap_or(false),
            Ordering::Relaxed,
        );
        self.hurt_entities.store(
            nbt.get_bool("HurtEntities")
                .unwrap_or(block.has_tag(&tag::Block::MINECRAFT_ANVIL)),
            Ordering::Relaxed,
        );
        self.fall_damage_amount
            .store(nbt.get_float("FallHurtAmount").unwrap_or(0.0));
        self.fall_damage_max
            .store(nbt.get_int("FallHurtMax").unwrap_or(40), Ordering::Relaxed);
        *self
            .block_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            nbt.get_compound("TileEntityData").cloned();
    }
}
