use crate::entity::EntityBase;
use std::sync::Arc;

use pumpkin_data::{
    Block, BlockDirection, BlockStateId, FacingExt, HorizontalFacingExt,
    game_event::GameEvent,
    sound::{Sound, SoundCategory},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

use crate::{
    block::{
        BlockBehaviour, CanPlaceAtArgs, EmitsRedstonePowerArgs, GetRedstonePowerArgs,
        GetStateForNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs,
        PlayerPlacedArgs,
    },
    world::World,
};

type TripwireProperties = pumpkin_data::block_properties::TripwireLikeProperties;
type TripwireHookProperties = pumpkin_data::block_properties::TripwireHookLikeProperties;

#[pumpkin_block("minecraft:tripwire_hook")]
pub struct TripwireHookBlock;

impl BlockBehaviour for TripwireHookBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = TripwireHookProperties::default(args.block);
        props.powered = false;
        props.attached = false;
        let mut directions = args.player.get_entity().get_entity_facing_order();
        if args.use_item_on.position != *args.position {
            if let Some(index) = directions
                .iter()
                .position(|face| *face == args.direction.to_facing())
            {
                directions[..=index].rotate_right(1);
            }
        }
        for direction in directions {
            let facing = direction.opposite().to_block_direction();
            if Self::can_place_at(args.world, args.position, facing) {
                props.facing = facing.to_cardinal_direction();
                return props.to_state_id(args.block);
            }
        }
        Block::AIR.default_state.id
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let props = TripwireHookProperties::from_state_id(args.state.id);

        Self::can_place_at(
            args.block_accessor,
            args.position,
            props.facing.to_block_direction(),
        )
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        Self::update(
            args.world,
            *args.position,
            args.state_id,
            false,
            false,
            -1,
            None,
        );
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction.to_horizontal_facing().is_some_and(|facing| {
            let props = TripwireHookProperties::from_state_id(args.state_id);
            facing.opposite() == props.facing
        }) && !Self::can_place_at(args.world, args.position, args.direction.opposite())
        {
            Block::AIR.default_state.id
        } else {
            args.state_id
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        if state_id.to_block() == args.block {
            Self::update(args.world, *args.position, state_id, false, true, -1, None);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            Self::removed(args.world, *args.position, args.old_state_id);
        }
    }

    #[inline]
    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = TripwireHookProperties::from_state_id(args.state.id);
        if props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = TripwireHookProperties::from_state_id(args.state.id);
        if props.powered
            && args
                .direction
                .to_horizontal_facing()
                .is_some_and(|facing| props.facing == facing)
        {
            15
        } else {
            0
        }
    }
}

impl TripwireHookBlock {
    fn removed(world: &Arc<World>, pos: BlockPos, state: BlockStateId) {
        let props = TripwireHookProperties::from_state_id(state);
        if props.powered || props.attached {
            Self::update(world, pos, state, true, false, -1, None);
        }
        if props.powered {
            Self::update_neighbors_on_axis(
                &Block::TRIPWIRE_HOOK,
                world,
                pos,
                props.facing.to_block_direction(),
            );
        }
    }

