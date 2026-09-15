use rustc_hash::FxHashSet;

use super::spreader::{bonemeal_lichen, can_attach, can_bonemeal_lichen};
use crate::block::{
    BlockBehaviour, BlockIsReplacing, BlockMetadata, BonemealArgs, CanPlaceAtArgs, CanUpdateAtArgs,
    GetStateForNeighborUpdateArgs, OnPlaceArgs,
};
use crate::entity::{EntityBase, player::Player};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId, FacingExt,
    block_properties::GlowLichenLikeProperties,
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;

pub struct MultifaceBlock;

impl BlockMetadata for MultifaceBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SCULK_VEIN,
            BlockId::GLOW_LICHEN,
            BlockId::RESIN_CLUMP,
        ]
        .into()
    }
}

impl BlockBehaviour for MultifaceBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let (Some(direction), _) = get_attach_direction(
                args.world,
                args.position,
                args.block,
                Some(args.player),
                args.direction,
                args.use_item_on.position == *args.position,
            ) else {
                return Block::AIR.default_state.id;
            };
            let mut props = GlowLichenLikeProperties::from_state_id(state_id);
            set_face(&mut props, direction);
            return props.to_state_id(args.block);
        }
        let (Some(direction), _) = get_attach_direction(
            args.world,
            args.position,
            args.block,
            Some(args.player),
            args.direction,
            args.use_item_on.position == *args.position,
        ) else {
            return Block::AIR.default_state.id;
        };
        let mut props = GlowLichenLikeProperties::default(args.block);
        set_face(&mut props, direction);
        let (fluid, fluid_state) = crate::world::World::fluid_state_from_block_state(
            args.world.get_block_state_id(args.position),
        );
        props.waterlogged = fluid.matches_type(&Fluid::WATER) && fluid_state.is_source;
        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        if let Some(packet) = args.use_item_on {
            get_attach_direction(
                args.block_accessor,
                args.position,
                args.block,
                args.player,
                args.direction.unwrap_or(BlockDirection::Down),
                packet.position == *args.position,
            )
            .0
            .is_some()
        } else {
            let faces = active_directions(GlowLichenLikeProperties::from_state_id(args.state.id));
            !faces.is_empty()
                && faces
                    .into_iter()
                    .all(|dir| can_attach(args.block_accessor, *args.position, dir))
        }
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        // MultifaceBlock.canBeReplaced only checks for a vacant face. Placement
        // then selects a supported face in the player's nearest-looking order.
        active_directions(GlowLichenLikeProperties::from_state_id(args.state_id)).len() < 6
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let old_props = GlowLichenLikeProperties::from_state_id(args.state_id);
        if old_props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }

        let mut new_directions = active_directions(old_props);
        if !can_attach(args.world.as_ref(), *args.position, args.direction) {
            new_directions.remove(&args.direction);
        }

        if new_directions.is_empty() {
            return Block::AIR.default_state.id;
        }
        let mut new_props = GlowLichenLikeProperties::default(args.block);
        for dir in new_directions {
            set_face(&mut new_props, dir);
        }
        new_props.waterlogged = old_props.waterlogged;
        new_props.to_state_id(args.block)
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        args.block.id == BlockId::GLOW_LICHEN
            && can_bonemeal_lichen(args.world, *args.position, args.state_id.to_state())
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if args.block.id == BlockId::GLOW_LICHEN {
            bonemeal_lichen(args.world, *args.position, args.state_id.to_state());
        }
    }
}

fn get_attach_direction(
    block_accessor: &dyn BlockAccessor,
    block_pos: &BlockPos,
    target_block: &Block,
    player_wrapper: Option<&Player>,
    click_direction: BlockDirection,
    replacing_clicked: bool,
) -> (Option<BlockDirection>, bool) {
    let (old_block, old_state) = block_accessor.get_block_and_state(block_pos);
    let active = if old_block == target_block {
        active_directions(GlowLichenLikeProperties::from_state_id(old_state.id))
    } else {
        FxHashSet::default()
    };
    let mut directions = player_wrapper.map_or(BlockDirection::all(), |player| {
        player
            .get_entity()
            .get_entity_facing_order()
            .map(|facing| facing.to_block_direction())
    });
    if !replacing_clicked
        && let Some(index) = directions.iter().position(|dir| *dir == click_direction)
    {
        directions[..=index].rotate_right(1);
    }
    (
        directions
            .into_iter()
            .find(|dir| !active.contains(dir) && can_attach(block_accessor, *block_pos, *dir)),
        false,
    )
}

fn active_directions(props: GlowLichenLikeProperties) -> FxHashSet<BlockDirection> {
    let mut set = FxHashSet::default();
    if props.down {
        set.insert(BlockDirection::Down);
    }
    if props.up {
        set.insert(BlockDirection::Up);
    }
    if props.north {
        set.insert(BlockDirection::North);
    }
    if props.south {
        set.insert(BlockDirection::South);
    }
    if props.east {
        set.insert(BlockDirection::East);
    }
    if props.west {
        set.insert(BlockDirection::West);
    }
    set
}

const fn set_face(props: &mut GlowLichenLikeProperties, direction: BlockDirection) {
    match direction {
        BlockDirection::Down => props.down = true,
        BlockDirection::Up => props.up = true,
        BlockDirection::North => props.north = true,
        BlockDirection::South => props.south = true,
        BlockDirection::West => props.west = true,
        BlockDirection::East => props.east = true,
    }
}
