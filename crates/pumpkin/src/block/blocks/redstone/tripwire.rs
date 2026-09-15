use std::sync::Arc;

use pumpkin_data::item::Item;
use pumpkin_data::{Block, BlockDirection, BlockStateId, block_properties::HorizontalFacing};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos};
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

use crate::{
    block::{
        BlockBehaviour, BrokenArgs, GetInsideCollisionShapeArgs, GetStateForNeighborUpdateArgs,
        OnEntityCollisionArgs, OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs, PlacedArgs,
    },
    world::World,
};

use super::tripwire_hook::TripwireHookBlock;

type TripwireProperties = pumpkin_data::block_properties::TripwireLikeProperties;
type TripwireHookProperties = pumpkin_data::block_properties::TripwireHookLikeProperties;

#[pumpkin_block("minecraft:tripwire")]
pub struct TripwireBlock;

impl BlockBehaviour for TripwireBlock {
    fn get_inside_collision_shape(&self, args: GetInsideCollisionShapeArgs<'_>) -> BoundingBox {
        Self::detection_box(args.state.id)
    }
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if !TripwireProperties::from_state_id(args.state.id).powered
            && !args
                .world
                .is_block_tick_scheduled(args.position, args.block)
        {
            Self::check_pressed(
                args.world,
                args.position,
                !args.entity.is_ignoring_block_triggers(),
            );
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let [connect_north, connect_east, connect_south, connect_west] = [
            BlockDirection::North,
            BlockDirection::East,
            BlockDirection::South,
            BlockDirection::West,
        ]
        .map(|dir| {
            let current_pos = args.position.offset(dir.to_offset());
            let state_id = args.world.get_block_state_id(&current_pos);
            Self::should_connect_to(state_id, dir)
        });

        let mut props = TripwireProperties::from_state_id(args.block.default_state.id);

        props.north = connect_north;
        props.south = connect_south;
        props.west = connect_west;
        props.east = connect_east;

        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if Block::from_state_id(args.old_state_id) == Block::from_state_id(args.state_id) {
            return;
        }

        Self::update(args.world, args.position, args.state_id);
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        let has_shears = args.player.inventory().held_item().get_item() == &Item::SHEARS;
        if has_shears {
            let mut props = TripwireProperties::from_state_id(args.state.id);
            props.disarmed = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
            args.world.emit_game_event_from_entity(
                pumpkin_data::game_event::GameEvent::Shear.name(),
                args.position.to_centered_f64(),
                Some(args.player.as_ref()),
                None,
            );
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        args.direction
            .to_horizontal_facing()
            .map_or(args.state_id, |facing| {
                let mut props = TripwireProperties::from_state_id(args.state_id);
                *match facing {
                    HorizontalFacing::North => &mut props.north,
                    HorizontalFacing::South => &mut props.south,
                    HorizontalFacing::West => &mut props.west,
                    HorizontalFacing::East => &mut props.east,
                } = Self::should_connect_to(args.neighbor_state_id, args.direction);
                props.to_state_id(args.block)
            })
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state_id(args.position);
        if TripwireProperties::from_state_id(state).powered {
            let bounds = Self::detection_box(state).at_pos(*args.position);
            let pressed = args
                .world
                .get_all_at_box(&bounds)
                .iter()
                .any(|entity| !entity.is_spectator() && !entity.is_ignoring_block_triggers());
            Self::check_pressed(args.world, args.position, pressed);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if args.moved {
            return;
        }
        let mut props = TripwireProperties::from_state_id(args.old_state_id);
        props.powered = true;
        let state_id = props.to_state_id(args.block);
        Self::update(args.world, args.position, state_id);
    }
}

impl TripwireBlock {
    fn detection_box(state: BlockStateId) -> BoundingBox {
        if TripwireProperties::from_state_id(state).attached {
            BoundingBox::new_array([0.0, 1.0 / 16.0, 0.0], [1.0, 2.5 / 16.0, 1.0])
        } else {
            BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, 0.5, 1.0])
        }
    }
    fn check_pressed(world: &Arc<World>, pos: &BlockPos, pressed: bool) {
        let mut props = TripwireProperties::from_state_id(world.get_block_state_id(pos));
        let was_pressed = props.powered;
        if pressed != was_pressed {
            props.powered = pressed;
            let state = props.to_state_id(&Block::TRIPWIRE);
            world.set_block_state(pos, state, BlockFlags::NOTIFY_ALL);
            Self::update(world, pos, state);
        }
        if pressed || was_pressed {
            world.schedule_block_tick(
                &Block::TRIPWIRE,
                *pos,
                if pressed { 10 } else { 0 },
                TickPriority::Normal,
            );
        }
    }

    fn update(world: &Arc<World>, pos: &BlockPos, state_id: BlockStateId) {
        for dir in [BlockDirection::South, BlockDirection::West] {
            for i in 1..42 {
                let current_pos = pos.offset_dir(dir.to_offset(), i);
                let (current_block, current_state) = world.get_block_and_state_id(&current_pos);
                if current_block == &Block::TRIPWIRE_HOOK {
                    let current_props = TripwireHookProperties::from_state_id(current_state);
                    if dir
                        .opposite()
                        .to_horizontal_facing()
                        .is_some_and(|f| current_props.facing == f)
                    {
                        TripwireHookBlock::update(
                            world,
                            current_pos,
                            current_state,
                            false,
                            true,
                            i,
                            Some(state_id),
                        );
                    }
                    break;
                }
                if current_block != &Block::TRIPWIRE {
                    break;
                }
            }
        }
    }

    #[must_use]
    pub fn should_connect_to(state_id: BlockStateId, facing: BlockDirection) -> bool {
        let block = Block::from_state_id(state_id);
        if block == &Block::TRIPWIRE_HOOK {
            let props = TripwireHookProperties::from_state_id(state_id);
            Some(props.facing) == facing.opposite().to_horizontal_facing()
        } else {
            block == &Block::TRIPWIRE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tripwire_should_connect_to_tripwire() {
        let wire_state = Block::TRIPWIRE.default_state.id;
        for dir in [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
        ] {
            assert!(TripwireBlock::should_connect_to(wire_state, dir));
        }
    }

    #[test]
    fn test_tripwire_should_connect_to_matching_hook() {
        // A hook facing South connects to a tripwire to its South (direction from wire is North)
        let mut hook_props = TripwireHookProperties::default(&Block::TRIPWIRE_HOOK);
        hook_props.facing = HorizontalFacing::South;
        let hook_state = hook_props.to_state_id(&Block::TRIPWIRE_HOOK);

        // When looking North from wire towards hook, facing.opposite() is South -> matches
        assert!(TripwireBlock::should_connect_to(
            hook_state,
            BlockDirection::North
        ));
        // Other directions should not match
        assert!(!TripwireBlock::should_connect_to(
            hook_state,
            BlockDirection::South
        ));
        assert!(!TripwireBlock::should_connect_to(
            hook_state,
            BlockDirection::East
        ));
        assert!(!TripwireBlock::should_connect_to(
            hook_state,
            BlockDirection::West
        ));
    }

    #[test]
    fn test_tripwire_should_not_connect_to_other_blocks() {
        let air = Block::AIR.default_state.id;
        let stone = Block::STONE.default_state.id;
        assert!(!TripwireBlock::should_connect_to(
            air,
            BlockDirection::North
        ));
        assert!(!TripwireBlock::should_connect_to(
            stone,
            BlockDirection::South
        ));
    }
}