    pub fn can_place_at(
        world: &dyn BlockAccessor,
        block_pos: &BlockPos,
        face: BlockDirection,
    ) -> bool {
        if !face.is_horizontal() {
            return false;
        }
        let place_block_pos = block_pos.offset(face.opposite().to_offset());
        let place_block_state = world.get_block_state(&place_block_pos);
        place_block_state.is_side_solid(face)
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(
        world: &Arc<World>,
        start_hook_pos: BlockPos,
        start_hook_state_id: BlockStateId,
        skip_state_update: bool,
        notify_neighbors: bool,
        raw_wire_index: i32,
        raw_wire_state: Option<BlockStateId>,
    ) {
        let start_hook_props = TripwireHookProperties::from_state_id(start_hook_state_id);
        let mut can_attach = !skip_state_update;
        let mut wire_attached = false;
        let mut j = 0;
        let mut wires_props: Vec<Option<TripwireProperties>> = vec![None; 42];

        for k in 1..42 {
            let current_pos = start_hook_pos.offset_dir(start_hook_props.facing.to_offset(), k);
            let current_block = world.get_block(&current_pos);
            if current_block == &Block::TRIPWIRE_HOOK {
                let current_hook_props = {
                    let state_id = world.get_block_state_id(&current_pos);
                    TripwireHookProperties::from_state_id(state_id)
                };
                if current_hook_props.facing == start_hook_props.facing.opposite() {
                    j = k;
                }
                break;
            }
            if current_block == &Block::TRIPWIRE || k == raw_wire_index {
                let current_wire_props = {
                    let ro_state_id = world.get_block_state_id(&current_pos);
                    let state_id = if k == raw_wire_index {
                        raw_wire_state.unwrap_or(ro_state_id)
                    } else {
                        ro_state_id
                    };
                    TripwireProperties::from_state_id(state_id)
                };
                wire_attached |= (!current_wire_props.disarmed) && current_wire_props.powered;
                wires_props[k as usize] = Some(current_wire_props);
                if k == raw_wire_index {
                    world.schedule_block_tick(
                        &Block::TRIPWIRE_HOOK,
                        start_hook_pos,
                        10,
                        TickPriority::Normal,
                    );
                    can_attach &= !current_wire_props.disarmed;
                }
            } else {
                wires_props[k as usize] = None;
                can_attach = false;
            }
        }

        let future_attached = can_attach && (j > 1);
        let future_powered = wire_attached && future_attached;
        let mut future_hook_state = TripwireHookProperties::default(&Block::TRIPWIRE_HOOK);
        future_hook_state.attached = future_attached;
        future_hook_state.powered = future_powered;

        if j > 0 {
            let end_hook_pos = start_hook_pos.offset_dir(start_hook_props.facing.to_offset(), j);
            let future_hook_facing = start_hook_props.facing.opposite();
            let mut future_end_hook_state = future_hook_state;
            future_end_hook_state.facing = future_hook_facing;
            world.set_block_state(
                &end_hook_pos,
                future_end_hook_state.to_state_id(&Block::TRIPWIRE_HOOK),
                BlockFlags::NOTIFY_ALL,
            );
            Self::update_neighbors_on_axis(
                &Block::TRIPWIRE_HOOK,
                world,
                end_hook_pos,
                BlockDirection::from_cardinal_direction(future_hook_facing),
            );
            if world.get_block(&start_hook_pos) != &Block::TRIPWIRE_HOOK {
                Self::removed(
                    world,
                    start_hook_pos,
                    future_hook_state.to_state_id(&Block::TRIPWIRE_HOOK),
                );
                return;
            }
            Self::play_sound(
                world,
                &end_hook_pos,
                future_attached,
                future_powered,
                start_hook_props.attached,
                start_hook_props.powered,
            );
        }

        Self::play_sound(
            world,
            &start_hook_pos,
            future_attached,
            future_powered,
            start_hook_props.attached,
            start_hook_props.powered,
        );

        if !skip_state_update {
            let mut future_start_hook_state = future_hook_state;
            future_start_hook_state.facing = start_hook_props.facing;
            world.set_block_state(
                &start_hook_pos,
                future_start_hook_state.to_state_id(&Block::TRIPWIRE_HOOK),
                BlockFlags::NOTIFY_ALL,
            );
            if notify_neighbors {
                Self::update_neighbors_on_axis(
                    &Block::TRIPWIRE_HOOK,
                    world,
                    start_hook_pos,
                    BlockDirection::from_cardinal_direction(start_hook_props.facing),
                );
            }
        }

        if start_hook_props.attached != future_attached {
            for l in 1..j {
                let current_wrie_pos =
                    start_hook_pos.offset_dir(start_hook_props.facing.to_offset(), l);
                if let Some(mut lv8) = wires_props[l as usize]
                    && matches!(
                        world.get_block(&current_wrie_pos).id,
                        pumpkin_data::BlockId::TRIPWIRE | pumpkin_data::BlockId::TRIPWIRE_HOOK
                    )
                {
                    lv8.attached = future_attached;
                    world.set_block_state(
                        &current_wrie_pos,
                        lv8.to_state_id(&Block::TRIPWIRE),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
            }
        }
    }

    #[expect(clippy::fn_params_excessive_bools)]
    fn play_sound(
        world: &Arc<World>,
        block_pos: &BlockPos,
        attached: bool,
        powered: bool,
        was_attached: bool,
        was_powered: bool,
    ) {
        let cat = SoundCategory::Blocks;
        let pos = block_pos.to_centered_f64();
        if powered && !was_powered {
            world.play_sound_raw(Sound::BlockTripwireClickOn as u16, cat, &pos, 0.4, 0.6);
            world.emit_game_event(GameEvent::BlockActivate.name(), block_pos.to_centered_f64());
        } else if !powered && was_powered {
            world.play_sound_raw(Sound::BlockTripwireClickOff as u16, cat, &pos, 0.4, 0.5);
            world.emit_game_event(
                GameEvent::BlockDeactivate.name(),
                block_pos.to_centered_f64(),
            );
        } else if attached && !was_attached {
            world.play_sound_raw(Sound::BlockTripwireAttach as u16, cat, &pos, 0.4, 0.7);
            world.emit_game_event(GameEvent::BlockAttach.name(), block_pos.to_centered_f64());
        } else if !attached && was_attached {
            let pitch = 1.2 / (world.rand_f32() * 0.2 + 0.9);
            world.play_sound_raw(Sound::BlockTripwireDetach as u16, cat, &pos, 0.4, pitch);
            world.emit_game_event(GameEvent::BlockDetach.name(), block_pos.to_centered_f64());
        }
    }

    pub fn update_neighbors_on_axis(
        block: &Block,
        world: &Arc<World>,
        block_pos: BlockPos,
        direction: BlockDirection,
    ) {
        world.update_neighbors_at(&block_pos, block, None);
        world.update_neighbors_at(
            &block_pos.offset(direction.opposite().to_offset()),
            block,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::HorizontalFacing;

    #[test]
    fn test_tripwire_hook_power_output() {
        let block = &Block::TRIPWIRE_HOOK;
        let mut props = TripwireHookProperties::default(block);
        props.facing = HorizontalFacing::South;
        props.powered = false;

        let weak_power = |p: TripwireHookProperties| if p.powered { 15 } else { 0 };
        let strong_power = |p: TripwireHookProperties, dir: BlockDirection| {
            if p.powered && dir.to_horizontal_facing().is_some_and(|f| p.facing == f) {
                15
            } else {
                0
            }
        };

        // When unpowered: no weak, no strong
        assert_eq!(weak_power(props), 0);
        assert_eq!(strong_power(props, BlockDirection::South), 0);
        assert_eq!(strong_power(props, BlockDirection::North), 0);

        // When powered: weak power everywhere, strong power only in facing direction (South)
        props.powered = true;
        assert_eq!(weak_power(props), 15);
        assert_eq!(strong_power(props, BlockDirection::South), 15);
        assert_eq!(strong_power(props, BlockDirection::North), 0);
        assert_eq!(strong_power(props, BlockDirection::East), 0);
        assert_eq!(strong_power(props, BlockDirection::West), 0);
        assert_eq!(strong_power(props, BlockDirection::Up), 0);
        assert_eq!(strong_power(props, BlockDirection::Down), 0);
    }
}
