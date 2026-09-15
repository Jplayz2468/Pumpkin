use crate::{
    block::{
        BlockBehaviour, BlockMetadata, GetStateForNeighborUpdateArgs, OnPlaceArgs,
        OnScheduledTickArgs, PlacedArgs,
    },
    entity::falling::FallingEntity,
};
use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockState, BlockStateId,
    fluid::Fluid,
    tag::{self, Taggable},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;

pub struct FallingBlock;

impl FallingBlock {
    #[must_use]
    pub fn can_fall_through(state: &BlockState, block: &Block) -> bool {
        state.is_air()
            || block.has_tag(&tag::Block::MINECRAFT_FIRE)
            || state.is_liquid()
            || state.replaceable()
    }

    #[must_use]
    pub fn can_solidify(state: &BlockState) -> bool {
        state.is_waterlogged()
            || Fluid::from_state_id(state.id)
                .is_some_and(|f| f.has_tag(&tag::Fluid::MINECRAFT_WATER))
    }

    #[must_use]
    pub fn touches_liquid(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        // Down is skipped: vanilla only checks it when the position being tested is
        // itself already water (ConcretePowderBlock.java:59-62, the pre-offset
        // `canSolidify(blockState)` gate on the DOWN branch), which `should_solidify`
        // already covers via its own `can_solidify(own position)` check.
        for dir in BlockDirection::all() {
            if dir == BlockDirection::Down {
                continue;
            }
            let neighbor = world.get_block_state(&pos.offset(dir.to_offset()));
            // ConcretePowderBlock.java:64 also requires the neighbour's face towards
            // `pos` to not be sturdy -- e.g. a waterlogged slab/stairs whose solid
            // half blocks contact should not solidify the powder even though it
            // contains water.
            if Self::can_solidify(neighbor) && !neighbor.is_side_solid(dir.opposite()) {
                return true;
            }
        }
        false
    }

    #[must_use]
    pub fn should_solidify(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        Self::can_solidify(world.get_block_state(pos)) || Self::touches_liquid(world, pos)
    }
}

impl BlockMetadata for FallingBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::GRAVEL,
            BlockId::SAND,
            BlockId::RED_SAND,
            BlockId::SUSPICIOUS_SAND,
            BlockId::SUSPICIOUS_GRAVEL,
            BlockId::WHITE_CONCRETE_POWDER,
            BlockId::ORANGE_CONCRETE_POWDER,
            BlockId::MAGENTA_CONCRETE_POWDER,
            BlockId::LIGHT_BLUE_CONCRETE_POWDER,
            BlockId::YELLOW_CONCRETE_POWDER,
            BlockId::LIME_CONCRETE_POWDER,
            BlockId::PINK_CONCRETE_POWDER,
            BlockId::GRAY_CONCRETE_POWDER,
            BlockId::LIGHT_GRAY_CONCRETE_POWDER,
            BlockId::CYAN_CONCRETE_POWDER,
            BlockId::PURPLE_CONCRETE_POWDER,
            BlockId::BLUE_CONCRETE_POWDER,
            BlockId::BROWN_CONCRETE_POWDER,
            BlockId::GREEN_CONCRETE_POWDER,
            BlockId::RED_CONCRETE_POWDER,
            BlockId::BLACK_CONCRETE_POWDER,
        ]
        .into()
    }
}

