use std::sync::Arc;

use pumpkin_data::block_properties::{SnowLikeProperties, WaterCauldronLikeProperties};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::Block;
use pumpkin_util::biome::BiomePrecipitation;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::chunk::ChunkHeightmapType;
use pumpkin_world::world::BlockFlags;

use super::World;

/// Determines if water at `pos` should freeze into ice.
///
/// Reference: Vanilla Java 26.2 `Biome.java:145-169` (`shouldFreeze(level, pos, true)`).
#[must_use]
pub fn should_freeze(world: &World, pos: &BlockPos) -> bool {
    let biome = world.get_biome(pos);
    if biome
        .weather
        .warm_enough_to_rain(pos.0.x, pos.0.y, pos.0.z, world.sea_level)
    {
        return false;
    }

    let min_y = world.min_y;
    let max_y = min_y + world.dimension.height as i32;
    if pos.0.y < min_y || pos.0.y >= max_y {
        return false;
    }

    if world.get_block_light_level(pos).unwrap_or(0) >= 10 {
        return false;
    }

    let (block, fluid) = world.get_block_and_fluid(pos);
    if fluid.id != Fluid::WATER.id || block.id != Block::WATER.id {
        return false;
    }

    // Still water source check (Blocks.WATER default state has level 0)
    let state = world.get_block_state(pos);
    if state.id != Block::WATER.default_state.id {
        return false;
    }

    // Ice only freezes from shore inward: must NOT be completely surrounded by water on horizontal sides
    let surrounded_by_water = is_water_at(world, &pos.west())
        && is_water_at(world, &pos.east())
        && is_water_at(world, &pos.north())
        && is_water_at(world, &pos.south());

    !surrounded_by_water
}

#[inline]
fn is_water_at(world: &World, pos: &BlockPos) -> bool {
    world.get_fluid(pos).id == Fluid::WATER.id
}

/// Determines if snow should be placed at `pos`.
///
/// Reference: Vanilla Java 26.2 `Biome.java:183-195` (`shouldSnow(level, pos)`).
#[must_use]
pub fn should_snow(world: &World, pos: &BlockPos) -> bool {
    let biome = world.get_biome(pos);
    if biome
        .weather
        .get_precipitation_at(pos.0.x, pos.0.y, pos.0.z, world.sea_level)
        != BiomePrecipitation::Snow
    {
        return false;
    }

    let min_y = world.min_y;
    let max_y = min_y + world.dimension.height as i32;
    if pos.0.y < min_y || pos.0.y >= max_y {
        return false;
    }

    if world.get_block_light_level(pos).unwrap_or(0) >= 10 {
        return false;
    }

    let (block, state) = world.get_block_and_state(pos);
    if (state.is_air() || block.id == Block::SNOW.id)
        && crate::block::blocks::snow::can_place_at(world, pos)
    {
        return true;
    }

    false
}

/// Handles precipitation effects on special blocks such as cauldrons.
///
/// Reference: Vanilla Java 26.2 `CauldronBlock.java:33-51` and `LayeredCauldronBlock.java:121-127`.
pub fn handle_precipitation(
    world: &Arc<World>,
    pos: &BlockPos,
    precipitation: BiomePrecipitation,
) {
    let (block, state_id) = world.get_block_and_state_id(pos);

    if block.id == Block::CAULDRON.id {
        let should_fill = match precipitation {
            BiomePrecipitation::Rain => world.rand_f32() < 0.05,
            BiomePrecipitation::Snow => world.rand_f32() < 0.1,
            BiomePrecipitation::None => false,
        };

        if should_fill {
            let new_state_id = match precipitation {
                BiomePrecipitation::Rain => {
                    let props = WaterCauldronLikeProperties { level: 1 };
                    props.to_state_id(&Block::WATER_CAULDRON)
                }
                BiomePrecipitation::Snow => {
                    let props = WaterCauldronLikeProperties { level: 1 };
                    props.to_state_id(&Block::POWDER_SNOW_CAULDRON)
                }
                BiomePrecipitation::None => return,
            };
            world.set_block_state(pos, new_state_id, BlockFlags::NOTIFY_ALL);
        }
    } else if block.id == Block::WATER_CAULDRON.id {
        if precipitation == BiomePrecipitation::Rain && world.rand_f32() < 0.05 {
            let mut props = WaterCauldronLikeProperties::from_state_id(state_id);
            if props.level < 3 {
                props.level += 1;
                let new_state_id = props.to_state_id(&Block::WATER_CAULDRON);
                world.set_block_state(pos, new_state_id, BlockFlags::NOTIFY_ALL);
            }
        }
    } else if block.id == Block::POWDER_SNOW_CAULDRON.id
        && precipitation == BiomePrecipitation::Snow
        && world.rand_f32() < 0.1
    {
        let mut props = WaterCauldronLikeProperties::from_state_id(state_id);
        if props.level < 3 {
            props.level += 1;
            let new_state_id = props.to_state_id(&Block::POWDER_SNOW_CAULDRON);
            world.set_block_state(pos, new_state_id, BlockFlags::NOTIFY_ALL);
        }
    }
}

