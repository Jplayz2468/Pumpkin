use super::super::block_receives_redstone_power;
use super::{
    RailProperties,
    common::{can_place_rail_at, rail_placement_is_valid},
};
use crate::{
    block::{
        BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnNeighborUpdateArgs,
        OnPlaceArgs, OnStateReplacedArgs, PlacedArgs,
    },
    world::World,
};
use pumpkin_data::{Block, BlockStateId, block_properties::RailShape};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

#[pumpkin_block("minecraft:powered_rail")]
pub struct PoweredRailBlock;

impl BlockBehaviour for PoweredRailBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        super::common::placement(args)
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        super::common::update_direction(args.world, *args.position, args.state_id, true);
        args.world.update_neighbor(args.position, args.block);
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.world.get_block(args.position) != args.block {
            return;
        }
        if !rail_placement_is_valid(args.world, args.block, args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        } else {
            Self::update_power(args.world, args.block, args.position);
        }
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        super::common::water_update(args)
    }
    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        super::common::removed(args, true);
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_rail_at(args.block_accessor, args.position)
    }
}
impl PoweredRailBlock {
    fn find_signal(
        world: &World,
        block: &Block,
        pos: BlockPos,
        props: RailProperties,
        forward: bool,
        depth: u8,
    ) -> bool {
        if depth >= 8 {
            return false;
        }
        let mut next = pos;
        let mut below = true;
        let mut axis = props.shape();
        match props.shape() {
            RailShape::NorthSouth => next.0.z += if forward { 1 } else { -1 },
            RailShape::EastWest => next.0.x += if forward { -1 } else { 1 },
            RailShape::AscendingEast => {
                next.0.x += if forward { -1 } else { 1 };
                if !forward {
                    next.0.y += 1;
                    below = false;
                }
                axis = RailShape::EastWest;
            }
            RailShape::AscendingWest => {
                next.0.x += if forward { -1 } else { 1 };
                if forward {
                    next.0.y += 1;
                    below = false;
                }
                axis = RailShape::EastWest;
            }
            RailShape::AscendingNorth => {
                next.0.z += if forward { 1 } else { -1 };
                if !forward {
                    next.0.y += 1;
                    below = false;
                }
                axis = RailShape::NorthSouth;
            }
            RailShape::AscendingSouth => {
                next.0.z += if forward { 1 } else { -1 };
                if forward {
                    next.0.y += 1;
                    below = false;
                }
                axis = RailShape::NorthSouth;
            }
            _ => return false,
        }
        Self::same_rail_with_power(world, block, next, forward, depth, axis)
            || below && Self::same_rail_with_power(world, block, next.down(), forward, depth, axis)
    }
    fn same_rail_with_power(
        world: &World,
        block: &Block,
        pos: BlockPos,
        forward: bool,
        depth: u8,
        axis: RailShape,
    ) -> bool {
        let (next_block, state) = world.get_block_and_state_id(&pos);
        if next_block != block {
            return false;
        }
        let props = RailProperties::new(state, block);
        let crosses = match axis {
            RailShape::EastWest => matches!(
                props.shape(),
                RailShape::NorthSouth | RailShape::AscendingNorth | RailShape::AscendingSouth
            ),
            RailShape::NorthSouth => matches!(
                props.shape(),
                RailShape::EastWest | RailShape::AscendingEast | RailShape::AscendingWest
            ),
            _ => false,
        };
        !crosses
            && props.is_powered()
            && (block_receives_redstone_power(world, &pos)
                || Self::find_signal(world, block, pos, props, forward, depth + 1))
    }
    fn update_power(world: &Arc<World>, block: &Block, pos: &BlockPos) {
        let mut props = RailProperties::new(world.get_block_state_id(pos), block);
        let powered = block_receives_redstone_power(world, pos)
            || Self::find_signal(
                world,
                block,
                *pos,
                RailProperties::new(world.get_block_state_id(pos), block),
                true,
                0,
            )
            || Self::find_signal(
                world,
                block,
                *pos,
                RailProperties::new(world.get_block_state_id(pos), block),
                false,
                0,
            );
        if powered != props.is_powered() {
            props.set_powered(powered);
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
            world.update_neighbors_at(&pos.down(), block, None);
            if props.shape().is_ascending() {
                world.update_neighbors_at(&pos.up(), block, None);
            }
        }
    }
}
