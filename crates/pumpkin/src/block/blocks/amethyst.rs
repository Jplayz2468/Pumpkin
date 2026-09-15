use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId, FacingExt,
    block_properties::AmethystClusterLikeProperties,
    fluid::Fluid,
    sound::{Sound, SoundCategory},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use rand::RngExt;

use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnProjectileHitArgs, RandomTickArgs, blocks::abstract_wall_mounting::WallMountedBlock,
};

const ALL_DIRECTIONS: [BlockDirection; 6] = [
    BlockDirection::Down,
    BlockDirection::Up,
    BlockDirection::North,
    BlockDirection::South,
    BlockDirection::West,
    BlockDirection::East,
];

pub struct AmethystBlock;

impl BlockMetadata for AmethystBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SMALL_AMETHYST_BUD,
            BlockId::MEDIUM_AMETHYST_BUD,
            BlockId::LARGE_AMETHYST_BUD,
            BlockId::AMETHYST_CLUSTER,
        ]
        .into()
    }
}

impl BlockBehaviour for AmethystBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = AmethystClusterLikeProperties::from_state_id(args.block.default_state.id);
        // Vanilla stores the clicked face directly as FACING (AmethystClusterBlock.java:86:
        // `.setValue(FACING, context.getClickedFace())`), i.e. the direction pointing AWAY from
        // the support block. `args.direction` here is the direction from the new position
        // TOWARD the block that was clicked/is supporting it, so it must be flipped to match;
        // storing it unflipped renders the cluster/bud model upside-down relative to its support
        // and desyncs the FACING value from the identical convention used by
        // `BuddingAmethystBlock::random_tick` below, which grows buds with
        // `facing = grow_direction` (support -> bud, i.e. also "away from surface").
        props.facing = args.direction.opposite().to_facing();
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // Use the provided direction, or fallback to the current state's direction if missing
        let direction = args
            .direction
            .unwrap_or_else(|| self.get_direction(args.state.id, args.block));

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, direction)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        // Vanilla reschedules a water tick on every neighbor update, independent of which
        // neighbor changed or whether the block survives (AmethystClusterBlock.java:72-74).
        // Skipping this would leave a waterlogged cluster/bud's water permanently stale after,
        // e.g., a neighboring block is placed/removed.
        if AmethystClusterLikeProperties::from_state_id(args.state_id).waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }

        WallMountedBlock::get_state_for_neighbor_update(self, args)
    }
}

impl WallMountedBlock for AmethystBlock {
    fn get_direction(&self, state_id: BlockStateId, _block: &Block) -> BlockDirection {
        // `WallMountedBlock::get_state_for_neighbor_update`'s default impl compares
        // `get_direction().opposite()` against the direction of the neighbor that changed, i.e.
        // it expects this to return the "away from surface" direction (FACING itself), the same
        // convention LeverBlock/GrindstoneBlock use (Floor -> Up, not Up -> Down). Returning
        // FACING's opposite here (as before) canceled out the inverted assignment that used to
        // live in `on_place`, but only for that one call path; now that `on_place` stores FACING
        // the way vanilla does (AmethystClusterBlock.java:86), this must match it directly.
        let props = AmethystClusterLikeProperties::from_state_id(state_id);
        props.facing.to_block_direction()
    }
}

#[pumpkin_block("minecraft:budding_amethyst")]
pub struct BuddingAmethystBlock;

impl BlockBehaviour for BuddingAmethystBlock {
    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        if args.rand_bounded_i32(5) == 0 {
            let dir_index = args.rand_bounded_i32(ALL_DIRECTIONS.len() as i32) as usize;
            let grow_direction = ALL_DIRECTIONS[dir_index];
            let grow_pos = args.position.offset(grow_direction.to_offset());
            let (relative_block, relative_state) = args.world.get_block_and_state(&grow_pos);
            let relative_state_id = relative_state.id;

            let next_stage_and_water =
                if can_cluster_grow_at_state(relative_block, relative_state_id) {
                    Some((&Block::SMALL_AMETHYST_BUD, relative_block == &Block::WATER))
                } else if relative_block == &Block::SMALL_AMETHYST_BUD {
                    let props = AmethystClusterLikeProperties::from_state_id(relative_state_id);
                    (props.facing == grow_direction.to_facing())
                        .then_some((&Block::MEDIUM_AMETHYST_BUD, props.waterlogged))
                } else if relative_block == &Block::MEDIUM_AMETHYST_BUD {
                    let props = AmethystClusterLikeProperties::from_state_id(relative_state_id);
                    (props.facing == grow_direction.to_facing())
                        .then_some((&Block::LARGE_AMETHYST_BUD, props.waterlogged))
                } else if relative_block == &Block::LARGE_AMETHYST_BUD {
                    let props = AmethystClusterLikeProperties::from_state_id(relative_state_id);
                    (props.facing == grow_direction.to_facing())
                        .then_some((&Block::AMETHYST_CLUSTER, props.waterlogged))
                } else {
                    None
                };

            if let Some((next_stage, waterlogged)) = next_stage_and_water {
                let mut target_props = AmethystClusterLikeProperties::default(next_stage);
                target_props.facing = grow_direction.to_facing();
                target_props.waterlogged = waterlogged;
                let target_state_id = target_props.to_state_id(next_stage);
                args.world
                    .set_block_state(&grow_pos, target_state_id, BlockFlags::NOTIFY_ALL);
            }
        }
    }
}

#[must_use]
pub fn can_cluster_grow_at_state(block: &Block, state_id: BlockStateId) -> bool {
    block.default_state.is_air()
        || (block == &Block::WATER && state_id == Block::WATER.default_state.id)
}

