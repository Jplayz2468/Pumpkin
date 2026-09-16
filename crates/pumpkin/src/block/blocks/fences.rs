use super::is_exception_for_connection;
use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, NormalUseArgs, OnPlaceArgs, PathComputationType,
};
use crate::world::World;
use pumpkin_data::block_properties::HorizontalFacing;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, HorizontalFacingExt, tag};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;

type FenceGateProperties = pumpkin_data::block_properties::OakFenceGateLikeProperties;
type FenceProperties = pumpkin_data::block_properties::OakFenceLikeProperties;

#[pumpkin_block_from_tag("minecraft:fences")]
pub struct FenceBlock;

impl BlockBehaviour for FenceBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> crate::block::registry::BlockActionResult {
        crate::item::items::lead::LeadItem::bind_player_mobs(args.player, *args.position)
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut fence_props = FenceProperties::default(args.block);
        fence_props.waterlogged = args.replacing.water_source();

        compute_fence_state(fence_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = FenceProperties::from_state_id(args.state_id);
        super::schedule_waterlogged_tick(args.world, args.position, props.waterlogged);
        if args.direction.is_horizontal() {
            let connected = FenceProperties::from_state_id(compute_fence_state(
                props,
                args.world,
                args.block,
                args.position,
            ));
            match args.direction {
                BlockDirection::North => props.north = connected.north,
                BlockDirection::East => props.east = connected.east,
                BlockDirection::South => props.south = connected.south,
                BlockDirection::West => props.west = connected.west,
                _ => {}
            }
        }
        props.to_state_id(args.block)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

pub fn compute_fence_state(
    mut fence_props: FenceProperties,
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
) -> BlockStateId {
    for direction in BlockDirection::horizontal() {
        let other_block_pos = block_pos.offset(direction.to_offset());
        let (other_block, other_block_state) = world.get_block_and_state(&other_block_pos);

        let connected = connects_to(
            block,
            other_block,
            other_block_state,
            direction.to_block_direction(),
        );
        match direction {
            HorizontalFacing::North => fence_props.north = connected,
            HorizontalFacing::South => fence_props.south = connected,
            HorizontalFacing::West => fence_props.west = connected,
            HorizontalFacing::East => fence_props.east = connected,
        }
    }

    fence_props.to_state_id(block)
}

fn connects_to(from: &Block, to: &Block, to_state: &BlockState, direction: BlockDirection) -> bool {
    if from == to {
        return true;
    }

    // Vanilla: FenceBlock.connectsTo (FenceBlock.java:59-64) is
    // `!isExceptionForConnection(state) && faceSolid || sameFence || gate`. Without the
    // exception check a fence would connect to pumpkins, melons, leaves, barriers and
    // shulker boxes just because those happen to have a sturdy face on that side.
    if !is_exception_for_connection(to) && to_state.is_side_solid(direction.opposite()) {
        return true;
    }

    if to.has_tag(&tag::Block::C_FENCE_GATES) {
        let fence_gate_props = FenceGateProperties::from_state_id(to_state.id);
        if BlockDirection::from_cardinal_direction(fence_gate_props.facing).to_axis()
            == direction.rotate_clockwise().to_axis()
        {
            return true;
        }
    }

    to.has_tag(&tag::Block::MINECRAFT_FENCES)
        && from.has_tag(&tag::Block::MINECRAFT_WOODEN_FENCES)
            == to.has_tag(&tag::Block::MINECRAFT_WOODEN_FENCES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wooden fences connect to each other regardless of face sturdiness
    /// (`isSameFence`, FenceBlock.java:66-68).
    #[test]
    fn wooden_fences_connect_to_each_other() {
        assert!(connects_to(
            &Block::OAK_FENCE,
            &Block::SPRUCE_FENCE,
            Block::SPRUCE_FENCE.default_state,
            BlockDirection::North,
        ));
        assert!(connects_to(
            &Block::OAK_FENCE,
            &Block::OAK_FENCE,
            Block::OAK_FENCE.default_state,
            BlockDirection::North,
        ));
    }

    /// Nether brick fence is not a wooden fence, but two of them still connect to each
    /// other (`isSameFence`: both `is(WOODEN_FENCES)` are `false`, so they're equal).
    #[test]
    fn nether_brick_fence_connects_to_itself_but_not_wooden_fences() {
        assert!(connects_to(
            &Block::NETHER_BRICK_FENCE,
            &Block::NETHER_BRICK_FENCE,
            Block::NETHER_BRICK_FENCE.default_state,
            BlockDirection::North,
        ));
        assert!(!connects_to(
            &Block::NETHER_BRICK_FENCE,
            &Block::OAK_FENCE,
            Block::OAK_FENCE.default_state,
            BlockDirection::North,
        ));
    }

    /// A plain solid block (stone) has a sturdy face and is not in the exception list,
    /// so `faceSolid` alone is enough to connect (FenceBlock.java:63).
    #[test]
    fn fence_connects_to_a_solid_block() {
        assert!(connects_to(
            &Block::OAK_FENCE,
            &Block::STONE,
            Block::STONE.default_state,
            BlockDirection::North,
        ));
    }

    /// Fence gates facing perpendicular to the fence connect to it even though a gate
    /// does not pass the sturdy-face check (FenceBlock.java:62, FenceGateBlock.connectsToDirection).
    #[test]
    fn fence_connects_to_perpendicular_fence_gate() {
        let mut gate_props = FenceGateProperties::default(&Block::OAK_FENCE_GATE);
        gate_props.facing = HorizontalFacing::East;
        let state_id = gate_props.to_state_id(&Block::OAK_FENCE_GATE);
        let gate_state = BlockState::from_id(state_id);

        assert!(connects_to(
            &Block::OAK_FENCE,
            &Block::OAK_FENCE_GATE,
            gate_state,
            BlockDirection::North,
        ));
    }

    /// Air is neither solid nor a fence/gate, so it doesn't connect.
    #[test]
    fn fence_does_not_connect_to_air() {
        assert!(!connects_to(
            &Block::OAK_FENCE,
            &Block::AIR,
            Block::AIR.default_state,
            BlockDirection::North,
        ));
    }

    /// `Block.isExceptionForConnection` (Block.java:251-259) explicitly excludes leaves,
    /// pumpkins, melons, barriers and shulker boxes even though several of them have a
    /// sturdy face on every side. This is the bug this change fixes: previously a fence
    /// would connect to all of these because it only checked `is_side_solid`.
    #[test]
    fn fence_does_not_connect_to_exception_blocks_despite_sturdy_faces() {
        for exception in [
            &Block::OAK_LEAVES,
            &Block::PUMPKIN,
            &Block::CARVED_PUMPKIN,
            &Block::JACK_O_LANTERN,
            &Block::MELON,
            &Block::BARRIER,
            &Block::SHULKER_BOX,
        ] {
            assert!(
                !connects_to(
                    &Block::OAK_FENCE,
                    exception,
                    exception.default_state,
                    BlockDirection::North,
                ),
                "fence should not connect to {}",
                exception.name
            );
        }
    }
}
