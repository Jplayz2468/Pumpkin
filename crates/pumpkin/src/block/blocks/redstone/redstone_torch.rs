use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::block::BlockIsReplacing;
use crate::block::CanPlaceAtArgs;
use crate::block::EmitsRedstonePowerArgs;
use crate::block::GetRedstonePowerArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnScheduledTickArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::PlacedArgs;
use crate::entity::EntityBase;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::FacingExt;
use pumpkin_data::HorizontalFacingExt;
use pumpkin_data::block_properties::Facing;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;
use pumpkin_world::world::BlockFlags;

type RWallTorchProps = pumpkin_data::block_properties::FurnaceLikeProperties;
type RTorchProps = pumpkin_data::block_properties::RedstoneOreLikeProperties;

use crate::block::{BlockBehaviour, BlockMetadata};
use crate::world::World;

use super::get_redstone_power;

static RECENT_TOGGLES: LazyLock<Mutex<HashMap<(uuid::Uuid, BlockPos), Vec<i64>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Checks if a torch at `pos` has been toggled >= 8 times in 60 ticks (burnout).
/// Matches vanilla RedstoneTorchBlock.java:133-150.
///
/// Vanilla keys `RECENT_TOGGLES` per-level (`Map<BlockGetter, List<Toggle>>`), not by
/// position alone -- two torches at the same coordinates in different dimensions (or
/// separate worlds) must not share burnout state. `world_id` carries that distinction.
pub fn is_toggled_too_frequently(
    world_id: uuid::Uuid,
    pos: BlockPos,
    current_time: i64,
    add: bool,
) -> bool {
    let mut lock = RECENT_TOGGLES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entries = lock.entry((world_id, pos)).or_default();
    entries.retain(|&time| current_time.saturating_sub(time) <= 60);

    if add {
        entries.push(current_time);
    }

    let too_frequent = entries.len() >= 8;
    if entries.is_empty() {
        lock.remove(&(world_id, pos));
    }
    too_frequent
}

pub struct RedstoneTorchBlock;

impl BlockMetadata for RedstoneTorchBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::REDSTONE_TORCH, BlockId::REDSTONE_WALL_TORCH].into()
    }
}

impl BlockBehaviour for RedstoneTorchBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let world = args.world;
        let block = args.block;
        let location = args.position;

        if args.direction == BlockDirection::Down {
            let support_block = world.get_block_state(&location.down());
            if support_block.is_center_solid(BlockDirection::Up) {
                return block.default_state.id;
            }
        }
        let mut directions = args.player.get_entity().get_entity_facing_order();

        if args.replacing == BlockIsReplacing::None {
            let face = args.direction.to_facing();
            let mut i = 0;
            while i < directions.len() && directions[i] != face {
                i += 1;
            }

            if i > 0 {
                directions.copy_within(0..i, 1);
                directions[0] = face;
            }
        } else if directions[0] == Facing::Down {
            let support_block = world.get_block_state(&location.down());
            if support_block.is_center_solid(BlockDirection::Up) {
                return block.default_state.id;
            }
        }

        for dir in directions {
            if dir != Facing::Up
                && dir != Facing::Down
                && can_place_at(world, location, dir.to_block_direction())
            {
                let mut torch_props = RWallTorchProps::default(&Block::REDSTONE_WALL_TORCH);
                if let Some(facing) = dir.opposite().to_horizontal_facing() {
                    torch_props.facing = facing;
                    return torch_props.to_state_id(&Block::REDSTONE_WALL_TORCH);
                }
            }
        }

        let support_block = world.get_block_state(&location.down());
        if support_block.is_center_solid(BlockDirection::Up) {
            block.default_state.id
        } else {
            BlockStateId::AIR
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support_block = args.block_accessor.get_block_state(&args.position.down());
        if support_block.is_center_solid(BlockDirection::Up) {
            return true;
        }
        for dir in BlockDirection::horizontal() {
            if can_place_at(args.block_accessor, args.position, dir.to_block_direction()) {
                return true;
            }
        }
        false
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(args.state_id);
            if props.facing.to_block_direction().opposite() == args.direction
                && !can_place_at(
                    args.world,
                    args.position,
                    props.facing.to_block_direction().opposite(),
                )
            {
                return BlockStateId::AIR;
            }
        } else if args.direction == BlockDirection::Down {
            let support_block = args.world.get_block_state(&args.position.down());
            if !support_block.is_center_solid(BlockDirection::Up) {
                return BlockStateId::AIR;
            }
        }
        args.state_id
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args
            .world
            .is_block_tick_scheduled(args.position, args.block)
        {
            return;
        }

        let state = args.world.get_block_state(args.position);
        let (lit, neighbor_signal) = if args.block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(state.id);
            let face = props.facing.to_block_direction().opposite();
            (props.lit, !should_be_lit(args.world, args.position, face))
        } else if args.block == &Block::REDSTONE_TORCH {
            let props = RTorchProps::from_state_id(state.id);
            (
                props.lit,
                !should_be_lit(args.world, args.position, BlockDirection::Down),
            )
        } else {
            return;
        };

        if lit == neighbor_signal {
            args.world
                .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(args.state.id);
            if props.lit && args.direction != props.facing.to_block_direction() {
                return 15;
            }
        } else if args.block == &Block::REDSTONE_TORCH {
            let props = RTorchProps::from_state_id(args.state.id);
            if props.lit && args.direction != BlockDirection::Up {
                return 15;
            }
        }
        0
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.direction == BlockDirection::Down {
            if args.block == &Block::REDSTONE_WALL_TORCH {
                let props = RWallTorchProps::from_state_id(args.state.id);
                if props.lit {
                    return 15;
                }
            } else if args.block == &Block::REDSTONE_TORCH {
                let props = RTorchProps::from_state_id(args.state.id);
                if props.lit {
                    return 15;
                }
            }
        }
        0
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (block, state) = args.world.get_block_and_state(args.position);
        let current_time = args.world.get_world_age();

        let (lit, neighbor_signal) = if block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(state.id);
            let face = props.facing.to_block_direction().opposite();
            (props.lit, !should_be_lit(args.world, args.position, face))
        } else if block == &Block::REDSTONE_TORCH {
            let props = RTorchProps::from_state_id(state.id);
            (
                props.lit,
                !should_be_lit(args.world, args.position, BlockDirection::Down),
            )
        } else {
            return;
        };

        if lit {
            if neighbor_signal {
                Self::set_lit(args.world, args.position, block, state.id, false);
                if is_toggled_too_frequently(args.world.uuid, *args.position, current_time, true) {
                    args.world.sync_world_event(
                        pumpkin_data::world::WorldEvent::RedstoneTorchBurnout,
                        *args.position,
                        0,
                    );
                    args.world.schedule_block_tick(
                        block,
                        *args.position,
                        160,
                        TickPriority::Normal,
                    );
                }
            }
        } else if !neighbor_signal
            && !is_toggled_too_frequently(args.world.uuid, *args.position, current_time, false)
        {
            Self::set_lit(args.world, args.position, block, state.id, true);
        }
    }

    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        update_neighbors(args.world, args.position);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        update_neighbors(args.world, args.position);
    }
}

