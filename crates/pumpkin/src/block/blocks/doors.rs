use pumpkin_data::block_properties::{Axis, DoorHinge, DoubleBlockHalf, HorizontalFacing};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_protocol::java::server::play::SUseItemOn;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::registry::{BlockActionResult, can_replace_with_other_block};
use crate::block::{
    BlockBehaviour, BrokenArgs, CanPlaceAtArgs, ExplodeArgs, GetStateForNeighborUpdateArgs,
    NormalUseArgs, OnNeighborUpdateArgs, OnPlaceArgs, PathComputationType, PlayerPlacedArgs,
};
use crate::entity::{EntityBase, player::Player};
use crate::world::World;

type DoorProperties = pumpkin_data::block_properties::OakDoorLikeProperties;

fn get_sound(block: &Block, open: bool) -> Sound {
    let (closed, opened) = if block == &Block::CHERRY_DOOR {
        (
            Sound::BlockCherryWoodDoorClose,
            Sound::BlockCherryWoodDoorOpen,
        )
    } else if block == &Block::BAMBOO_DOOR {
        (
            Sound::BlockBambooWoodDoorClose,
            Sound::BlockBambooWoodDoorOpen,
        )
    } else if block == &Block::CRIMSON_DOOR || block == &Block::WARPED_DOOR {
        (
            Sound::BlockNetherWoodDoorClose,
            Sound::BlockNetherWoodDoorOpen,
        )
    } else if block.has_tag(&tag::Block::MINECRAFT_WOODEN_DOORS) {
        (Sound::BlockWoodenDoorClose, Sound::BlockWoodenDoorOpen)
    } else if block == &Block::IRON_DOOR {
        (Sound::BlockIronDoorClose, Sound::BlockIronDoorOpen)
    } else {
        (Sound::BlockCopperDoorClose, Sound::BlockCopperDoorOpen)
    };
    if open { opened } else { closed }
}

fn play_sound(
    world: &Arc<World>,
    pos: &BlockPos,
    block: &Block,
    open: bool,
    player: Option<&Player>,
) {
    let sound = get_sound(block, open);
    let pitch = world.rand_f32() * 0.1 + 0.9;
    if let Some(player) = player {
        world.play_sound_raw_expect(
            player,
            sound as u16,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            pitch,
        );
    } else {
        world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            pitch,
        );
    }
    world.emit_game_event_from_entity(
        if open { "block_open" } else { "block_close" },
        pos.to_centered_f64(),
        player.map(|p| p as &dyn EntityBase),
        None,
    );
}

fn get_hinge(
    world: &World,
    pos: &BlockPos,
    use_item: &SUseItemOn,
    facing: HorizontalFacing,
) -> DoorHinge {
    let left = pos.offset(facing.rotate_counter_clockwise().to_offset());
    let right = pos.offset(facing.rotate_clockwise().to_offset());
    let left_state = world.get_block_state(&left);
    let right_state = world.get_block_state(&right);
    let left_above = world.get_block_state(&left.up());
    let right_above = world.get_block_state(&right.up());
    let door_left = left_state
        .id
        .to_block()
        .has_tag(&tag::Block::MINECRAFT_DOORS)
        && DoorProperties::from_state_id(left_state.id).half == DoubleBlockHalf::Lower;
    let door_right = right_state
        .id
        .to_block()
        .has_tag(&tag::Block::MINECRAFT_DOORS)
        && DoorProperties::from_state_id(right_state.id).half == DoubleBlockHalf::Lower;
    let score = -i32::from(left_state.is_full_cube()) - i32::from(left_above.is_full_cube())
        + i32::from(right_state.is_full_cube())
        + i32::from(right_above.is_full_cube());
    if (!door_left || door_right) && score <= 0 {
        if (!door_right || door_left) && score >= 0 {
            let step = facing.to_offset();
            let hit = use_item.cursor_pos;
            if (step.x >= 0 || !(hit.z < 0.5))
                && (step.x <= 0 || !(hit.z > 0.5))
                && (step.z >= 0 || !(hit.x > 0.5))
                && (step.z <= 0 || !(hit.x < 0.5))
            {
                DoorHinge::Left
            } else {
                DoorHinge::Right
            }
        } else {
            DoorHinge::Left
        }
    } else {
        DoorHinge::Right
    }
}

#[pumpkin_block_from_tag("minecraft:doors")]
pub struct DoorBlock;

impl DoorBlock {
    pub fn is_wooden_door(world: &World, block_pos: &BlockPos) -> bool {
        let block = world.get_block(block_pos);
        // Vanilla's historical name means any door that opens by hand, including copper.
        block.has_tag(&tag::Block::MINECRAFT_DOORS) && block != &Block::IRON_DOOR
    }
    pub fn is_open(world: &World, pos: &BlockPos) -> bool {
        let (block, state) = world.get_block_and_state_id(pos);
        block.has_tag(&tag::Block::MINECRAFT_DOORS) && DoorProperties::from_state_id(state).open
    }
    pub fn set_open(world: &Arc<World>, pos: &BlockPos, open: bool) {
        Self::set_open_by(world, pos, open, None);
    }
    fn set_open_by(world: &Arc<World>, pos: &BlockPos, open: bool, player: Option<&Player>) {
        let (block, state) = world.get_block_and_state_id(pos);
        if !block.has_tag(&tag::Block::MINECRAFT_DOORS) {
            return;
        }
        let mut props = DoorProperties::from_state_id(state);
        if props.open == open {
            return;
        }
        props.open = open;
        // Java's extra flag 8 requests immediate client rendering; it is not DROP suppression.
        world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
        play_sound(world, pos, block, open, player);
    }
}