impl BlockBehaviour for FallingBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        // Vanilla getPlacementState uses shouldSolidify (own position OR neighbors),
        // so powder placed directly into water hardens even without a water neighbor.
        if args.block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS)
            && Self::should_solidify(args.world, args.position)
            && let Some(name) = args.block.name.strip_suffix("_powder")
            && let Some(concrete) = Block::from_name(name)
        {
            return concrete.default_state.id;
        }
        args.block.default_state.id
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        args.world
            .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.block.has_tag(&tag::Block::MINECRAFT_CONCRETE_POWDERS)
            && Self::touches_liquid(args.world, args.position)
            && let Some(name) = args.block.name.strip_suffix("_powder")
            && let Some(concrete) = Block::from_name(name)
        {
            return concrete.default_state.id;
        }

        args.world
            .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (block, state) = args.world.get_block_and_state(&args.position.down());
        if !Self::can_fall_through(state, block) || args.position.0.y < args.world.min_y {
            return;
        }
        let state = args.world.get_block_state(args.position);
        FallingEntity::replace_spawn(args.world, *args.position, state.id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pumpkin_data::block_properties::{BlockProperties, OakSlabProperties, SlabType};

    use super::*;

    /// A still, source water state -- the state vanilla's `canSolidify`
    /// (ConcretePowderBlock.java:74-76) checks for via `FluidState.is(FluidTags.WATER)`.
    fn water_source_state() -> &'static BlockState {
        let id = Fluid::WATER
            .states
            .iter()
            .find(|state| state.is_still && state.is_source)
            .map(|state| state.block_state_id)
            .expect("the water fluid always has a still source state");
        BlockState::from_id(id)
    }

    fn waterlogged_bottom_slab() -> &'static BlockState {
        let id = OakSlabProperties {
            r#type: SlabType::Bottom,
            r#waterlogged: true,
        }
        .to_state_id(&Block::OAK_SLAB);
        BlockState::from_id(id)
    }

    /// A minimal [`BlockAccessor`] backed by a map, so `touches_liquid` -- the real
    /// production function -- can be exercised without a full `World`.
    #[derive(Default)]
    struct MockAccessor {
        states: HashMap<BlockPos, (&'static Block, &'static BlockState)>,
    }

    impl MockAccessor {
        fn set(&mut self, pos: BlockPos, block: &'static Block, state: &'static BlockState) {
            self.states.insert(pos, (block, state));
        }
    }

    impl BlockAccessor for MockAccessor {
        fn get_block(&self, position: &BlockPos) -> &'static Block {
            self.get_block_and_state(position).0
        }

        fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
            self.get_block_and_state(position).1
        }

        fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
            self.get_block_state(position).id
        }

        fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState) {
            self.states
                .get(position)
                .copied()
                .unwrap_or((&Block::AIR, Block::AIR.default_state))
        }
    }

    #[test]
    fn can_solidify_matches_vanilla_fluid_tag_check() {
        // ConcretePowderBlock.java:74-76: `canSolidify` is true exactly when the
        // fluid state is tagged water -- true for a water source, true for a
        // waterlogged solid block, false for stone and for a dry slab.
        assert!(FallingBlock::can_solidify(water_source_state()));
        assert!(FallingBlock::can_solidify(waterlogged_bottom_slab()));
        assert!(!FallingBlock::can_solidify(Block::STONE.default_state));

        let dry_slab_id = OakSlabProperties {
            r#type: SlabType::Bottom,
            r#waterlogged: false,
        }
        .to_state_id(&Block::OAK_SLAB);
        assert!(!FallingBlock::can_solidify(BlockState::from_id(dry_slab_id)));
    }

    #[test]
    fn touches_liquid_true_for_plain_water_neighbor() {
        let pos = BlockPos::new(0, 64, 0);
        let mut world = MockAccessor::default();
        world.set(
            pos.offset(BlockDirection::North.to_offset()),
            &Block::WATER,
            water_source_state(),
        );

        assert!(FallingBlock::touches_liquid(&world, &pos));
    }

    #[test]
    fn touches_liquid_ignores_sturdy_face_of_waterlogged_neighbor() {
        // ConcretePowderBlock.java:64: a neighbour counts only if it `canSolidify`
        // AND its face towards `pos` is not sturdy. A waterlogged bottom slab placed
        // directly above `pos` carries water, but its bottom face is a full sturdy
        // square resting on `pos`, so vanilla does not solidify the powder under it.
        let slab_state = waterlogged_bottom_slab();
        assert!(
            slab_state.is_side_solid(BlockDirection::Down),
            "a bottom slab's bottom face must be sturdy for this test to be meaningful"
        );

        let pos = BlockPos::new(0, 64, 0);
        let mut world = MockAccessor::default();
        world.set(pos.up(), &Block::OAK_SLAB, slab_state);

        assert!(!FallingBlock::touches_liquid(&world, &pos));
    }

    #[test]
    fn touches_liquid_ignores_water_directly_below() {
        // ConcretePowderBlock.java:59-62: the DOWN neighbour is only consulted
        // through the pre-offset gate on `pos`'s own state (already covered by
        // `should_solidify`'s separate `can_solidify(own position)` check), so
        // `touches_liquid` alone must not react to water only below.
        let pos = BlockPos::new(0, 64, 0);
        let mut world = MockAccessor::default();
        world.set(pos.down(), &Block::WATER, water_source_state());

        assert!(!FallingBlock::touches_liquid(&world, &pos));
    }

    #[test]
    fn should_solidify_true_when_placed_into_water() {
        // ConcretePowderBlock.java:48-52: `getStateForPlacement` solidifies when the
        // replaced block itself is water, even with no water-bearing neighbor at all.
        let pos = BlockPos::new(0, 64, 0);
        let mut world = MockAccessor::default();
        world.set(pos, &Block::WATER, water_source_state());

        assert!(FallingBlock::should_solidify(&world, &pos));
    }
}