impl RedstoneTorchBlock {
    fn set_lit(
        world: &Arc<World>,
        pos: &BlockPos,
        block: &Block,
        state_id: BlockStateId,
        lit: bool,
    ) {
        let new_state = if block == &Block::REDSTONE_WALL_TORCH {
            let mut props = RWallTorchProps::from_state_id(state_id);
            props.lit = lit;
            props.to_state_id(block)
        } else {
            let mut props = RTorchProps::from_state_id(state_id);
            props.lit = lit;
            props.to_state_id(block)
        };
        world.set_block_state(pos, new_state, BlockFlags::NOTIFY_ALL);
    }
}

pub fn should_be_lit(world: &World, pos: &BlockPos, face: BlockDirection) -> bool {
    let other_pos = pos.offset(face.to_offset());
    let (block, state) = world.get_block_and_state(&other_pos);
    get_redstone_power(block, state, world, &other_pos, face) == 0
}

pub fn update_neighbors(world: &Arc<World>, pos: &BlockPos) {
    for dir in BlockDirection::all() {
        let other_pos = pos.offset(dir.to_offset());
        world.update_neighbors(&other_pos, None);
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos, facing: BlockDirection) -> bool {
    world
        .get_block_state(&block_pos.offset(facing.to_offset()))
        .is_side_solid(facing.opposite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_torch_burnout_counter() {
        let world_id = uuid::Uuid::new_v4();
        let pos = BlockPos::new(100, 64, 100);
        let start_time = 1000;

        // Toggling 7 times within 60 ticks: not burned out yet
        for i in 0..7 {
            assert!(!is_toggled_too_frequently(
                world_id,
                pos,
                start_time + i * 2,
                true
            ));
        }

        // 8th toggle within 60 ticks: burns out!
        assert!(is_toggled_too_frequently(
            world_id,
            pos,
            start_time + 14,
            true
        ));

        // While still within 60 ticks: query returns true
        assert!(is_toggled_too_frequently(
            world_id,
            pos,
            start_time + 20,
            false
        ));

        // After 61 ticks: old toggles are pruned, no longer burned out
        assert!(!is_toggled_too_frequently(
            world_id,
            pos,
            start_time + 80,
            false
        ));
    }

    #[test]
    fn test_torch_burnout_is_per_world() {
        // Two different worlds/dimensions with a torch at the same coordinates must not
        // share burnout state (vanilla keys RECENT_TOGGLES per-level, not by pos alone).
        let world_a = uuid::Uuid::new_v4();
        let world_b = uuid::Uuid::new_v4();
        let pos = BlockPos::new(200, 64, 200);
        let start_time = 5000;

        for i in 0..8 {
            is_toggled_too_frequently(world_a, pos, start_time + i * 2, true);
        }
        assert!(is_toggled_too_frequently(
            world_a,
            pos,
            start_time + 14,
            false
        ));
        assert!(!is_toggled_too_frequently(
            world_b,
            pos,
            start_time + 14,
            false
        ));
    }
}
