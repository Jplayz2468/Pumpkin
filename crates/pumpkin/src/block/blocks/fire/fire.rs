use pumpkin_data::BlockStateId;
use pumpkin_data::biome::Biome;
use pumpkin_data::block_properties::HorizontalAxis;
use pumpkin_data::dimension::Dimension;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockDirection};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use std::sync::Arc;

use crate::block::blocks::tnt::TNTBlock;
use crate::block::{
    BlockBehaviour, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnEntityCollisionArgs, OnScheduledTickArgs, PlacedArgs,
};
use crate::entity::EntityBase;
use crate::world::World;
use crate::world::portal::nether::NetherPortal;

type FireProperties = pumpkin_data::block_properties::FireLikeProperties;

use super::FireBlockBase;

#[pumpkin_block("minecraft:fire")]
pub struct FireBlock;

impl FireBlock {
    #[must_use]
    pub fn get_fire_tick_delay(world: &World) -> i32 {
        30 + world.rand_bounded_i32(10)
    }

    fn is_flammable(id: BlockStateId) -> bool {
        let block = id.to_block();

        if block.is_waterlogged(id) {
            return false;
        }

        block
            .flammable
            .as_ref()
            .is_some_and(|f| f.spread_chance > 0)
    }

    fn are_blocks_around_flammable(block_accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        for direction in BlockDirection::all() {
            let neighbor_pos = pos.offset(direction.to_offset());
            let state_id = block_accessor.get_block_state_id(&neighbor_pos);
            if Self::is_flammable(state_id) {
                return true;
            }
        }
        false
    }

    pub fn get_state_for_position(
        &self,
        world: &World,
        block: &Block,
        pos: &BlockPos,
    ) -> BlockStateId {
        let down_pos = pos.down();
        let down_state = world.get_block_state(&down_pos);
        if Self::is_flammable(down_state.id) || down_state.is_side_solid(BlockDirection::Up) {
            return block.default_state.id;
        }
        let mut fire_props = FireProperties::from_state_id(block.default_state.id);
        for direction in BlockDirection::all() {
            let neighbor_pos = pos.offset(direction.to_offset());
            let neighbor_state_id = world.get_block_state_id(&neighbor_pos);
            if Self::is_flammable(neighbor_state_id) {
                match direction {
                    BlockDirection::North => fire_props.north = true,
                    BlockDirection::South => fire_props.south = true,
                    BlockDirection::East => fire_props.east = true,
                    BlockDirection::West => fire_props.west = true,
                    BlockDirection::Up => fire_props.up = true,
                    BlockDirection::Down => {}
                }
            }
        }
        fire_props.to_state_id(block)
    }

    // Used for spreading fire
    pub fn get_burn_chance(&self, world: &Arc<World>, pos: &BlockPos) -> i32 {
        let block_state = world.get_block_state(pos);
        if !block_state.is_air() {
            return 0;
        }
        let mut total_burn_chance = 0;

        for dir in BlockDirection::all() {
            let neighbor_block = world.get_block(&pos.offset(dir.to_offset()));
            if *world.get_fluid(&pos.offset(dir.to_offset())) != Fluid::EMPTY {
                continue; // Skip if there is a fluid
            }
            if let Some(flammable) = &neighbor_block.flammable {
                total_burn_chance = total_burn_chance.max(i32::from(flammable.spread_chance));
            }
        }

        total_burn_chance
    }

    fn is_near_rain(world: &World, pos: &BlockPos) -> bool {
        world.is_raining_at(pos)
            || world.is_raining_at(&pos.west())
            || world.is_raining_at(&pos.east())
            || world.is_raining_at(&pos.north())
            || world.is_raining_at(&pos.south())
    }

    fn get_state_with_age(&self, world: &World, pos: &BlockPos, age: u8) -> BlockStateId {
        if FireBlockBase::get_fire_type(world, pos) == Block::SOUL_FIRE {
            return Block::SOUL_FIRE.default_state.id;
        }
        let mut props =
            FireProperties::from_state_id(self.get_state_for_position(world, &Block::FIRE, pos));
        props.age = age;
        props.to_state_id(&Block::FIRE)
    }

    fn is_increased_burnout_biome(world: &World, pos: &BlockPos) -> bool {
        // Fire burnout increases in specific biomes
        // TODO: Use proper tag or bool for this when available
        let biome_id = world.level.get_rough_biome(pos).id;
        matches!(
            biome_id,
            id if id == Biome::BAMBOO_JUNGLE.id
                || id == Biome::MUSHROOM_FIELDS.id
                || id == Biome::MANGROVE_SWAMP.id
                || id == Biome::SNOWY_SLOPES.id
                || id == Biome::FROZEN_PEAKS.id
                || id == Biome::JAGGED_PEAKS.id
                || id == Biome::SWAMP.id
                || id == Biome::JUNGLE.id
        )
    }

