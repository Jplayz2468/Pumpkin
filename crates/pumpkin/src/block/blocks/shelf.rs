use crate::{
    block::{
        BlockBehaviour, GetComparatorOutputArgs, GetStateForNeighborUpdateArgs,
        OnNeighborUpdateArgs, OnPlaceArgs, OnStateReplacedArgs, PathComputationType, PlacedArgs,
        UseWithItemArgs, blocks::redstone::block_receives_redstone_power,
        entities::shelf::ShelfBlockEntity, registry::BlockActionResult,
    },
    entity::EntityBase,
    world::World,
};
use pumpkin_data::{
    BlockState, BlockStateId, HorizontalFacingExt,
    block_properties::{AcaciaShelfLikeProperties as ShelfProps, HorizontalFacing, SideChainPart},
    data_component_impl::{EquipmentSlot, UseEffectsImpl},
    fluid::Fluid,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
};
use pumpkin_inventory::{Inventory, screen_handler::InventoryPlayer};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use std::sync::Arc;

#[pumpkin_block_from_tag("minecraft:wooden_shelves")]
pub struct ShelfBlock;

fn connectable(state: BlockStateId) -> Option<ShelfProps> {
    state
        .to_block()
        .has_tag(&tag::Block::MINECRAFT_WOODEN_SHELVES)
        .then(|| ShelfProps::from_state_id(state))
        .filter(|props| props.powered)
}

fn neighbor_part(world: &World, pos: &BlockPos, facing: HorizontalFacing) -> Option<SideChainPart> {
    connectable(world.get_block_state_id(pos))
        .filter(|props| props.facing == facing)
        .map(|props| props.side_chain)
}

fn set_part(world: &Arc<World>, pos: &BlockPos, part: SideChainPart) {
    let (block, state) = world.get_block_and_state_id(pos);
    if !block.has_tag(&tag::Block::MINECRAFT_WOODEN_SHELVES) {
        return;
    }
    let mut props = ShelfProps::from_state_id(state);
    if props.side_chain != part {
        props.side_chain = part;
        world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
    }
}

fn connected_blocks(world: &World, pos: &BlockPos) -> Vec<BlockPos> {
    let Some(props) = connectable(world.get_block_state_id(pos)) else {
        return Vec::new();
    };
    let facing = props.facing.to_block_direction();
    let mut result = vec![*pos];
    for (direction, end) in [
        (facing.rotate_clockwise(), SideChainPart::Left),
        (facing.rotate_counter_clockwise(), SideChainPart::Right),
    ] {
        for step in 1..3 {
            let neighbor = pos.offset(direction.to_offset() * step);
            let Some(part) = neighbor_part(world, &neighbor, props.facing) else {
                break;
            };
            if part == SideChainPart::Center || part == end {
                if end == SideChainPart::Left {
                    result.insert(0, neighbor);
                } else {
                    result.push(neighbor);
                }
            }
            if part != SideChainPart::Center {
                break;
            }
        }
    }
    result
}

fn disconnect_neighbors(world: &Arc<World>, pos: &BlockPos, props: ShelfProps) {
    let facing = props.facing.to_block_direction();
    let left = pos.offset(facing.rotate_clockwise().to_offset());
    let right = pos.offset(facing.rotate_counter_clockwise().to_offset());
    if let Some(part) = neighbor_part(world, &left, props.facing) {
        set_part(
            world,
            &left,
            match part {
                SideChainPart::Right | SideChainPart::Center => SideChainPart::Right,
                _ => SideChainPart::Unconnected,
            },
        );
    }
    if let Some(part) = neighbor_part(world, &right, props.facing) {
        set_part(
            world,
            &right,
            match part {
                SideChainPart::Left | SideChainPart::Center => SideChainPart::Left,
                _ => SideChainPart::Unconnected,
            },
        );
    }
}

fn on_place(args: PlacedArgs<'_>) {
    let props = ShelfProps::from_state_id(args.state_id);
    if !props.powered {
        disconnect_neighbors(args.world, args.position, props);
        return;
    }
    if props.side_chain != SideChainPart::Unconnected
        || connectable(args.old_state_id)
            .is_some_and(|old| old.side_chain != SideChainPart::Unconnected)
    {
        return;
    }
    let facing = props.facing.to_block_direction();
    let left = args.position.offset(facing.rotate_clockwise().to_offset());
    let right = args
        .position
        .offset(facing.rotate_counter_clockwise().to_offset());
    // Cache the neighboring parts before updating either side, as SideChainPartBlock does.
    let left_part = neighbor_part(args.world, &left, props.facing);
    let right_part = neighbor_part(args.world, &right, props.facing);
    let left_count = if left_part.is_some() {
        connected_blocks(args.world, &left).len()
    } else {
        0
    };
    let right_count = if right_part.is_some() {
        connected_blocks(args.world, &right).len()
    } else {
        0
    };
    let mut own = SideChainPart::Unconnected;
    let mut count = 1;
    if left_count > 0 && count + left_count <= 3 {
        own = SideChainPart::Right;
        set_part(
            args.world,
            &left,
            match left_part.unwrap() {
                SideChainPart::Right | SideChainPart::Center => SideChainPart::Center,
                _ => SideChainPart::Left,
            },
        );
        count += left_count;
    }
    if right_count > 0 && count + right_count <= 3 {
        own = if own == SideChainPart::Right {
            SideChainPart::Center
        } else {
            SideChainPart::Left
        };
        set_part(
            args.world,
            &right,
            match right_part.unwrap() {
                SideChainPart::Left | SideChainPart::Center => SideChainPart::Center,
                _ => SideChainPart::Right,
            },
        );
    }
    set_part(args.world, args.position, own);
}