impl BlockBehaviour for DoorBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let above = args.position.up();
        let (block_above, state_above) = args.world.get_block_and_state(&above);
        if !args.world.is_in_height_limit(above.0.y)
            || !can_replace_with_other_block(block_above, state_above)
        {
            return BlockStateId::AIR;
        }
        let mut props = DoorProperties::default(args.block);
        props.facing = args.player.get_entity().get_horizontal_facing();
        props.half = DoubleBlockHalf::Lower;
        props.hinge = get_hinge(args.world, args.position, args.use_item_on, props.facing);
        props.powered = block_receives_redstone_power(args.world, args.position)
            || block_receives_redstone_power(args.world, &above);
        props.open = props.powered;
        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let below = args.block_accessor.get_block_state(&args.position.down());
        if DoorProperties::from_state_id(args.state.id).half == DoubleBlockHalf::Lower {
            below.is_side_solid(BlockDirection::Up)
        } else {
            below.id.to_block() == args.block
        }
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        let mut props = DoorProperties::from_state_id(args.state_id);
        props.half = DoubleBlockHalf::Upper;
        args.world.set_block_state(
            &args.position.up(),
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if args.block == &Block::IRON_DOOR {
            return BlockActionResult::Pass;
        }
        let props = DoorProperties::from_state_id(args.world.get_block_state_id(args.position));
        Self::set_open_by(args.world, args.position, !props.open, Some(args.player));
        BlockActionResult::Success
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        let props = DoorProperties::from_state_id(args.state.id);
        if args.can_trigger_blocks
            && args.block != &Block::IRON_DOOR
            && props.half == DoubleBlockHalf::Lower
            && !props.powered
        {
            Self::set_open(args.world, args.position, !props.open);
        }
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        if (args.player.gamemode.load() == GameMode::Creative
            || !args
                .player
                .can_harvest(args.state, args.state.id.to_block()))
            && DoorProperties::from_state_id(args.state.id).half == DoubleBlockHalf::Upper
        {
            let bottom = args.position.down();
            let state = args.world.get_block_state_id(&bottom);
            if state.to_block() == args.block
                && DoorProperties::from_state_id(state).half == DoubleBlockHalf::Lower
            {
                args.world.set_block_state(
                    &bottom,
                    BlockStateId::AIR,
                    BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS,
                );
                let packet = pumpkin_protocol::java::client::play::CWorldEvent::new(
                    pumpkin_data::world::WorldEvent::ParticlesDestroyBlock as i32,
                    bottom,
                    i32::from(state.as_u16()),
                    false,
                );
                args.world.broadcast_to_chunk_except(
                    bottom.chunk_position(),
                    &[args.player.gameprofile.id],
                    &packet,
                );
            }
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let mut props = DoorProperties::from_state_id(args.world.get_block_state_id(args.position));
        let other = if props.half == DoubleBlockHalf::Lower {
            args.position.up()
        } else {
            args.position.down()
        };
        let powered = block_receives_redstone_power(args.world, args.position)
            || block_receives_redstone_power(args.world, &other);
        if args.source_block != args.block && powered != props.powered {
            if powered != props.open {
                play_sound(args.world, args.position, args.block, powered, None);
            }
            props.powered = powered;
            props.open = powered;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let half = DoorProperties::from_state_id(args.state_id).half;
        if args.direction.to_axis() != Axis::Y
            || (half == DoubleBlockHalf::Lower) != (args.direction == BlockDirection::Up)
        {
            if half == DoubleBlockHalf::Lower
                && args.direction == BlockDirection::Down
                && !has_support(args.world, args.position)
            {
                return BlockStateId::AIR;
            }
        } else if args
            .neighbor_state_id
            .to_block()
            .has_tag(&tag::Block::MINECRAFT_DOORS)
            && DoorProperties::from_state_id(args.neighbor_state_id).half != half
        {
            let mut props = DoorProperties::from_state_id(args.neighbor_state_id);
            props.half = half;
            return props.to_state_id(args.neighbor_state_id.to_block());
        } else {
            return BlockStateId::AIR;
        }
        args.state_id
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        match computation_type {
            PathComputationType::Land | PathComputationType::Air => {
                DoorProperties::from_state_id(state.id).open
            }
            PathComputationType::Water => false,
        }
    }
}

fn has_support(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    world
        .get_block_state(&pos.down())
        .is_side_solid(BlockDirection::Up)
}