/// Ticks precipitation at the given column position.
///
/// Reference: Vanilla Java 26.2 `ServerLevel.java:581-611` (`tickPrecipitation`).
pub fn tick_precipitation(world: &Arc<World>, pos: BlockPos) {
    let height = world.get_heightmap_height(ChunkHeightmapType::MotionBlocking, pos.0.x, pos.0.z);
    let top_pos = BlockPos::new(pos.0.x, height + 1, pos.0.z);
    let below_pos = BlockPos::new(pos.0.x, height, pos.0.z);

    let biome = world.get_biome(&top_pos);

    // Freezing water to ice (ServerLevel.java:585-587)
    if should_freeze(world, &below_pos) {
        world.set_block_state(
            &below_pos,
            Block::ICE.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
    }

    // Precipitation (ServerLevel.java:589-610)
    if world.is_raining() {
        let max_height = world.level_info.load().game_rules.max_snow_accumulation_height;
        if max_height > 0 && should_snow(world, &top_pos) {
            let (block, state_id) = world.get_block_and_state_id(&top_pos);
            if block.id == Block::SNOW.id {
                let mut props = SnowLikeProperties::from_state_id(state_id);
                let current_layers = i64::from(props.layers);
                if current_layers < max_height.min(8) {
                    props.layers += 1;
                    let new_state_id = props.to_state_id(&Block::SNOW);
                    world.set_block_state(&top_pos, new_state_id, BlockFlags::NOTIFY_ALL);
                }
            } else {
                let props = SnowLikeProperties { layers: 1 };
                let new_state_id = props.to_state_id(&Block::SNOW);
                world.set_block_state(&top_pos, new_state_id, BlockFlags::NOTIFY_ALL);
            }
        }

        let precipitation = biome.weather.get_precipitation_at(
            below_pos.0.x,
            below_pos.0.y,
            below_pos.0.z,
            world.sea_level,
        );
        if precipitation != BiomePrecipitation::None {
            handle_precipitation(world, &below_pos, precipitation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_layer_accumulation_bounds() {
        let max_height = 4i64;
        let mut props = SnowLikeProperties { layers: 1 };

        for _ in 0..10 {
            let current_layers = i64::from(props.layers);
            if current_layers < max_height.min(8) {
                props.layers += 1;
            }
        }

        assert_eq!(props.layers, 4);

        let max_height = 8i64;
        for _ in 0..10 {
            let current_layers = i64::from(props.layers);
            if current_layers < max_height.min(8) {
                props.layers += 1;
            }
        }

        assert_eq!(props.layers, 8);
    }

    #[test]
    fn cauldron_layer_increment() {
        let mut props = WaterCauldronLikeProperties { level: 1 };
        if props.level < 3 {
            props.level += 1;
        }
        assert_eq!(props.level, 2);

        if props.level < 3 {
            props.level += 1;
        }
        assert_eq!(props.level, 3);

        // Does not exceed level 3
        if props.level < 3 {
            props.level += 1;
        }
        assert_eq!(props.level, 3);
    }
}
