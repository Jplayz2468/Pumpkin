use std::sync::Arc;

use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs, PathComputationType, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::block_properties::{Facing, LightningRodLikeProperties};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{BlockState, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block("minecraft:lightning_rod")]
pub struct LightningRodBlock;

impl LightningRodBlock {
    pub fn trigger(world: &Arc<World>, pos: &BlockPos) {
        let (block, state_id) = world.get_block_and_state_id(pos);
        let mut props = LightningRodLikeProperties::from_state_id(state_id);
        if !props.powered {
            props.powered = true;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);

            Self::update_neighbors(world, pos, props);

            // In vanilla, it stays powered for 8 ticks (4 redstone ticks) before scheduled tick turns it off.
            world.schedule_block_tick(block, *pos, 8, TickPriority::Normal);

            let axis_index = match props.facing {
                Facing::East | Facing::West => 0,
                Facing::Up | Facing::Down => 1,
                Facing::North | Facing::South => 2,
            };
            world.sync_world_event(WorldEvent::ParticlesElectricSpark, *pos, axis_index);
        }
    }

    fn update_neighbors(world: &Arc<World>, pos: &BlockPos, props: LightningRodLikeProperties) {
        world.update_neighbors(pos, None);
        // The block it is attached to is in the opposite of the facing direction
        let attached_pos = pos.offset(props.facing.opposite().to_block_direction().to_offset());
        world.update_neighbors(&attached_pos, None);
    }
}

impl BlockBehaviour for LightningRodBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LightningRodLikeProperties::default(args.block);
        props.facing = args.direction.to_facing().opposite();
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let props = LightningRodLikeProperties::from_state_id(args.state_id);
        if props.powered {
            args.world
                .schedule_block_tick(args.block, *args.position, 8, TickPriority::Normal);
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = LightningRodLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u8,
                TickPriority::Normal,
            );
        }
        props.to_state_id(args.block)
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LightningRodLikeProperties::from_state_id(args.state.id);
        if props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LightningRodLikeProperties::from_state_id(args.state.id);
        // It emits strong power only in its facing direction (the direction pointing outward)
        if props.powered && props.facing.to_block_direction() == args.direction {
            15
        } else {
            0
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = LightningRodLikeProperties::from_state_id(state.id);
        if props.powered {
            props.powered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            Self::update_neighbors(args.world, args.position, props);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let props = LightningRodLikeProperties::from_state_id(args.old_state_id);
            if props.powered {
                Self::update_neighbors(args.world, args.position, props);
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::{Block, BlockDirection};

    #[test]
    fn lightning_rod_power_logic() {
        let block = &Block::LIGHTNING_ROD;
        let mut props = LightningRodLikeProperties::default(block);
        props.facing = Facing::Up;
        props.powered = false;

        let weak_power = |p: LightningRodLikeProperties| if p.powered { 15 } else { 0 };
        let strong_power = |p: LightningRodLikeProperties, dir: BlockDirection| {
            if p.powered && p.facing.to_block_direction() == dir {
                15
            } else {
                0
            }
        };

        assert_eq!(weak_power(props), 0);
        assert_eq!(strong_power(props, BlockDirection::Up), 0);

        props.powered = true;
        // Weak power is 15 in all directions when powered
        assert_eq!(weak_power(props), 15);

        // Strong power is 15 strictly in facing direction (Up)
        assert_eq!(strong_power(props, BlockDirection::Up), 15);
        assert_eq!(strong_power(props, BlockDirection::Down), 0);
        assert_eq!(strong_power(props, BlockDirection::North), 0);
    }
}

