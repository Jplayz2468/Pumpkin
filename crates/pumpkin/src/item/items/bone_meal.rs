use std::any::Any;
use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;
use pumpkin_data::block_properties::{HorizontalFacing, LadderLikeProperties};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockDirection, BlockId, BlockStateId, HorizontalFacingExt};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct BoneMealItem;

impl ItemMetadata for BoneMealItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::BONE_MEAL.id])
    }
}

impl ItemBehaviour for BoneMealItem {
    #[allow(clippy::too_many_lines)]
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        let state_id = world.get_block_state_id(&location);
        if server
            .block_registry
            .bone_meal(block, &world, &location, state_id)
        {
            world.sync_world_event(WorldEvent::ParticlesAndSoundPlantGrowth, location, 15);
            item.decrement_unless_creative(player.gamemode.load(), 1);
            return BlockActionResult::Success;
        }

        // Vanilla `BoneMealItem.useOn` stage 2 (BoneMealItem.java:48-61): when the crop-growth
        // stage (`growCrop`) does nothing, and the clicked face is sturdy, bonemeal instead
        // tries to spread seagrass/coral on the block adjacent to that face.
        let clicked_state = world.get_block_state(&location);
        if clicked_state.is_side_solid(face) {
            let relative = location.offset(face.to_offset());
            if grow_water_plant(&world, relative, face) {
                world.sync_world_event(WorldEvent::ParticlesAndSoundPlantGrowth, relative, 15);
                item.decrement_unless_creative(player.gamemode.load(), 1);
                return BlockActionResult::Success;
            }
        }

        BlockActionResult::Pass
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Port of vanilla `BoneMealItem.growWaterPlant` (BoneMealItem.java:81-142).
///
/// `pos` must be a full water source (BoneMealItem.java:82); from there this runs a
/// 128-iteration random walk (BoneMealItem.java:89-134) that spreads seagrass, and in
/// `#produces_corals_from_bonemeal` biomes, wall/free-standing corals or
/// `#underwater_bonemeals`-tagged blocks, with a coral wall-fan orientation retry
/// (BoneMealItem.java:118-122) and a 1/10 chance of re-triggering seagrass's own bonemeal
/// on a hit (BoneMealItem.java:128-131).
fn grow_water_plant(world: &Arc<World>, pos: BlockPos, clicked_face: BlockDirection) -> bool {
    // BoneMealItem.java:82
    if world.get_block(&pos) != &Block::WATER || !is_full_water(world, &pos) {
        return false;
    }

    'attempt: for j in 0..128i32 {
        // BoneMealItem.java:90-91
        let mut test_pos = pos;
        let mut state_to_grow = Block::SEAGRASS.default_state.id;

        // BoneMealItem.java:93-98: drift `j / 16` steps away from `pos`, aborting this
        // attempt (continuing the outer loop) the moment the walk hits a full-cube block.
        for _ in 0..(j / 16) {
            let dx = world.rand_bounded_i32(3) - 1;
            let dy = (world.rand_bounded_i32(3) - 1) * world.rand_bounded_i32(3) / 2;
            let dz = world.rand_bounded_i32(3) - 1;
            test_pos = test_pos.offset(Vector3::new(dx, dy, dz));
            if world.get_block_state(&test_pos).is_full_cube() {
                continue 'attempt;
            }
        }

        // BoneMealItem.java:100-116
        let biome = world.get_biome(&test_pos);
        if biome.has_tag(&tag::WorldgenBiome::MINECRAFT_PRODUCES_CORALS_FROM_BONEMEAL) {
            if j == 0 && clicked_face.is_horizontal() {
                if let Some(wall_block) =
                    random_tag_block(world, tag::Block::MINECRAFT_WALL_CORALS)
                {
                    let mut props = LadderLikeProperties::default(wall_block);
                    if let Some(facing) = clicked_face.to_horizontal_facing() {
                        props.facing = facing;
                    }
                    state_to_grow = props.to_state_id(wall_block);
                }
            } else if world.rand_bounded_i32(4) == 0 {
                if let Some(random_block) =
                    random_tag_block(world, tag::Block::MINECRAFT_UNDERWATER_BONEMEALS)
                {
                    state_to_grow = random_block.default_state.id;
                }
            }
        }

        // BoneMealItem.java:118-122: a coral wall-fan candidate retries a random horizontal
        // FACING up to 4 times until it can survive.
        if Block::from_state_id(state_to_grow).has_tag(&tag::Block::MINECRAFT_WALL_CORALS) {
            let mut attempts = 0;
            while !candidate_can_survive(world, state_to_grow, &test_pos) && attempts < 4 {
                let facing =
                    BlockDirection::horizontal_worldgen()[world.rand_bounded_i32(4) as usize];
                state_to_grow = set_wall_coral_facing(state_to_grow, facing);
                attempts += 1;
            }
        }

        // BoneMealItem.java:124-133
        if candidate_can_survive(world, state_to_grow, &test_pos) {
            let test_block = world.get_block(&test_pos);
            if test_block == &Block::WATER {
                if is_full_water(world, &test_pos) {
                    world.set_block_state(&test_pos, state_to_grow, BlockFlags::NOTIFY_ALL);
                }
            } else if test_block == &Block::SEAGRASS
                && world.get_block(&test_pos.up()) == &Block::WATER
                && world.rand_bounded_i32(10) == 0
            {
                let test_state_id = world.get_block_state_id(&test_pos);
                world
                    .block_registry
                    .bone_meal(&Block::SEAGRASS, world, &test_pos, test_state_id);
            }
        }
    }