impl BlockBehaviour for ShelfBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = ShelfProps::default(args.block);
        props.facing = args.player.get_entity().get_horizontal_facing().opposite();
        props.powered = block_receives_redstone_power(args.world, args.position);
        let (fluid, state) =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position));
        props.waterlogged = fluid.matches_type(&Fluid::WATER) && state.is_source;
        props.to_state_id(args.block)
    }
    fn placed(&self, args: PlacedArgs<'_>) {
        on_place(args);
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        on_place(args);
    }
    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
        disconnect_neighbors(
            args.world,
            args.position,
            ShelfProps::from_state_id(args.old_state_id),
        );
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let mut props = ShelfProps::from_state_id(args.world.get_block_state_id(args.position));
        let power = block_receives_redstone_power(args.world, args.position);
        if props.powered == power {
            return;
        }
        props.powered = power;
        if !power {
            props.side_chain = SideChainPart::Unconnected;
        }
        let state = props.to_state_id(args.block);
        args.world
            .set_block_state(args.position, state, BlockFlags::NOTIFY_ALL);
        args.world.play_sound(
            if power {
                Sound::BlockShelfActivate
            } else {
                Sound::BlockShelfDeactivate
            },
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
        );
        args.world.emit_game_event_from_entity(
            if power {
                "block_activate"
            } else {
                "block_deactivate"
            },
            args.position.to_centered_f64(),
            None,
            Some(state),
        );
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if ShelfProps::from_state_id(args.state_id).waterlogged {
            args.world
                .schedule_fluid_tick(&Fluid::WATER, *args.position, 5, TickPriority::Normal);
        }
        args.state_id
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if *args.equipment_slot == EquipmentSlot::OFF_HAND {
            return BlockActionResult::Pass;
        }
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(shelf) = entity.as_any().downcast_ref::<ShelfBlockEntity>() else {
            return BlockActionResult::Pass;
        };
        let props = ShelfProps::from_state_id(args.world.get_block_state_id(args.position));
        if *args.hit.face != props.facing.to_block_direction() {
            return BlockActionResult::Pass;
        }
        let x = match props.facing {
            HorizontalFacing::North => 1.0 - args.hit.cursor_pos.x,
            HorizontalFacing::South => args.hit.cursor_pos.x,
            HorizontalFacing::West => args.hit.cursor_pos.z,
            HorizontalFacing::East => 1.0 - args.hit.cursor_pos.z,
        };
        let slot = ((x * 16.0 / (16.0 / 3.0)).floor() as i32).clamp(0, 2) as usize;
        let inventory = args.player.inventory();
        let sound;
        if !props.powered {
            let was_empty = args.item_stack.is_empty();
            args.item_stack.item_count = args
                .item_stack
                .item_count
                .min(args.item_stack.get_max_stack_size().min(64));
            let removed = shelf.swap_item_no_update(slot, args.item_stack.clone());
            let had_item = !removed.is_empty();
            let new_item = if args.player.has_infinite_materials() && !had_item {
                args.item_stack.clone()
            } else {
                removed
            };
            let vibrations = new_item
                .get_data_component::<UseEffectsImpl>()
                .is_none_or(|effects| effects.interact_vibrations);
            inventory.set_held_item(new_item.clone());
            *args.item_stack = new_item;
            inventory.mark_dirty();
            shelf.set_changed(if vibrations {
                Some("item_interact_finish")
            } else {
                None
            });
            if !had_item && was_empty {
                return BlockActionResult::Pass;
            }
            sound = if had_item {
                if was_empty {
                    Sound::BlockShelfTakeItem
                } else {
                    Sound::BlockShelfSingleSwap
                }
            } else {
                Sound::BlockShelfPlaceItem
            };
        } else {
            let connected = connected_blocks(args.world, args.position);
            let mut any_swapped = false;
            for (part_index, pos) in connected.iter().enumerate() {
                let Some(entity) = args.world.get_block_entity(pos) else {
                    continue;
                };
                let Some(shelf) = entity.as_any().downcast_ref::<ShelfBlockEntity>() else {
                    continue;
                };
                for slot in 0..3 {
                    let inventory_slot =
                        9 - ((connected.len() - part_index) * 3) as isize + slot as isize;
                    if inventory_slot < 0 || inventory_slot > inventory.size() as isize {
                        continue;
                    }
                    let inventory_slot = inventory_slot as usize;
                    let placed = inventory.remove_stack(inventory_slot);
                    let was_empty = placed.is_empty();
                    let removed = shelf.swap_item_no_update(slot, placed);
                    if !was_empty || !removed.is_empty() {
                        inventory.set_stack(inventory_slot, removed);
                        any_swapped = true;
                    }
                }
                inventory.mark_dirty();
                shelf.set_changed(Some("entity_interact"));
            }
            *args.item_stack = inventory.held_item();
            if !any_swapped {
                return BlockActionResult::Consume;
            }
            sound = Sound::BlockShelfMultiSwap;
        }
        args.player.sync_inventory_to_client();
        args.world.play_sound(
            sound,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
        );
        BlockActionResult::Success
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = ShelfProps::from_state_id(args.state.id);
        if args.direction != props.facing.to_block_direction().opposite() {
            return Some(0);
        }
        Some(
            args.world
                .get_block_entity(args.position)
                .and_then(|entity| {
                    entity
                        .as_any()
                        .downcast_ref::<ShelfBlockEntity>()
                        .map(|shelf| {
                            u8::from(!shelf.get_stack(0).is_empty())
                                | (u8::from(!shelf.get_stack(1).is_empty()) << 1)
                                | (u8::from(!shelf.get_stack(2).is_empty()) << 2)
                        })
                })
                .unwrap_or(0),
        )
    }
    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        computation_type == PathComputationType::Water && state.is_waterlogged()
    }
}
