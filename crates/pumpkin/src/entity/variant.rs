//! Spawn-time variant selection for mobs whose appearance is drawn when they enter the
//! world rather than stored in their entity type.
//!
//! Vanilla drives this from data registries: each variant carries a
//! `SpawnPrioritySelectors` list, and `VariantUtils.selectVariantToSpawn`
//! (VariantUtils.java:35) hands them to `PriorityProvider.pick`
//! (PriorityProvider.java:20). `pick` keeps only the entries whose condition matches at
//! the *highest* priority that matched anything, then chooses uniformly among those.
//!
//! For the three farm animals the registry content is fixed in code
//! (CowVariants.java:27, PigVariants.java, ChickenVariants.java) and is the same shape in
//! all three cases:
//!
//! * `temperate` — no condition, priority 0 (the fallback)
//! * `warm`      — `BiomeCheck(#spawns_warm_variant_farm_animals)`, priority 1
//! * `cold`      — `BiomeCheck(#spawns_cold_variant_farm_animals)`, priority 1
//!
//! The two tagged sets are disjoint, so the priority machinery collapses to a plain
//! three-way test and is written that way here. The numeric ids are the registry-sync
//! order the client receives, which is alphabetical: cold = 0, temperate = 1, warm = 2.

use pumpkin_data::tag::{Taggable, WorldgenBiome};
use pumpkin_util::math::position::BlockPos;
use rand::RngExt;

use crate::world::World;

/// `cold` in the `cow_variant` / `pig_variant` / `chicken_variant` registries.
pub const TEMPERATURE_VARIANT_COLD: u8 = 0;
/// `temperate`, and the fallback when no biome tag matches.
pub const TEMPERATURE_VARIANT_TEMPERATE: u8 = 1;
/// `warm`.
pub const TEMPERATURE_VARIANT_WARM: u8 = 2;

/// The farm-animal variant a cow, pig or chicken spawning at `pos` should take.
///
/// Warm is tested before cold only because that is the order
/// `SheepColorSpawnRules.getSheepColorConfiguration` uses; the tags are disjoint so the
/// order cannot actually change the answer.
pub fn temperature_variant_at(world: &World, pos: &BlockPos) -> u8 {
    let biome = world.get_biome(pos);
    if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_WARM_VARIANT_FARM_ANIMALS) {
        TEMPERATURE_VARIANT_WARM
    } else if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_COLD_VARIANT_FARM_ANIMALS) {
        TEMPERATURE_VARIANT_COLD
    } else {
        TEMPERATURE_VARIANT_TEMPERATE
    }
}

/// Maps one of the three ids to the identifier vanilla writes under the `variant` NBT key.
pub fn temperature_variant_name(variant: u8) -> &'static str {
    match variant {
        TEMPERATURE_VARIANT_COLD => "minecraft:cold",
        TEMPERATURE_VARIANT_WARM => "minecraft:warm",
        _ => "minecraft:temperate",
    }
}

/// Parses the `variant` NBT value, accepting it with or without the namespace.
pub fn temperature_variant_from_name(name: &str) -> u8 {
    match name.strip_prefix("minecraft:").unwrap_or(name) {
        "cold" => TEMPERATURE_VARIANT_COLD,
        "warm" => TEMPERATURE_VARIANT_WARM,
        _ => TEMPERATURE_VARIANT_TEMPERATE,
    }
}

/// Uniform pick over a sound-variant registry of `count` entries, matching
/// `PigSoundVariants.pickRandomSoundVariant` (PigSoundVariants.java:34), which is
/// `Registry.getRandom` — an unweighted draw over every entry.
///
/// Registry order is alphabetical, as it is sent to the client:
/// `pig_sound_variant` = big, classic, mini; `cow_sound_variant` = classic, moody;
/// `chicken_sound_variant` = classic, picky.
pub fn random_sound_variant(count: i32) -> i32 {
    rand::rng().random_range(0..count)
}

// -- Sheep -----------------------------------------------------------------------------

