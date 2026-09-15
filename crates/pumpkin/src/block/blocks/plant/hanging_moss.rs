use pumpkin_data::block_properties::PaleHangingMossLikeProperties;
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnScheduledTickArgs,
};

/// `HangingMossBlock` in vanilla (`net.minecraft.world.level.block.HangingMossBlock`).
#[pumpkin_block("minecraft:pale_hanging_moss")]
pub struct HangingMossBlock;

impl HangingMossBlock {
    /// `HangingMossBlock.canStayAtPosition`: the block above must offer a full down-facing
    /// face (`MultifaceBlock.canAttachTo`), or itself be hanging moss so chains can dangle.
    #[must_use]
    pub fn can_stay_at_position(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let above_pos = pos.up();
        let (above_block, above_state) = world.get_block_and_state(&above_pos);
        above_state.is_side_solid(BlockDirection::Down)
            || above_block == &Block::PALE_HANGING_MOSS
    }

    /// `HangingMossBlock.getTip`: walks down through the contiguous chain of hanging moss
    /// starting at `pos` and returns the position of the lowest block still in the chain.
    #[must_use]
    pub fn get_tip(world: &dyn BlockAccessor, pos: &BlockPos) -> BlockPos {
        let mut forward_pos = *pos;
        loop {
            forward_pos = forward_pos.down();
            if world.get_block(&forward_pos) != &Block::PALE_HANGING_MOSS {
                break;
            }
        }
        forward_pos.up()
    }
}

impl BlockBehaviour for HangingMossBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_stay_at_position(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !Self::can_stay_at_position(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }

        let below_block = args.world.get_block(&args.position.down());
        let mut props = PaleHangingMossLikeProperties::from_state_id(args.state_id);
        props.tip = below_block != args.block;
        props.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !Self::can_stay_at_position(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::empty());
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let grow_pos = Self::get_tip(args.world.as_ref(), args.position).down();
        let grow_state = args.world.get_block_state(&grow_pos);
        grow_state.is_air()
            && grow_pos.0.y >= args.world.dimension.min_y
            && grow_pos.0.y < args.world.dimension.min_y + args.world.dimension.height
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let grow_pos = Self::get_tip(args.world.as_ref(), args.position).down();
        if args.world.get_block_state(&grow_pos).is_air() {
            let mut props = PaleHangingMossLikeProperties::default(args.block);
            props.tip = true;
            args.world.set_block_state(
                &grow_pos,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
