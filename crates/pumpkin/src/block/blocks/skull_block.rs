use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::entities::skull::SkullBlockEntity;
use crate::block::{
    BlockBehaviour, BlockIsReplacing, BlockMetadata, OnNeighborUpdateArgs, OnPlaceArgs,
    PathComputationType, PlacedArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::FacingExt;
use pumpkin_data::block_properties::{
    Facing, SkeletonSkullLikeProperties, SkeletonWallSkullLikeProperties,
};
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId};
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

pub struct SkullBlock;

impl BlockMetadata for SkullBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SKELETON_SKULL,
            BlockId::PLAYER_HEAD,
            BlockId::ZOMBIE_HEAD,
            BlockId::CREEPER_HEAD,
            BlockId::PIGLIN_HEAD,
            BlockId::DRAGON_HEAD,
            // Wall-mounted counterparts: Blocks.java registers each of these as the
            // `wallBlock` of a `StandingAndWallBlockItem` alongside the standing block
            // above (e.g. Blocks.java:2786-2833), so the same item places either one
            // depending on the clicked face -- handled in `on_place` below.
            BlockId::SKELETON_WALL_SKULL,
            BlockId::PLAYER_WALL_HEAD,
            BlockId::ZOMBIE_WALL_HEAD,
            BlockId::CREEPER_WALL_HEAD,
            BlockId::PIGLIN_WALL_HEAD,
            BlockId::DRAGON_WALL_HEAD,
        ]
        .into()
    }
}

/// Maps a standing skull/head block to its wall-mounted counterpart, mirroring the
/// `StandingAndWallBlockItem`/`PlayerHeadItem` pairings in `Items.java` (e.g. lines
/// 1519-1558): every skull item places the standing block when looking down at the top
/// of a block, and the wall block when placed against the side of one.
fn wall_variant(block: &Block) -> Option<&'static Block> {
    if block == &Block::SKELETON_SKULL {
        Some(&Block::SKELETON_WALL_SKULL)
    } else if block == &Block::WITHER_SKELETON_SKULL {
        Some(&Block::WITHER_SKELETON_WALL_SKULL)
    } else if block == &Block::PLAYER_HEAD {
        Some(&Block::PLAYER_WALL_HEAD)
    } else if block == &Block::ZOMBIE_HEAD {
        Some(&Block::ZOMBIE_WALL_HEAD)
    } else if block == &Block::CREEPER_HEAD {
        Some(&Block::CREEPER_WALL_HEAD)
    } else if block == &Block::PIGLIN_HEAD {
        Some(&Block::PIGLIN_WALL_HEAD)
    } else if block == &Block::DRAGON_HEAD {
        Some(&Block::DRAGON_WALL_HEAD)
    } else {
        None
    }
}

/// True for any of the seven wall-mounted skull/head blocks (`SkeletonWallSkullLikeProperties`
/// covers exactly this set of `BlockId`s in the generated properties table).
fn is_wall_variant(block: &Block) -> bool {
    matches!(
        block.id,
        BlockId::SKELETON_WALL_SKULL
            | BlockId::WITHER_SKELETON_WALL_SKULL
            | BlockId::ZOMBIE_WALL_HEAD
            | BlockId::PLAYER_WALL_HEAD
            | BlockId::CREEPER_WALL_HEAD
            | BlockId::DRAGON_WALL_HEAD
            | BlockId::PIGLIN_WALL_HEAD
    )
}