/// Dye colour ids, in `DyeColor` ordinal order — the ids stored in the sheep's wool byte.
const WHITE: u8 = 0;
const ORANGE: u8 = 1;
const LIGHT_BLUE: u8 = 3;
const YELLOW: u8 = 4;
const LIME: u8 = 5;
const PINK: u8 = 6;
const GRAY: u8 = 7;
const LIGHT_GRAY: u8 = 8;
const CYAN: u8 = 9;
const PURPLE: u8 = 10;
const BLUE: u8 = 11;
const BROWN: u8 = 12;
const GREEN: u8 = 13;
const RED: u8 = 14;
const BLACK: u8 = 15;

/// One entry of a `WeightedList`: either a fixed colour, or the nested "common colours"
/// list that mixes the biome's dominant colour with a 1-in-500 pink.
#[derive(Clone, Copy)]
enum ColorEntry {
    Single(u8),
    /// `SheepColorSpawnRules.commonColors` (SheepColorSpawnRules.java:45): a second
    /// weighted draw of 499 × the given colour against 1 × pink.
    Common(u8),
}

/// The three spawn configurations from SheepColorSpawnRules.java:11-42, in the order the
/// builder adds them — order matters, because `WeightedRandom.getWeightedItem`
/// (WeightedRandom.java:39) walks the list subtracting weights until it goes negative.
const TEMPERATE_SHEEP_COLORS: &[(ColorEntry, i32)] = &[
    (ColorEntry::Single(BLACK), 5),
    (ColorEntry::Single(GRAY), 5),
    (ColorEntry::Single(LIGHT_GRAY), 5),
    (ColorEntry::Single(BROWN), 3),
    (ColorEntry::Common(WHITE), 82),
];

const WARM_SHEEP_COLORS: &[(ColorEntry, i32)] = &[
    (ColorEntry::Single(GRAY), 5),
    (ColorEntry::Single(LIGHT_GRAY), 5),
    (ColorEntry::Single(WHITE), 5),
    (ColorEntry::Single(BLACK), 3),
    (ColorEntry::Common(BROWN), 82),
];

const COLD_SHEEP_COLORS: &[(ColorEntry, i32)] = &[
    (ColorEntry::Single(LIGHT_GRAY), 5),
    (ColorEntry::Single(GRAY), 5),
    (ColorEntry::Single(WHITE), 5),
    (ColorEntry::Single(BROWN), 3),
    (ColorEntry::Common(BLACK), 82),
];

/// `WeightedRandom.getRandomItem`: draw `nextInt(total)`, then subtract each weight in
/// turn and take the entry that pushes the running value below zero.
fn pick_weighted(entries: &[(ColorEntry, i32)]) -> ColorEntry {
    let total: i32 = entries.iter().map(|(_, weight)| *weight).sum();
    let mut selection = rand::rng().random_range(0..total);
    for (entry, weight) in entries {
        selection -= *weight;
        if selection < 0 {
            return *entry;
        }
    }
    // Unreachable while every weight is positive and the total is their sum.
    entries[entries.len() - 1].0
}

/// `Sheep.getRandomSheepColor` (Sheep.java:270) via
/// `SheepColorSpawnRules.getSheepColor`. Returns a `DyeColor` id.
pub fn random_sheep_color(world: &World, pos: &BlockPos) -> u8 {
    let biome = world.get_biome(pos);
    let config = if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_WARM_VARIANT_FARM_ANIMALS) {
        WARM_SHEEP_COLORS
    } else if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_COLD_VARIANT_FARM_ANIMALS) {
        COLD_SHEEP_COLORS
    } else {
        TEMPERATE_SHEEP_COLORS
    };

    match pick_weighted(config) {
        ColorEntry::Single(color) => color,
        ColorEntry::Common(dominant) => {
            // The nested list is a second, independent draw — vanilla's provider chain
            // calls `get(random)` on whatever the outer draw returned.
            match pick_weighted(&[
                (ColorEntry::Single(dominant), 499),
                (ColorEntry::Single(PINK), 1),
            ]) {
                ColorEntry::Single(color) => color,
                ColorEntry::Common(color) => color,
            }
        }
    }
}

// -- Tropical fish ---------------------------------------------------------------------

