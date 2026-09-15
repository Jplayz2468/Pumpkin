use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::entities::bell::BellBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, ExplodeArgs, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, OnProjectileHitArgs, OnSyncedBlockEventArgs,
    PathComputationType,
};
use crate::entity::EntityBase;
use crate::world::World;
use pumpkin_data::block_properties::{BellAttachment, BellLikeProperties, HorizontalFacing};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, HorizontalFacingExt, tag};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

fn ring_bell(
    position: BlockPos,
    world: &Arc<World>,
    hit_direction: Option<HorizontalFacing>,
    entity: Option<Arc<dyn EntityBase>>,
) -> bool {
    let Some(block_entity) = world.get_block_entity(&position) else {
        return false;
    };
    let Some(bell) = block_entity.as_any().downcast_ref::<BellBlockEntity>() else {
        return false;
    };
    let mut event = crate::plugin::block::bell_ring::BellRingEvent {
        block_pos: position,
        world: world.clone(),
        direction: hit_direction.map(|d| d.to_block_direction()),
        entity,
        cancelled: false,
    };
    if let Some(server) = world.server.upgrade() {
        server.plugin_manager.fire_blocking(&server, &mut event);
    }
    if event.cancelled {
        return false;
    }
    let props = BellLikeProperties::from_state_id(world.get_block_state_id(&position));
    let direction = event
        .direction
        .and_then(|d| d.to_horizontal_facing())
        .unwrap_or(props.facing);
    bell.activate(direction);
    world.play_sound_fine(
        Sound::BlockBellUse,
        SoundCategory::Blocks,
        &position.to_centered_f64(),
        2.0,
        1.0,
    );
    world.emit_game_event_from_entity(
        "block_change",
        position.to_centered_f64(),
        event.entity.as_deref(),
        None,
    );
    true
}

fn proper_hit(face: BlockDirection, y: f64, props: &BellLikeProperties) -> bool {
    face.is_horizontal()
        && !(y > f64::from(0.8124_f32))
        && match props.attachment {
            BellAttachment::Floor => face.to_axis() == props.facing.to_block_direction().to_axis(),
            BellAttachment::SingleWall | BellAttachment::DoubleWall => {
                face.to_axis() != props.facing.to_block_direction().to_axis()
            }
            BellAttachment::Ceiling => true,
        }
}

fn connection(props: &BellLikeProperties) -> BlockDirection {
    match props.attachment {
        BellAttachment::Ceiling => BlockDirection::Up,
        BellAttachment::Floor => BlockDirection::Down,
        _ => props.facing.to_block_direction(),
    }
}

fn survives(world: &dyn BlockAccessor, pos: &BlockPos, props: &BellLikeProperties) -> bool {
    let direction = connection(props);
    let support = world.get_block_state(&pos.offset(direction.to_offset()));
    if direction == BlockDirection::Up {
        !support
            .id
            .to_block()
            .has_tag(&tag::Block::MINECRAFT_UNSTABLE_BOTTOM_CENTER)
            && support.is_center_solid(BlockDirection::Down)
    } else {
        support.is_side_solid(direction.opposite())
    }
}

#[pumpkin_block("minecraft:bell")]
pub struct BellBlock;

impl BlockBehaviour for BellBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        survives(
            args.block_accessor,
            args.position,
            &BellLikeProperties::from_state_id(args.state.id),
        )
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let props = BellLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if !proper_hit(*args.hit.face, f64::from(args.hit.cursor_pos.y), &props) {
            return BlockActionResult::Pass;
        }
        if ring_bell(
            *args.position,
            args.world,
            args.hit.face.to_horizontal_facing(),
            Some(args.player.clone()),
        ) {
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::BellRing as i32,
                1,
            );
        }
        // A proper hit consumes the interaction even when the entity cannot ring.
        BlockActionResult::Success
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let clicked = args.direction.opposite();
        let mut props = BellLikeProperties::default(args.block);
        if !clicked.is_horizontal() {
            props.attachment = if clicked == BlockDirection::Down {
                BellAttachment::Ceiling
            } else {
                BellAttachment::Floor
            };
            props.facing = args.player.get_entity().get_horizontal_facing();
        } else {
            props.facing = clicked.opposite().to_cardinal_direction();
            let supported = |direction: BlockDirection| {
                args.world
                    .get_block_state(&args.position.offset(direction.to_offset()))
                    .is_side_solid(direction.opposite())
            };
            props.attachment = if supported(clicked) && supported(clicked.opposite()) {
                BellAttachment::DoubleWall
            } else {
                BellAttachment::SingleWall
            };
            if survives(args.world, args.position, &props) {
                return props.to_state_id(args.block);
            }
            props.attachment = if supported(BlockDirection::Down) {
                BellAttachment::Floor
            } else {
                BellAttachment::Ceiling
            };
        }
        if survives(args.world, args.position, &props) {
            props.to_state_id(args.block)
        } else {
            BlockStateId::AIR
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = BellLikeProperties::from_state_id(args.state_id);
        let connected = connection(&props);
        if connected == args.direction
            && props.attachment != BellAttachment::DoubleWall
            && !survives(args.world, args.position, &props)
        {
            return BlockStateId::AIR;
        }
        if args.direction.to_axis() == props.facing.to_block_direction().to_axis() {
            let neighbor = args.neighbor_state_id.to_state();
            if props.attachment == BellAttachment::DoubleWall
                && !neighbor.is_side_solid(args.direction)
            {
                props.attachment = BellAttachment::SingleWall;
                props.facing = args.direction.opposite().to_cardinal_direction();
            } else if props.attachment == BellAttachment::SingleWall
                && connected.opposite() == args.direction
                && neighbor.is_side_solid(props.facing.to_block_direction())
            {
                props.attachment = BellAttachment::DoubleWall;
            }
        }
        props.to_state_id(args.block)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let receiving = block_receives_redstone_power(args.world, args.position);
        let mut props =
            BellLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if props.powered != receiving {
            if receiving {
                ring_bell(*args.position, args.world, None, None);
            }
            props.powered = receiving;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        let props = BellLikeProperties::from_state_id(args.state.id);
        if !proper_hit(
            args.hit_face,
            args.hit_pos.y - f64::from(args.position.0.y),
            &props,
        ) {
            return;
        }
        let player = args
            .projectile
            .get_owner_id()
            .and_then(|id| args.world.get_player_by_id(id));
        if ring_bell(
            *args.position,
            args.world,
            args.hit_face.to_horizontal_facing(),
            player.as_ref().map(|p| p.clone() as Arc<dyn EntityBase>),
        ) && let Some(player) = player
        {
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::BellRing as i32,
                1,
            );
        }
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        if args.r#type != 1 {
            return false;
        }
        if let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(bell) = entity.as_any().downcast_ref::<BellBlockEntity>()
        {
            bell.trigger_ring(args.world, args.data);
            true
        } else {
            false
        }
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        if args.can_trigger_blocks {
            ring_bell(*args.position, args.world, None, None);
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