impl BlockBehaviour for SkullBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = SkullBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        // `StandingAndWallBlockItem.getPlacementState` (StandingAndWallBlockItem.java:27-45)
        // walks the same "nearest looking directions" used by every directional block
        // placement, with `attachmentDirection = Direction.DOWN` (Items.java:1519 etc.):
        // the very first candidate that isn't Up decides standing-vs-wall. `args.direction`
        // is already vanilla's `clickedFace.getOpposite()`, so it is moved to the front of
        // the order exactly like `getNearestLookingDirections` does (BlockPlaceContext.java:71-90),
        // unless we're replacing the clicked block itself (`replaceClicked`), matching the
        // `args.replacing == None` guard used for the same reorder in `TorchBlock`.
        let mut directions = args.player.get_entity().get_entity_facing_order();
        if args.replacing == BlockIsReplacing::None {
            let face = args.direction.to_facing();
            if let Some(i) = directions.iter().position(|d| *d == face)
                && i > 0
            {
                directions.copy_within(0..i, 1);
                directions[0] = face;
            }
        }

        let wall_block = wall_variant(args.block);
        let powered = block_receives_redstone_power(args.world, args.position);

        // `WallSkullBlock.getStateForPlacement` (WallSkullBlock.java:41-58) scans the same
        // ordered directions for the first horizontal one whose neighbor in that direction
        // is not replaceable, i.e. solid enough to hang the skull from; `facing` is set
        // away from that neighbor.
        let wall_facing = wall_block.and_then(|_| {
            directions.iter().find_map(|dir| {
                let horizontal = dir.to_horizontal_facing()?;
                let neighbor_pos = args.position.offset(horizontal.to_offset());
                if args.world.get_block_state(&neighbor_pos).replaceable() {
                    None
                } else {
                    Some(horizontal.opposite())
                }
            })
        });

        for dir in directions {
            match dir {
                // `StandingAndWallBlockItem.java:35` skips `attachmentDirection.getOpposite()`
                // (Up) entirely -- there is no ceiling-mounted skull variant.
                Facing::Up => continue,
                Facing::Down => {
                    // Standing skull: `SkullBlock.getStateForPlacement` (SkullBlock.java:47-49).
                    let mut props = SkeletonSkullLikeProperties::default(args.block);
                    props.rotation = args.player.get_entity().get_rotation_16();
                    props.powered = powered;
                    return props.to_state_id(args.block);
                }
                _ => {
                    if let (Some(wall_block), Some(facing)) = (wall_block, wall_facing) {
                        let mut props = SkeletonWallSkullLikeProperties::default(wall_block);
                        props.facing = facing;
                        props.powered = powered;
                        return props.to_state_id(wall_block);
                    }
                    // No solid neighbor found for any horizontal candidate: keep scanning
                    // (vanilla would too, since the precomputed `wallState` never changes),
                    // which always terminates at `Facing::Down` below.
                }
            }
        }

        // Down is always one of the six candidate directions and always succeeds --
        // `AbstractSkullBlock` never overrides `canSurvive` (default `true`,
        // BlockBehaviour.java:311-313), so a skull can even float with no support. This
        // arm only exists to satisfy the compiler; it mirrors the Down case above.
        let mut props = SkeletonSkullLikeProperties::default(args.block);
        props.rotation = args.player.get_entity().get_rotation_16();
        props.powered = powered;
        props.to_state_id(args.block)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        // `AbstractSkullBlock.neighborChanged` (AbstractSkullBlock.java:73-83) keeps the
        // `powered` property in sync with redstone signal and is inherited unmodified by
        // `WallSkullBlock`, so the same logic applies to standing and wall skulls alike.
        let state = args.world.get_block_state(args.position);
        let is_receiving_power = block_receives_redstone_power(args.world, args.position);

        let new_state_id = if is_wall_variant(args.block) {
            let mut props = SkeletonWallSkullLikeProperties::from_state_id(state.id);
            if props.powered == is_receiving_power {
                return;
            }
            props.powered = is_receiving_power;
            props.to_state_id(args.block)
        } else {
            let mut props = SkeletonSkullLikeProperties::from_state_id(state.id);
            if props.powered == is_receiving_power {
                return;
            }
            props.powered = is_receiving_power;
            props.to_state_id(args.block)
        };

        args.world.set_block_state(
            args.position,
            new_state_id,
            BlockFlags::NOTIFY_LISTENERS,
        );
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