    fn try_spreading_fire(&self, world: &Arc<World>, pos: &BlockPos, chance: i32, age: u8) {
        let block = world.get_block(pos);
        let odds = if world.get_block_state(pos).is_waterlogged() {
            0
        } else {
            block
                .flammable
                .as_ref()
                .map_or(0, |f| i32::from(f.burn_chance))
        };
        if world.rand_bounded_i32(chance) < odds {
            if let Some(server) = world.server.upgrade() {
                let mut event = crate::plugin::api::events::block::block_burn::BlockBurnEvent {
                    igniting_block: &Block::FIRE,
                    block,
                    cancelled: false,
                };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return;
                }
            }
            let old_block = block;
            if world.rand_bounded_i32(i32::from(age) + 10) < 5 && !world.is_raining_at(pos) {
                let new_age = (age + (world.rand_bounded_i32(5) as u8 / 4)).min(15) as u8;
                let new_state_id = self.get_state_with_age(world, pos, new_age);
                world.set_block_state(pos, new_state_id, BlockFlags::NOTIFY_ALL);
            } else {
                world.set_block_state(pos, Block::AIR.default_state.id, BlockFlags::NOTIFY_ALL);
            }

            if old_block == &Block::TNT {
                TNTBlock::prime(world, pos, false);
            }
        }
    }
}

