use pumpkin_data::{Block, BlockStateId, tag, tag::Taggable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockAccessor;

pub mod azalea;
pub mod bamboo;
pub mod bamboo_sapling;
pub mod big_dripleaf;
pub mod big_dripleaf_stem;
pub mod bush;
pub mod cactus;
pub mod cactus_flower;
pub mod cave_vines;
pub mod chorus_flower;
pub mod chorus_plant;
pub mod cocoa;
pub mod crop;
pub mod dry_vegetation;
pub mod eyeblossom;
pub mod flower;
pub mod flowerbed;
pub mod fungus;
mod growing;
pub mod hanging_moss;
pub mod hanging_roots;
pub mod kelp;
pub mod leaf_litter;
pub mod lily_pad;
pub mod mangrove_propagule;
pub mod mushroom_plant;
pub mod nether_sprouts;
pub mod roots;
pub mod sapling;
pub mod sea_pickles;
pub mod seagrass;
pub mod segmented;
pub mod short_plant;
pub mod small_dripleaf;
pub mod spore_blossom;
pub mod sugar_cane;
pub mod tall_plant;
pub mod tall_seagrass;
pub mod tree_grower;
pub mod twisting_vines;
pub mod weeping_vines;
pub mod wither_rose;

trait PlantBlockBase {
    fn can_plant_on_top(&self, block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let block = block_accessor.get_block(pos);
        block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION)
    }

    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !self.can_place_at(block_accessor, block_pos) {
            return Block::AIR.default_state.id;
        }
        block_state
    }

    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        self.can_plant_on_top(block_accessor, &block_pos.down())
    }
}

// BonemealableBlock: check N/E/S/W, then use the world's Java shuffle on growth.
const SPREAD_DIRECTIONS: [pumpkin_data::BlockDirection; 4] = [
    pumpkin_data::BlockDirection::North,
    pumpkin_data::BlockDirection::East,
    pumpkin_data::BlockDirection::South,
    pumpkin_data::BlockDirection::West,
];

fn spreadable_neighbor(
    world: &crate::world::World,
    pos: &BlockPos,
    state: &'static pumpkin_data::BlockState,
    shuffle: bool,
) -> Option<BlockPos> {
    let mut directions = SPREAD_DIRECTIONS;
    if shuffle {
        for i in (1..directions.len()).rev() {
            directions.swap(i, world.rand_bounded_i32((i + 1) as i32) as usize);
        }
    }
    directions.into_iter().find_map(|direction| {
        let next = pos.offset(direction.to_offset());
        (world.get_block_state(&next).is_air()
            && world.block_registry.can_place_at(
                None,
                Some(world),
                world,
                None,
                state.id.to_block(),
                state,
                &next,
                None,
                None,
            ))
        .then_some(next)
    })
}

fn is_upper_half(state: BlockStateId) -> bool {
    state
        .to_block()
        .properties(state)
        .is_some_and(|props| props.to_props().contains(&("half", "upper")))
}

fn double_plant_survives(
    accessor: &dyn BlockAccessor,
    block: &Block,
    state: BlockStateId,
    pos: &BlockPos,
    lower_survives: bool,
) -> bool {
    if !is_upper_half(state) {
        lower_survives
    } else {
        let (below, state) = accessor.get_block_and_state(&pos.down());
        below == block && !is_upper_half(state.id)
    }
}

fn double_plant_neighbor_state(
    args: &crate::block::GetStateForNeighborUpdateArgs<'_>,
    lower_survives: bool,
) -> BlockStateId {
    use pumpkin_data::BlockDirection;
    let upper = is_upper_half(args.state_id);
    let other_direction = if upper {
        BlockDirection::Down
    } else {
        BlockDirection::Up
    };
    if args.direction == other_direction
        && (args.neighbor_state_id.to_block() != args.block
            || is_upper_half(args.neighbor_state_id) == upper)
    {
        return Block::AIR.default_state.id;
    }
    if double_plant_survives(
        args.world,
        args.block,
        args.state_id,
        args.position,
        lower_survives,
    ) {
        args.state_id
    } else {
        Block::AIR.default_state.id
    }
}

fn full_water_at(accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let (fluid, state) =
        crate::world::World::fluid_state_from_block_state(accessor.get_block_state_id(pos));
    fluid.matches_type(&pumpkin_data::fluid::Fluid::WATER) && state.level == 8
}

fn harvest_loot(
    args: &crate::block::NormalUseArgs<'_>,
    table: &pumpkin_util::loot_table::LootTable,
) -> bool {
    let params = crate::world::loot::LootContextParameters {
        block_state: Some(args.world.get_block_state(args.position)),
        world_time: args.world.level_info.load().day_time as u64,
        is_raining: Some(args.world.is_raining()),
        is_thundering: Some(args.world.is_thundering()),
        ..Default::default()
    };
    let drops = crate::world::loot::generate_loot_in_world(args.world, table, 0, &params);
    let mut event =
        crate::plugin::api::events::player::player_harvest_block::PlayerHarvestBlockEvent {
            player: args.player.clone(),
            block_pos: *args.position,
            harvested_items: drops,
            cancelled: false,
        };
    if let Some(server) = args.world.server.upgrade() {
        server.plugin_manager.fire_blocking(&server, &mut event);
    }
    if event.cancelled {
        return false;
    }
    for stack in event.harvested_items {
        args.world.drop_stack(args.position, stack);
    }
    true
}
