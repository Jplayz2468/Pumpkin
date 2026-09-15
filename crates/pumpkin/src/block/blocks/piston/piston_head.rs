use pumpkin_data::block_properties::PistonType;
use pumpkin_data::{Block, BlockState, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnNeighborUpdateArgs, PathComputationType,
};

use super::piston::PistonProps;
use super::piston_extension::MovingPistonProps;

pub(crate) type PistonHeadProperties = pumpkin_data::block_properties::PistonHeadLikeProperties;

/// Vanilla `PistonHeadBlock.isFittingBase` (`PistonHeadBlock.java:64`): the arm belongs to
/// the base behind it only if the base is the matching piston kind, is currently extended,
/// and faces the same way. All three checks matter -- without the type/facing checks a head
/// will happily adopt (and later destroy) a completely unrelated piston next to it.
fn is_fitting_base(
    head: PistonHeadProperties,
    base_block: &Block,
    base_state_id: BlockStateId,
) -> bool {
    let expected_base = if head.r#type == PistonType::Normal {
        &Block::PISTON
    } else {
        &Block::STICKY_PISTON
    };
    if base_block != expected_base {
        return false;
    }
    let base = PistonProps::from_state_id(base_state_id);
    base.extended && base.facing == head.facing
}

/// The position of the piston base this arm is attached to.
fn base_pos(position: &BlockPos, head: PistonHeadProperties) -> BlockPos {
    position.offset(head.facing.opposite().to_block_direction().to_offset())
}

/// Vanilla `PistonHeadBlock.canSurvive` (`PistonHeadBlock.java:106`). The second arm of the
/// disjunction is what keeps the head alive during a retraction: while the base is animating
/// it is a `moving_piston`, not a piston, and the head must not delete itself mid-cycle.
fn can_survive(
    world: &dyn BlockAccessor,
    position: &BlockPos,
    head_state_id: BlockStateId,
) -> bool {
    let head = PistonHeadProperties::from_state_id(head_state_id);
    let pos = base_pos(position, head);
    let (base_block, base_state) = world.get_block_and_state(&pos);
    if is_fitting_base(head, base_block, base_state.id) {
        return true;
    }
    base_block == &Block::MOVING_PISTON
        && MovingPistonProps::from_state_id(base_state.id).facing == head.facing
}

#[pumpkin_block("minecraft:piston_head")]
pub struct PistonHeadBlock;

impl BlockBehaviour for PistonHeadBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        // Vanilla `PistonHeadBlock.affectNeighborsAfterRemoval` (`PistonHeadBlock.java:82`):
        // breaking the arm takes the base with it, but only when the base is genuinely this
        // arm's base. This used to accept any extended piston behind the head regardless of
        // kind or facing, so breaking a head could destroy a neighbouring piston.
        let head = PistonHeadProperties::from_state_id(args.state.id);
        let pos = base_pos(args.position, head);
        let (base_block, base_state_id) = args.world.get_block_and_state_id(&pos);
        if is_fitting_base(head, base_block, base_state_id) {
            // TODO: use player; vanilla drops the piston here (`destroyBlock(basePos, true)`)
            // and only skips drops for a creative-mode break (`playerWillDestroy`).
            args.world.break_block(&pos, None, BlockFlags::SKIP_DROPS);
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_survive(args.block_accessor, args.position, args.state.id)
    }

    /// Vanilla `PistonHeadBlock.updateShape` (`PistonHeadBlock.java:100`): when the neighbour
    /// on the base side changes and the arm can no longer survive, the arm becomes air.
    ///
    /// This is the *only* thing in vanilla that cleans up an orphaned `piston_head`. Pumpkin
    /// relied purely on `PistonBlock::broken` explicitly deleting the head, so any other way
    /// of losing the base -- an explosion, a fill/setblock, a push that replaced it -- left a
    /// head standing in the world forever.
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let head = PistonHeadProperties::from_state_id(args.state_id);
        if args.direction.opposite().to_facing() == head.facing
            && !can_survive(args.world, args.position, args.state_id)
        {
            return BlockStateId::AIR;
        }
        args.state_id
    }

    /// Vanilla `PistonHeadBlock.neighborChanged` (`PistonHeadBlock.java:112`): the arm relays
    /// every neighbour change back to its base.
    ///
    /// This is how a piston learns about redstone it can only "see" through quasi-connectivity
    /// -- e.g. a block above the arm, which is adjacent to `pos.up()` (and so counts for
    /// `PistonBaseBlock.getNeighborSignal`, `PistonBaseBlock.java:138`) but is two blocks away
    /// from the base and therefore never notifies it directly. Without the relay the base never
    /// re-runs `checkIfExtend` when that block is removed, so the piston stays extended and its
    /// head is left standing.
    ///
    /// The previous implementation only relayed for upward-facing pistons and only when the
    /// block above was not a redstone block -- a special case of this rule.
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let head_state_id = args.world.get_block_state_id(args.position);
        if !can_survive(args.world.as_ref(), args.position, head_state_id) {
            return;
        }
        let head = PistonHeadProperties::from_state_id(head_state_id);
        args.world
            .update_neighbor(&base_pos(args.position, head), args.source_block);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::Facing;

    fn head(facing: Facing, kind: PistonType) -> PistonHeadProperties {
        let mut props = PistonHeadProperties::default(&Block::PISTON_HEAD);
        props.facing = facing;
        props.r#type = kind;
        props
    }

    fn base(block: &Block, facing: Facing, extended: bool) -> BlockStateId {
        let mut props = PistonProps::default(block);
        props.facing = facing;
        props.extended = extended;
        props.to_state_id(block)
    }

    /// `is_fitting_base` is what both `can_survive` and `broken` key off, so every one of
    /// vanilla's three conditions (`PistonHeadBlock.java:64`) has to be enforced.
    #[test]
    fn fitting_base_requires_kind_extension_and_facing() {
        let normal_head = head(Facing::North, PistonType::Normal);
        assert!(is_fitting_base(
            normal_head,
            &Block::PISTON,
            base(&Block::PISTON, Facing::North, true)
        ));

        // A retracted base is not a fitting base.
        assert!(!is_fitting_base(
            normal_head,
            &Block::PISTON,
            base(&Block::PISTON, Facing::North, false)
        ));

        // Facing must match -- this is the case that used to destroy an unrelated piston.
        assert!(!is_fitting_base(
            normal_head,
            &Block::PISTON,
            base(&Block::PISTON, Facing::East, true)
        ));

        // A plain arm does not belong to a sticky base, and vice versa.
        assert!(!is_fitting_base(
            normal_head,
            &Block::STICKY_PISTON,
            base(&Block::STICKY_PISTON, Facing::North, true)
        ));
        let sticky_head = head(Facing::North, PistonType::Sticky);
        assert!(!is_fitting_base(
            sticky_head,
            &Block::PISTON,
            base(&Block::PISTON, Facing::North, true)
        ));
        assert!(is_fitting_base(
            sticky_head,
            &Block::STICKY_PISTON,
            base(&Block::STICKY_PISTON, Facing::North, true)
        ));
    }

    /// `base_pos` must look *backwards* along the arm. Getting this inverted would make the
    /// head check the block it is pushing instead of the piston holding it.
    #[test]
    fn base_is_behind_the_arm() {
        let pos = BlockPos::new(0, 64, 0);
        assert_eq!(
            base_pos(&pos, head(Facing::North, PistonType::Normal)),
            BlockPos::new(0, 64, 1)
        );
        assert_eq!(
            base_pos(&pos, head(Facing::Up, PistonType::Normal)),
            BlockPos::new(0, 63, 0)
        );
    }
}