/// `TropicalFish.Base`: the two body shapes, which occupy the low byte of the pattern id.
const FISH_BASE_SMALL: i32 = 0;
const FISH_BASE_LARGE: i32 = 1;

/// `TropicalFish.Pattern` (TropicalFish.java:284): `base.id | index << 8`.
const fn fish_pattern(base: i32, index: i32) -> i32 {
    base | (index << 8)
}

/// `TropicalFish.Pattern` constants (TropicalFish.java:284), declaration order.
const KOB: i32 = fish_pattern(FISH_BASE_SMALL, 0);
const SUNSTREAK: i32 = fish_pattern(FISH_BASE_SMALL, 1);
const SNOOPER: i32 = fish_pattern(FISH_BASE_SMALL, 2);
const DASHER: i32 = fish_pattern(FISH_BASE_SMALL, 3);
const BRINELY: i32 = fish_pattern(FISH_BASE_SMALL, 4);
const SPOTTY: i32 = fish_pattern(FISH_BASE_SMALL, 5);
const FLOPPER: i32 = fish_pattern(FISH_BASE_LARGE, 0);
const STRIPEY: i32 = fish_pattern(FISH_BASE_LARGE, 1);
const GLITTER: i32 = fish_pattern(FISH_BASE_LARGE, 2);
const BLOCKFISH: i32 = fish_pattern(FISH_BASE_LARGE, 3);
const BETTY: i32 = fish_pattern(FISH_BASE_LARGE, 4);
const CLAYFISH: i32 = fish_pattern(FISH_BASE_LARGE, 5);

/// `Pattern.values()` order — the uniform draw in `finalizeSpawn` walks this array.
const FISH_PATTERNS: [i32; 12] = [
    KOB, SUNSTREAK, SNOOPER, DASHER, BRINELY, SPOTTY, FLOPPER, STRIPEY, GLITTER, BLOCKFISH,
    BETTY, CLAYFISH,
];

/// `TropicalFish.packVariant` (TropicalFish.java:84).
pub const fn pack_fish_variant(pattern: i32, base_color: u8, pattern_color: u8) -> i32 {
    (pattern & 0xFFFF) | ((base_color as i32 & 0xFF) << 16) | ((pattern_color as i32 & 0xFF) << 24)
}

/// `TropicalFish.COMMON_VARIANTS` (TropicalFish.java:50), already packed. These are the
/// twenty-two named fish; 90% of spawns take one of them.
pub const COMMON_FISH_VARIANTS: [i32; 22] = [
    pack_fish_variant(STRIPEY, ORANGE, GRAY),
    pack_fish_variant(FLOPPER, GRAY, GRAY),
    pack_fish_variant(FLOPPER, GRAY, BLUE),
    pack_fish_variant(CLAYFISH, WHITE, GRAY),
    pack_fish_variant(SUNSTREAK, BLUE, GRAY),
    pack_fish_variant(KOB, ORANGE, WHITE),
    pack_fish_variant(SPOTTY, PINK, LIGHT_BLUE),
    pack_fish_variant(BLOCKFISH, PURPLE, YELLOW),
    pack_fish_variant(CLAYFISH, WHITE, RED),
    pack_fish_variant(SPOTTY, WHITE, YELLOW),
    pack_fish_variant(GLITTER, WHITE, GRAY),
    pack_fish_variant(CLAYFISH, WHITE, ORANGE),
    pack_fish_variant(DASHER, CYAN, PINK),
    pack_fish_variant(BRINELY, LIME, LIGHT_BLUE),
    pack_fish_variant(BETTY, RED, WHITE),
    pack_fish_variant(SNOOPER, GRAY, RED),
    pack_fish_variant(BLOCKFISH, RED, WHITE),
    pack_fish_variant(FLOPPER, WHITE, YELLOW),
    pack_fish_variant(KOB, RED, WHITE),
    pack_fish_variant(SUNSTREAK, GRAY, WHITE),
    pack_fish_variant(DASHER, CYAN, YELLOW),
    pack_fish_variant(FLOPPER, YELLOW, YELLOW),
];