    // BoneMealItem.java:136-137
    true
}

/// Vanilla `FluidState#isFull()` is `getAmount() == 8` (FluidState.java:57-59). For the WATER
/// block, Pumpkin's `get_fluid_and_fluid_state` reports amount 8 for both a source
/// (`is_source`) and a falling column (`falling`) -- see the comment above the `LEVEL`
/// handling in `World::fluid_state_from_block_state` -- so both count as "full" here.
fn is_full_water(world: &Arc<World>, pos: &BlockPos) -> bool {
    let (_, fluid_state) = world.get_fluid_and_fluid_state(pos);
    fluid_state.is_source || fluid_state.falling
}

/// Port of `BuiltInRegistries.BLOCK.getRandomElementOf(tag, random)` (BoneMealItem.java:103-104,
/// :111-112): pick a uniformly random block out of `tag`'s members.
fn random_tag_block(world: &Arc<World>, tag: tag::Tag) -> Option<&'static Block> {
    let ids = tag.1;
    if ids.is_empty() {
        return None;
    }
    let index = world.rand_bounded_i32(ids.len() as i32) as usize;
    BlockId::new(ids[index]).map(BlockId::to_block)
}

/// Port of `BaseCoralWallFanBlock.canSurvive` (BaseCoralWallFanBlock.java:76-81): the block
/// behind the fan (opposite its FACING) needs a sturdy face toward FACING.
fn wall_coral_can_survive(world: &Arc<World>, pos: &BlockPos, facing: HorizontalFacing) -> bool {
    let support_pos = pos.offset(facing.opposite().to_offset());
    world
        .get_block_state(&support_pos)
        .is_side_solid(facing.to_block_direction())
}

/// `BlockState#canSurvive` for a not-yet-placed candidate state (BoneMealItem.java:119, :124).
/// Wall corals get the exact FACING-specific vanilla check above (their registered
/// `can_place_at` ignores the candidate's FACING and instead accepts any sturdy horizontal
/// neighbour, which is not equivalent); every other candidate goes through the registered
/// block's own `can_place_at`, the same generic canSurvive dispatch `GrassBlock::perform_bonemeal`
/// uses for its own bonemeal-placed candidate states.
fn candidate_can_survive(world: &Arc<World>, state_id: BlockStateId, pos: &BlockPos) -> bool {
    let block = Block::from_state_id(state_id);
    if block.has_tag(&tag::Block::MINECRAFT_WALL_CORALS) {
        let facing = LadderLikeProperties::from_state_id(state_id).facing;
        wall_coral_can_survive(world, pos, facing)
    } else {
        let state = state_id.to_state();
        world.block_registry.can_place_at(
            None,
            Some(world),
            world.as_ref(),
            None,
            block,
            state,
            pos,
            None,
            None,
        )
    }
}

fn set_wall_coral_facing(state_id: BlockStateId, facing: HorizontalFacing) -> BlockStateId {
    let block = Block::from_state_id(state_id);
    let mut props = LadderLikeProperties::from_state_id(state_id);
    props.facing = facing;
    props.to_state_id(block)
}