/// The solid `minecraft:amethyst_block`, distinct from the bud/cluster growth stages above.
#[pumpkin_block("minecraft:amethyst_block")]
pub struct AmethystBlockBlock;

impl BlockBehaviour for AmethystBlockBlock {
    // AmethystBlock.java: onProjectileHit plays a chime whenever a projectile strikes the block.
    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        args.world.play_sound_fine(
            Sound::BlockAmethystBlockChime,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            1.0,
            0.5 + rand::rng().random::<f32>() * 1.2,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::BlockState;
    use pumpkin_data::block_properties::Facing;
    use pumpkin_util::math::position::BlockPos;
    use pumpkin_world::world::BlockAccessor;

    /// A `BlockAccessor` that reports one fixed position as solid stone and everything else
    /// as air, so `can_place_at`'s support search can be pointed at (or away from) a real
    /// solid block without needing a full `World`.
    struct FakeAccessor {
        solid_pos: BlockPos,
    }

    impl BlockAccessor for FakeAccessor {
        fn get_block(&self, position: &BlockPos) -> &'static Block {
            if *position == self.solid_pos {
                &Block::STONE
            } else {
                &Block::AIR
            }
        }

        fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
            if *position == self.solid_pos {
                Block::STONE.default_state
            } else {
                Block::AIR.default_state
            }
        }

        fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
            self.get_block_state(position).id
        }

        fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState) {
            (self.get_block(position), self.get_block_state(position))
        }
    }

    /// `get_direction` feeds `WallMountedBlock`'s default `get_state_for_neighbor_update`,
    /// which expects the "away from surface" convention (matching `LeverBlock`/
    /// `GrindstoneBlock`'s `Floor -> Up`), i.e. the same direction as FACING itself. This
    /// guards against reintroducing the `.opposite()` that used to be here, which only looked
    /// right because it canceled out an equal-and-opposite bug in `on_place` (see below).
    #[test]
    fn get_direction_matches_facing_directly() {
        for &(facing, block_direction) in &[
            (Facing::Up, BlockDirection::Up),
            (Facing::Down, BlockDirection::Down),
            (Facing::North, BlockDirection::North),
            (Facing::South, BlockDirection::South),
            (Facing::East, BlockDirection::East),
            (Facing::West, BlockDirection::West),
        ] {
            let mut props = AmethystClusterLikeProperties::default(&Block::AMETHYST_CLUSTER);
            props.facing = facing;
            let state_id = props.to_state_id(&Block::AMETHYST_CLUSTER);

            assert_eq!(
                AmethystBlock.get_direction(state_id, &Block::AMETHYST_CLUSTER),
                block_direction,
                "facing {facing:?} should map to direction {block_direction:?}"
            );
        }
    }

    /// Exercises the real `can_place_at` (`AmethystClusterBlock.java:58-62`: canSurvive checks
    /// that the block in the direction opposite FACING has a sturdy face towards FACING) the
    /// same way `BlockRegistry::place_block` actually calls it: with `direction` set to the
    /// direction from the new position towards the clicked/support block (i.e. `FACING`'s
    /// opposite, "support-ward"), not `None`.
    #[test]
    fn can_place_at_requires_sturdy_support_in_facing_direction() {
        let pos = BlockPos::new(0, 1, 0);

        // A cluster placed on a floor (FACING = Up) needs solid ground directly below it.
        let mut props = AmethystClusterLikeProperties::default(&Block::AMETHYST_CLUSTER);
        props.facing = Facing::Up;
        let state_id = props.to_state_id(&Block::AMETHYST_CLUSTER);
        let state = BlockState::from_id(state_id);

        let below = BlockPos::new(0, 0, 0);
        let accessor_supported = FakeAccessor { solid_pos: below };
        assert!(
            BlockBehaviour::can_place_at(&AmethystBlock, CanPlaceAtArgs {
                server: None,
                world: None,
                block_accessor: &accessor_supported,
                block: &Block::AMETHYST_CLUSTER,
                state,
                position: &pos,
                direction: Some(BlockDirection::Down),
                player: None,
                use_item_on: None,
            }),
            "stone directly below an up-facing cluster must count as support"
        );

        // Solid stone in the wrong place (above, instead of below) must NOT count as support --
        // this is exactly the case the original inverted-direction bug would have gotten wrong.
        let above = BlockPos::new(0, 2, 0);
        let accessor_unsupported = FakeAccessor { solid_pos: above };
        assert!(
            !BlockBehaviour::can_place_at(&AmethystBlock, CanPlaceAtArgs {
                server: None,
                world: None,
                block_accessor: &accessor_unsupported,
                block: &Block::AMETHYST_CLUSTER,
                state,
                position: &pos,
                direction: Some(BlockDirection::Down),
                player: None,
                use_item_on: None,
            }),
            "stone above an up-facing cluster is not its support and must not satisfy canSurvive"
        );
    }

    /// `can_cluster_grow_at_state` (`BuddingAmethystBlock.canClusterGrowAtState`,
    /// `BuddingAmethystBlock.java:44-46`) is the driver's test for "is this an empty spot a bud
    /// can start growing into" -- air, or a full water source block.
    #[test]
    fn cluster_can_grow_into_air_and_water_source_but_not_flowing_water_or_solids() {
        assert!(can_cluster_grow_at_state(
            &Block::AIR,
            Block::AIR.default_state.id
        ));
        assert!(can_cluster_grow_at_state(
            &Block::WATER,
            Block::WATER.default_state.id
        ));
        assert!(!can_cluster_grow_at_state(
            &Block::STONE,
            Block::STONE.default_state.id
        ));
    }
}