/// `TropicalFish.finalizeSpawn` (TropicalFish.java:236). 90% of fish take one of the
/// twenty-two common variants; the rest roll pattern and both colours independently,
/// which is where the "rare" fish come from.
///
/// Vanilla shares one variant across a school via `TropicalFishGroupData`. Group data is
/// not threaded through this codebase's spawner yet, so each fish rolls its own — the
/// distribution is right, but a school is not guaranteed to match.
pub fn random_fish_variant() -> i32 {
    let mut rng = rand::rng();
    if rng.random_range(0.0..1.0) < 0.9 {
        COMMON_FISH_VARIANTS[rng.random_range(0..COMMON_FISH_VARIANTS.len())]
    } else {
        let pattern = FISH_PATTERNS[rng.random_range(0..FISH_PATTERNS.len())];
        let base_color = rng.random_range(0..16u8);
        let pattern_color = rng.random_range(0..16u8);
        pack_fish_variant(pattern, base_color, pattern_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_the_default_tropical_fish_variant_like_vanilla() {
        // TropicalFish.DEFAULT_VARIANT is kob/white/white, which packs to 0: the kob
        // pattern is small base 0 index 0, and white is DyeColor id 0.
        assert_eq!(pack_fish_variant(KOB, WHITE, WHITE), 0);
    }

    #[test]
    fn packs_pattern_base_and_colors_into_separate_fields() {
        // clayfish = large base (1) index 5 => 0x0501; base red (14), pattern white (0).
        let packed = pack_fish_variant(CLAYFISH, RED, WHITE);
        assert_eq!(packed & 0xFFFF, 0x0501);
        assert_eq!((packed >> 16) & 0xFF, 14);
        assert_eq!((packed >> 24) & 0xFF, 0);
    }

    #[test]
    fn sheep_weight_tables_total_one_hundred() {
        // Each configuration is a percentage table; if one drifts off 100 the nested
        // "common colours" branch stops being the 82% case vanilla documents.
        for config in [
            TEMPERATE_SHEEP_COLORS,
            WARM_SHEEP_COLORS,
            COLD_SHEEP_COLORS,
        ] {
            assert_eq!(config.iter().map(|(_, w)| *w).sum::<i32>(), 100);
        }
    }

    #[test]
    fn weighted_pick_stays_inside_the_table() {
        // pick_weighted must never fall through to its guard return; run it enough times
        // that every bucket is hit.
        for _ in 0..1000 {
            let picked = pick_weighted(TEMPERATE_SHEEP_COLORS);
            match picked {
                ColorEntry::Single(c) => {
                    assert!([BLACK, GRAY, LIGHT_GRAY, BROWN].contains(&c));
                }
                ColorEntry::Common(c) => assert_eq!(c, WHITE),
            }
        }
    }

    #[test]
    fn temperature_variant_names_round_trip() {
        for variant in [
            TEMPERATURE_VARIANT_COLD,
            TEMPERATURE_VARIANT_TEMPERATE,
            TEMPERATURE_VARIANT_WARM,
        ] {
            assert_eq!(
                temperature_variant_from_name(temperature_variant_name(variant)),
                variant
            );
        }
        // Unqualified names are accepted too, because that is what older saves hold.
        assert_eq!(temperature_variant_from_name("warm"), TEMPERATURE_VARIANT_WARM);
    }

    #[test]
    fn mixes_dyes_through_the_real_crafting_recipes() {
        // These are vanilla's two-dye recipes, so a lamb of two such parents takes the
        // mixed colour rather than one parent's. If the recipe table shape ever changes
        // under us this stops finding them and silently degrades to the coin flip.
        assert_eq!(mix_dye_colors(BLUE, GREEN), Some(CYAN));
        assert_eq!(mix_dye_colors(RED, YELLOW), Some(ORANGE));
        assert_eq!(mix_dye_colors(RED, WHITE), Some(PINK));
        assert_eq!(mix_dye_colors(BLACK, WHITE), Some(GRAY));
        // Order must not matter: the recipe is shapeless.
        assert_eq!(mix_dye_colors(GREEN, BLUE), Some(CYAN));
    }

    #[test]
    fn falls_back_to_a_parent_when_no_recipe_mixes_the_pair() {
        // Two of the same colour have no mixing recipe, so the result is that colour
        // either way the coin lands.
        assert_eq!(mix_dye_colors(RED, RED), None);
        assert_eq!(mixed_sheep_color(RED, RED), RED);
    }

    #[test]
    fn inherited_variant_always_comes_from_one_parent() {
        for _ in 0..200 {
            let child = inherit_variant(TEMPERATURE_VARIANT_COLD, TEMPERATURE_VARIANT_WARM);
            assert!(
                child == TEMPERATURE_VARIANT_COLD || child == TEMPERATURE_VARIANT_WARM,
                "a calf must take one parent's variant, never a third value"
            );
        }
    }
}

// -- Breeding ---------------------------------------------------------------------------

/// `DyeColor.getMixedColor` (DyeColor.java): the two parents' colours are looked up as a
/// two-dye crafting recipe — blue + yellow gives a green lamb — and only when no recipe
/// exists does it fall back to a coin flip between them.
///
/// Vanilla searches the live recipe manager. This walks the same recipe table, which is
/// the generated snapshot of it, so a datapack that adds a dye recipe is not picked up.
pub fn mixed_sheep_color(first: u8, second: u8) -> u8 {
    mix_dye_colors(first, second).unwrap_or_else(|| {
        if rand::rng().random_bool(0.5) {
            first
        } else {
            second
        }
    })
}

/// `DyeColor.findColorMixInRecipes`: a shapeless recipe taking exactly the two dyes,
/// whose result is itself a dye.
fn mix_dye_colors(first: u8, second: u8) -> Option<u8> {
    use pumpkin_data::recipes::{CraftingRecipeTypes, RECIPES_CRAFTING, RecipeIngredientTypes};

    let wanted = [dye_item_name(first)?, dye_item_name(second)?];
    for recipe in RECIPES_CRAFTING {
        let CraftingRecipeTypes::CraftingShapeless {
            ingredients,
            result,
            ..
        } = recipe
        else {
            continue;
        };
        if ingredients.len() != 2 {
            continue;
        }
        let named: Vec<&str> = ingredients
            .iter()
            .filter_map(|ingredient| match ingredient {
                RecipeIngredientTypes::Simple(name) => Some(*name),
                // A dye-mixing recipe never takes a tag or a choice of items.
                RecipeIngredientTypes::Tagged(_) | RecipeIngredientTypes::OneOf(_) => None,
            })
            .collect();
        if named.len() != 2 {
            continue;
        }
        // Shapeless: either order counts as a match.
        let matches = (named[0] == wanted[0] && named[1] == wanted[1])
            || (named[0] == wanted[1] && named[1] == wanted[0]);
        if matches
            && let Some(color) = dye_color_from_item_name(result.id)
        {
            return Some(color);
        }
    }
    None
}

/// The namespaced dye item for a `DyeColor` id.
fn dye_item_name(color: u8) -> Option<&'static str> {
    Some(match color {
        0 => "minecraft:white_dye",
        1 => "minecraft:orange_dye",
        2 => "minecraft:magenta_dye",
        3 => "minecraft:light_blue_dye",
        4 => "minecraft:yellow_dye",
        5 => "minecraft:lime_dye",
        6 => "minecraft:pink_dye",
        7 => "minecraft:gray_dye",
        8 => "minecraft:light_gray_dye",
        9 => "minecraft:cyan_dye",
        10 => "minecraft:purple_dye",
        11 => "minecraft:blue_dye",
        12 => "minecraft:brown_dye",
        13 => "minecraft:green_dye",
        14 => "minecraft:red_dye",
        15 => "minecraft:black_dye",
        _ => return None,
    })
}

fn dye_color_from_item_name(name: &str) -> Option<u8> {
    (0..16u8).find(|color| dye_item_name(*color) == Some(name))
}

/// `Cow`/`Pig`/`Chicken.getBreedOffspring`: the calf takes one parent's variant at random.
pub fn inherit_variant(first: u8, second: u8) -> u8 {
    if rand::rng().random_bool(0.5) {
        first
    } else {
        second
    }
}
