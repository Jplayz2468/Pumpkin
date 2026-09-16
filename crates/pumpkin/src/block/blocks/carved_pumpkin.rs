use std::sync::Arc;

use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId, block_properties::WallTorchLikeProperties,
    entity::EntityType, world::WorldEvent,
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::{
    block::{BlockBehaviour, BlockMetadata, OnPlaceArgs, PlacedArgs},
    entity::{
        Entity, EntityBase,
        passive::{iron_golem::IronGolemEntity, snow_golem::SnowGolemEntity},
    },
    world::World,
};

pub struct GolemPattern {
    entity_type: &'static EntityType,
    blocks: Vec<BlockPos>,
    base: BlockPos,
}

// Upright patterns; full rotated BlockPattern matching is tracked in D03.
#[must_use]
pub fn find_golem_pattern(world: &Arc<World>, pos: &BlockPos) -> Option<GolemPattern> {
    let down_pos = pos.down();
    let upper = world.get_block(&down_pos);
    let lower = world.get_block(&down_pos.down());

    if upper == &Block::SNOW_BLOCK && lower == &Block::SNOW_BLOCK {
        return Some(GolemPattern {
            entity_type: &EntityType::SNOW_GOLEM,
            blocks: vec![*pos, down_pos, down_pos.down()],
            base: down_pos.down(),
        });
    }

    if upper != &Block::IRON_BLOCK || lower != &Block::IRON_BLOCK {
        return None;
    }

    for dir in [BlockDirection::North, BlockDirection::West] {
        let arm1 = down_pos.offset(dir.to_offset());
        let arm2 = down_pos.offset(dir.opposite().to_offset());

        if world.get_block(&arm1) == &Block::IRON_BLOCK
            && world.get_block(&arm2) == &Block::IRON_BLOCK
            && [arm1.up(), arm2.up(), arm1.down(), arm2.down()]
                .iter()
                .all(|pos| world.get_block_state(pos).is_air())
        {
            return Some(GolemPattern {
                entity_type: &EntityType::IRON_GOLEM,
                blocks: vec![
                    arm1.up(),
                    arm1,
                    arm1.down(),
                    *pos,
                    down_pos,
                    down_pos.down(),
                    arm2.up(),
                    arm2,
                    arm2.down(),
                ],
                base: down_pos.down(),
            });
        }
    }

    None
}

fn spawn_golem(world: &Arc<World>, pattern: GolemPattern) {
    for pos in &pattern.blocks {
        let state_id = world.get_block_state_id(pos);
        world.set_block_state(
            pos,
            Block::AIR.default_state.id,
            BlockFlags::NOTIFY_LISTENERS,
        );
        world.sync_world_event(
            WorldEvent::ParticlesDestroyBlock,
            *pos,
            state_id.as_u16().into(),
        );
    }

    let entity = Entity::new(
        world.clone(),
        pattern.base.to_f64().add_raw(0.5, 0.05, 0.5),
        pattern.entity_type,
    );
    let golem: Arc<dyn EntityBase> = if pattern.entity_type == &EntityType::SNOW_GOLEM {
        SnowGolemEntity::new(entity)
    } else {
        let golem = IronGolemEntity::new(entity);
        golem.set_player_created(true);
        golem
    };
    world.spawn_entity(golem);
    for pos in &pattern.blocks {
        world.update_neighbors_at(pos, &Block::AIR, None);
    }
}

pub struct CarvedPumpkinBlock;

impl BlockMetadata for CarvedPumpkinBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::JACK_O_LANTERN, BlockId::CARVED_PUMPKIN].into()
    }
}

impl BlockBehaviour for CarvedPumpkinBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = WallTorchLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if let Some(pattern) = find_golem_pattern(args.world, args.position) {
            spawn_golem(args.world, pattern);
        }
    }
}
