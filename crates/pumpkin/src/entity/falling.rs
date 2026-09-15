use pumpkin_data::Block;
use pumpkin_data::BlockStateId;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::{Arc, atomic::Ordering};

use crate::{
    block::blocks::falling::FallingBlock,
    entity::{Entity, EntityBase, living::LivingEntity},
    server::Server,
    world::World,
};

pub struct FallingEntity {
    entity: Entity,
    block_state_id: BlockStateId,
    cancel_drop: bool,
}

impl FallingEntity {
    pub fn new(entity: Entity, block_state_id: BlockStateId) -> Self {
        Self {
            entity,
            block_state_id,
            cancel_drop: matches!(
                block_state_id.to_block().id,
                pumpkin_data::BlockId::SUSPICIOUS_SAND | pumpkin_data::BlockId::SUSPICIOUS_GRAVEL
            ),
        }
    }

    /// Replaced the current Block and Spawns a new Falling one (synchronous)
    pub fn replace_spawn(world: &Arc<World>, position: BlockPos, block_state: BlockStateId) {
        // Falling blocks leave their original fluid behind.
        world.set_block_state(
            &position,
            World::fluid_state_from_block_state(block_state)
                .1
                .block_state_id,
            BlockFlags::NOTIFY_ALL,
        );

        let position = position.0.to_f64().add_raw(0.5, 0.0, 0.5);
        let entity = Entity::new(world.clone(), position, &EntityType::FALLING_BLOCK);
        entity
            .data
            .store(i32::from(block_state.as_u16()), Ordering::Relaxed);
        let entity = Arc::new(Self::new(entity, block_state));
        world.spawn_entity_non_save(entity);
    }
}

impl EntityBase for FallingEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.velocity.store(velo);

        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
            let world = entity.world.load();
            let landing_pos = self.entity.block_pos.load();
            if world.get_block(&landing_pos) == &Block::MOVING_PISTON {
                return;
            }
            if self.cancel_drop {
                self.entity.remove();
                let bounds = entity.bounding_box.load();
                let center = (bounds.min + bounds.max) * 0.5;
                world.sync_world_event(
                    pumpkin_data::world::WorldEvent::ParticlesDestroyBlock,
                    BlockPos::floored(center.x, center.y, center.z),
                    i32::from(self.block_state_id.as_u16()),
                );
                world.emit_game_event_from_entity("block_destroy", center, Some(self), None);
                return;
            }
            let mut state_id = self.block_state_id;
            let block = Block::from_state_id(state_id);
            if block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS)
                && FallingBlock::should_solidify(&**world, &landing_pos)
                && let Some(name) = block.name.strip_suffix("_powder")
                && let Some(concrete) = Block::from_name(name)
            {
                state_id = concrete.default_state.id;
            }
            world.set_block_state(&landing_pos, state_id, BlockFlags::NOTIFY_ALL);
            self.entity.remove();
        }

        entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
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
    fn damage(&self, _caller: &dyn EntityBase, _amount: f32, _damage_type: DamageType) -> bool {
        false
    }

    fn get_default_gravity(&self) -> f64 {
        0.04
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
