use crate::block::entities::creaking_heart::CreakingHeartBlockEntity;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs,
};
use crate::world::World;
use pumpkin_data::block_properties::{
    Axis, CreakingHeartLikeProperties, CreakingHeartState, PaleOakWoodLikeProperties,
};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::{GameMode, math::position::BlockPos};
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

#[pumpkin_block("minecraft:creaking_heart")]
pub struct CreakingHeartBlock;

impl CreakingHeartBlock {
    fn has_required_logs(world: &World, pos: &BlockPos, axis: Axis) -> bool {
        let neighbors = match axis {
            Axis::X => [pos.west(), pos.east()],
            Axis::Y => [pos.down(), pos.up()],
            Axis::Z => [pos.north(), pos.south()],
        };
        neighbors.iter().all(|neighbor| {
            let (block, state) = world.get_block_and_state(neighbor);
            block.has_tag(&tag::Block::MINECRAFT_PALE_OAK_LOGS)
                && PaleOakWoodLikeProperties::from_state_id(state.id).axis == axis
        })
    }

    fn updated_state(
        world: &World,
        pos: &BlockPos,
        mut props: CreakingHeartLikeProperties,
    ) -> BlockStateId {
        if props.creaking_heart_state == CreakingHeartState::Uprooted
            && Self::has_required_logs(world, pos, props.axis)
        {
            props.creaking_heart_state = if world.creaking_active(pos) {
                CreakingHeartState::Awake
            } else {
                CreakingHeartState::Dormant
            };
        }
        props.to_state_id(&Block::CREAKING_HEART)
    }
}

impl BlockBehaviour for CreakingHeartBlock {
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = CreakingHeartLikeProperties::from_state_id(args.state.id);
        if props.creaking_heart_state == CreakingHeartState::Uprooted {
            return Some(0);
        }
        Some(
            args.world
                .get_block_entity(args.position)
                .and_then(|entity| {
                    entity
                        .as_any()
                        .downcast_ref::<CreakingHeartBlockEntity>()
                        .map(CreakingHeartBlockEntity::analog_output_signal)
                })
                .unwrap_or(0),
        )
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = CreakingHeartLikeProperties::default(args.block);
        props.axis = args.direction.to_axis();
        Self::updated_state(args.world, args.position, props)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        args.world
            .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (block, state) = args.world.get_block_and_state(args.position);
        if block != args.block {
            return;
        }
        let new_state = Self::updated_state(
            args.world,
            args.position,
            CreakingHeartLikeProperties::from_state_id(state.id),
        );
        if new_state != state.id {
            args.world
                .set_block_state(args.position, new_state, BlockFlags::NOTIFY_ALL);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        let props = CreakingHeartLikeProperties::from_state_id(args.state.id);
        if props.natural
            && !matches!(
                args.player.gamemode.load(),
                GameMode::Creative | GameMode::Spectator
            )
            && args
                .world
                .get_block_entity(args.position)
                .is_some_and(|entity| entity.as_any().is::<CreakingHeartBlockEntity>())
        {
            let amount = 20 + args.world.rand_bounded_i32(5);
            let mut event = crate::plugin::block::block_exp::BlockExpEvent {
                block_pos: *args.position,
                world: args.world.clone(),
                exp: amount,
            };
            if let Some(server) = args.world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.exp > 0 && args.world.level_info.load().game_rules.block_drops {
                crate::entity::experience_orb::ExperienceOrbEntity::spawn(
                    args.world,
                    args.position.to_centered_f64(),
                    event.exp as u32,
                );
            }
        }
    }
}