impl BlockBehaviour for FireBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let dimension = &args.world.dimension;
        // First lets check if we are in OverWorld or Nether, its not possible to place an Nether portal in other dimensions in Vanilla
        if (dimension == &Dimension::OVERWORLD || dimension == &Dimension::THE_NETHER)
            && let Some(portal) =
                NetherPortal::get_new_portal(args.world, args.position, HorizontalAxis::X)
        {
            portal.create(args.world);
        } else if !self.can_place_at(CanPlaceAtArgs {
            server: None,
            world: Some(args.world),
            block_accessor: args.world.as_ref(),
            block: args.block,
            state: args.state_id.to_state(),
            position: args.position,
            direction: None,
            player: None,
            use_item_on: None,
        }) {
            args.world
                .set_block_state(args.position, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
        }

        self.state_changed(args);
    }

    fn state_changed(&self, args: PlacedArgs<'_>) {
        args.world.schedule_block_tick(
            args.block,
            *args.position,
            Self::get_fire_tick_delay(args.world) as u32,
            TickPriority::Normal,
        );
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        FireBlockBase::apply_fire_collision(&args, false);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if self.can_place_at(CanPlaceAtArgs {
            server: None,
            world: Some(args.world),
            block_accessor: args.world,
            block: &Block::FIRE,
            state: Block::FIRE.default_state,
            position: args.position,
            direction: None,
            player: None,
            use_item_on: None,
        }) {
            self.get_state_with_age(
                args.world,
                args.position,
                FireProperties::from_state_id(args.state_id).age,
            )
        } else {
            Block::AIR.default_state.id
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let state = args.block_accessor.get_block_state(&args.position.down());
        if state.is_side_solid(BlockDirection::Up) {
            return true;
        }
        Self::are_blocks_around_flammable(args.block_accessor, args.position)
    }

    #[expect(clippy::too_many_lines)]
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (world, block, pos) = (args.world, args.block, args.position);

        // Schedule next tick first
        world.schedule_block_tick(
            block,
            *pos,
            Self::get_fire_tick_delay(world) as u32,
            TickPriority::Normal,
        );

        let spread_radius = world
            .level_info
            .load()
            .game_rules
            .fire_spread_radius_around_player;
        if spread_radius != -1
            && !world.players.load().iter().any(|player| {
                player.gamemode.load() != pumpkin_util::GameMode::Spectator
                    && player
                        .get_entity()
                        .pos
                        .load()
                        .squared_distance_to_vec(&pos.to_f64())
                        < f64::from(spread_radius).powi(2)
                    && spread_radius > 0
            })
        {
            return;
        }
        let block_state = world.get_block_state(pos);

        // Vanilla keeps evaluating the captured state after removing unsupported fire.
        if !self.can_place_at(CanPlaceAtArgs {
            server: None,
            world: Some(world),
            block_accessor: world.as_ref(),
            block,
            state: block.default_state,
            position: pos,
            direction: None,
            player: None,
            use_item_on: None,
        }) {
            world.set_block_state(pos, Block::AIR.default_state.id, BlockFlags::NOTIFY_ALL);
        }

        let block_below = world.get_block(&pos.down());

        // Check for infiniburn blocks (depending on dimension)
        let infiniburn = match world.dimension.id {
            id if id == Dimension::OVERWORLD.id => {
                block_below.has_tag(&tag::Block::MINECRAFT_INFINIBURN_OVERWORLD)
            }
            id if id == Dimension::THE_NETHER.id => {
                block_below.has_tag(&tag::Block::MINECRAFT_INFINIBURN_NETHER)
            }
            id if id == Dimension::THE_END.id => {
                block_below.has_tag(&tag::Block::MINECRAFT_INFINIBURN_END)
            }
            _ => false,
        };

        let mut fire_props = FireProperties::from_state_id(block_state.id);
        let age = fire_props.age;

        // Check if rain should extinguish the fire
        if !infiniburn && Self::is_near_rain(world.as_ref(), pos) {
            let rain_chance = 0.2 + (age as f32) * 0.03;
            if world.rand_f32() < rain_chance {
                world.set_block_state(pos, Block::AIR.default_state.id, BlockFlags::NOTIFY_ALL);
                return;
            }
        }

        // Increment age
        let random = (world.rand_bounded_i32(3) / 2) as u8;
        let new_age = (age + random).min(15);
        if new_age != age {
            fire_props.age = new_age;
            let new_state_id = fire_props.to_state_id(&Block::FIRE);
            world.set_block_state(
                pos,
                new_state_id,
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
        }

        if !infiniburn {
            // Check if fire should extinguish due to lack of fuel
            if !Self::are_blocks_around_flammable(world.as_ref(), pos) {
                let block_below_state = world.get_block_state(&pos.down());
                if !block_below_state.is_side_solid(BlockDirection::Up) || age > 3 {
                    world.set_block_state(pos, Block::AIR.default_state.id, BlockFlags::NOTIFY_ALL);
                }
                return;
            }

            // At max age, fire has a chance to extinguish if not on flammable block
            if age == 15
                && world.rand_bounded_i32(4) == 0
                && !Self::is_flammable(world.get_block_state_id(&pos.down()))
            {
                world.set_block_state(pos, Block::AIR.default_state.id, BlockFlags::NOTIFY_ALL);
                return;
            }
        }

        // Burn adjacent blocks
        let extra = if Self::is_increased_burnout_biome(world, pos) {
            -50 // Increases chance of block being destroyed
        } else {
            0
        };

        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::East.to_offset()),
            300 + extra,
            age,
        );
        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::West.to_offset()),
            300 + extra,
            age,
        );
        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::Down.to_offset()),
            250 + extra,
            age,
        );
        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::Up.to_offset()),
            250 + extra,
            age,
        );
        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::North.to_offset()),
            300 + extra,
            age,
        );
        self.try_spreading_fire(
            world,
            &pos.offset(BlockDirection::South.to_offset()),
            300 + extra,
            age,
        );

        // Try to spread fire to nearby air blocks
        let difficulty = world.level_info.load().difficulty as i32;
        for xx in -1..=1 {
            for zz in -1..=1 {
                for yy in -1..=4 {
                    if xx != 0 || yy != 0 || zz != 0 {
                        let offset_pos = pos.offset(Vector3::new(xx, yy, zz));
                        let ignite_odds = self.get_burn_chance(world, &offset_pos);

                        if ignite_odds > 0 {
                            // Calculate spread rate based on height
                            let rate = if yy > 1 { 100 + (yy - 1) * 100 } else { 100 };

                            // Calculate odds of spreading
                            let mut odds =
                                (ignite_odds + 40 + difficulty * 7) / (i32::from(age) + 30);

                            // Reduce spread odds in certain biomes
                            if extra != 0 {
                                odds /= 2; // Fire spreads 50% slower
                            }

                            if odds > 0
                                && world.rand_bounded_i32(rate) <= odds
                                && !Self::is_near_rain(world.as_ref(), &offset_pos)
                            {
                                let spread_age =
                                    (age + world.rand_bounded_i32(5) as u8 / 4).min(15) as u8;
                                let new_state_id =
                                    self.get_state_with_age(world, &offset_pos, spread_age);

                                if let Some(server) = world.server.upgrade() {
                                    let mut event = crate::plugin::api::events::block::block_spread::BlockSpreadEvent::new(
                                        *pos,
                                        offset_pos,
                                        world.clone(),
                                        new_state_id,
                                    );
                                    server.plugin_manager.fire_blocking(&server, &mut event);
                                    if event.cancelled {
                                        continue;
                                    }
                                }

                                world.set_block_state(
                                    &offset_pos,
                                    new_state_id,
                                    BlockFlags::NOTIFY_ALL,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        {
            FireBlockBase::broken(args.world, *args.position);
        }
    }
}
