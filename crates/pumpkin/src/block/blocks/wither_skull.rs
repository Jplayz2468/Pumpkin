use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId, entity::EntityType, world::WorldEvent,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, OnNeighborUpdateArgs, OnPlaceArgs, PathComputationType, PlacedArgs,
        blocks::skull_block::SkullBlock,
    },
    entity::{Entity, boss::wither::WitherEntity},
    world::World,
};

pub struct WitherPattern {
    blocks: [BlockPos; 9],
    base: BlockPos,
    yaw: f32,
}

#[must_use]
pub fn find_wither_pattern(world: &Arc<World>, skull_pos: &BlockPos) -> Option<WitherPattern> {
    use pumpkin_data::tag::{self, Taggable};
    if world.level_info.load().difficulty == pumpkin_util::Difficulty::Peaceful
        || skull_pos.0.y < world.get_bottom_y()
    {
        return None;
    }
    let is_soul_block =
        |block: &Block| block.has_tag(&tag::Block::MINECRAFT_WITHER_SUMMON_BASE_BLOCKS);
    // `WitherSkullBlock.getOrCreateWitherFull` matches each skull position against
    // `WITHER_SKELETON_SKULL` *or* `WITHER_SKELETON_WALL_SKULL`
    // (WitherSkullBlock.java:101), so a wall-mounted wither skull completes the pattern
    // exactly like the standing one.
    let is_skull = |pos: &BlockPos| {
        pos == skull_pos
            || world.get_block(pos) == &Block::WITHER_SKELETON_SKULL
            || world.get_block(pos) == &Block::WITHER_SKELETON_WALL_SKULL
    };

    for dir in [BlockDirection::North, BlockDirection::West] {
        let opposite = dir.opposite();
        for center in [
            *skull_pos,
            skull_pos.offset(opposite.to_offset()),
            skull_pos.offset(dir.to_offset()),
        ] {
            let top_middle = center.down();
            let base = top_middle.down();
            let arm1 = top_middle.offset(dir.to_offset());
            let arm2 = top_middle.offset(opposite.to_offset());
            let skull1_pos = arm1.up();
            let skull2_pos = arm2.up();

            if is_soul_block(world.get_block(&top_middle))
                && is_soul_block(world.get_block(&base))
                && is_soul_block(world.get_block(&arm1))
                && is_soul_block(world.get_block(&arm2))
                && is_skull(&center)
                && is_skull(&skull1_pos)
                && is_skull(&skull2_pos)
                && world.get_block_state(&arm1.down()).is_air()
                && world.get_block_state(&arm2.down()).is_air()
            {
                return Some(WitherPattern {
                    blocks: [
                        skull1_pos,
                        arm1,
                        arm1.down(),
                        center,
                        top_middle,
                        base,
                        skull2_pos,
                        arm2,
                        arm2.down(),
                    ],
                    base,
                    yaw: if dir == BlockDirection::North {
                        0.0
                    } else {
                        90.0
                    },
                });
            }
        }
    }

    None
}

fn spawn_wither(world: &Arc<World>, pattern: &WitherPattern) {
    for pos in pattern.blocks {
        let state_id = world.get_block_state_id(&pos);
        world.set_block_state(
            &pos,
            Block::AIR.default_state.id,
            BlockFlags::NOTIFY_LISTENERS,
        );
        world.sync_world_event(
            WorldEvent::ParticlesDestroyBlock,
            pos,
            state_id.as_u16().into(),
        );
    }

    let entity = Entity::new(
        world.clone(),
        pattern.base.to_f64().add_raw(0.5, 0.55, 0.5),
        &EntityType::WITHER,
    );
    entity.set_rotation(pattern.yaw, 0.0);
    let wither = WitherEntity::new(entity);
    wither.make_invulnerable();
    world.spawn_entity(wither);
    for pos in &pattern.blocks {
        world.update_neighbors_at(pos, &Block::AIR, None);
    }
}

// Blocks.java:2801-2805 registers `WITHER_SKELETON_WALL_SKULL` as the wall counterpart of
// this block via `wallVariant(WITHER_SKELETON_SKULL, ...)`; `on_place` below (delegating to
// `SkullBlock::on_place`) picks whichever one applies based on where the player is looking.
#[pumpkin_block("wither_skeleton_skull", "wither_skeleton_wall_skull")]
pub struct WitherSkeletonSkullBlock;

impl BlockBehaviour for WitherSkeletonSkullBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        SkullBlock::on_place(&SkullBlock, args)
    }

    // Neither `WitherSkullBlock` nor `WitherWallSkullBlock` override `neighborChanged` in
    // vanilla, so both inherit `AbstractSkullBlock.neighborChanged`
    // (AbstractSkullBlock.java:73-83) unmodified -- the same powered/redstone sync that
    // plain skulls get. Delegate to the identical logic in `SkullBlock`.
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        SkullBlock::on_neighbor_update(&SkullBlock, args);
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if !args
            .world
            .get_block_entity(args.position)
            .is_some_and(|entity| {
                entity
                    .as_any()
                    .is::<crate::block::entities::skull::SkullBlockEntity>()
            })
        {
            return;
        }

        if let Some(pattern) = find_wither_pattern(args.world, args.position) {
            spawn_wither(args.world, &pattern);
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
