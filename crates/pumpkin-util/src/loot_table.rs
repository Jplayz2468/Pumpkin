/// Conditions required for an entry or pool to be eligible for loot generation.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LootCondition {
    #[default]
    None,
    SilkTouch,
    NoSilkTouch,
    Shears,
    SilkTouchOrShears,
    NoSilkTouchOrShears,
    SurvivesExplosion,
    KilledByPlayer,
    RandomChance {
        chance: f32,
    },
    RandomChanceWithEnchantedBonus {
        unenchanted_chance: f32,
        enchanted_chance_base: f32,
        enchanted_chance_per_level_above_first: f32,
    },
    TableBonus {
        chances: &'static [f32],
    },
    AllOf(&'static [Self]),

    /// `minecraft:entity_properties` on `this` with `flags.is_baby`.
    ThisIsBaby(bool),
    /// `minecraft:entity_properties` on `this` with `vehicle.entity_type`.
    ThisVehicleIs(&'static str),
    /// `minecraft:entity_properties` matching an entity type, e.g. the creeper's
    /// "killed by a skeleton" music-disc pool.
    EntityTypeMatches {
        /// Which entity the predicate is about: vanilla's `"this"`, `"killer"` or
        /// `"direct_killer"`.
        target: EntityTarget,
        /// A registry name, or a `#tag` to be resolved by the evaluator.
        entity_type: &'static str,
    },
    /// `minecraft:entity_properties` with `type_specific/cube_mob.size` -- slime and
    /// magma cube size gating.
    ThisCubeSizeIs(i32),
    /// `minecraft:entity_properties` with `type_specific/raider.is_captain`.
    ThisIsRaidCaptain(bool),
    /// `minecraft:damage_source_properties` asserting a damage-type tag.
    DamageTypeHasTag {
        tag: &'static str,
        expected: bool,
    },

    /// `minecraft:block_state_property`: the broken block must carry these property
    /// values. Used by cave vines (berries), double plants (which half), and others.
    BlockStateProperties {
        block: &'static str,
        properties: &'static [(&'static str, &'static str)],
    },

    /// A condition the generator could not represent.
    ///
    /// It never passes. Before this existed, an unrepresentable condition was silently
    /// discarded, which left the pool *unconditional* -- so a pool gated on something rare
    /// dropped every single time. Failing closed is the safe direction: a missed rare drop
    /// is a much smaller error than a guaranteed wrong one.
    Unsupported,
}

/// Which entity an `entity_properties` condition is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityTarget {
    This,
    Killer,
    DirectKiller,
}

/// Bonus count formulas when tools have fortune or looting enchantments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LootBonusFormula {
    OreDrops,
    UniformBonusCount(i32),
    BinomialWithBonusCount { extra: i32, probability: f32 },
}

/// A single item entry inside a loot pool.
#[derive(Clone, Copy, Debug)]
pub struct LootEntry {
    /// Registry name of the item (e.g. `"minecraft:diamond"`), or empty for a
    /// `minecraft:empty` outcome. Empty outcomes retain their condition and order.
    pub item: &'static str,
    /// Relative probability weight; higher values are more likely.
    pub weight: i32,
    /// Minimum stack size (inclusive).
    pub min_count: i32,
    /// Maximum stack size (inclusive).
    pub max_count: i32,
    /// Condition required for this entry to be eligible.
    pub condition: LootCondition,
    /// Bonus formula to apply with fortune / looting (if any).
    pub bonus_formula: Option<LootBonusFormula>,
}

/// One roll pool inside a loot table.
#[derive(Clone, Copy, Debug)]
pub struct LootPool {
    /// Item entries eligible for selection each roll.
    pub entries: &'static [LootEntry],
    /// Minimum number of roll attempts (inclusive).
    pub min_rolls: i32,
    /// Maximum number of roll attempts (inclusive).
    pub max_rolls: i32,
    /// Condition required for this entire pool to run.
    pub condition: LootCondition,
}

/// A complete loot table consisting of one or more pools.
#[derive(Clone, Copy, Debug)]
pub struct LootTable {
    pub random_sequence: Option<&'static str>,
    /// All pools to roll when generating loot for this table.
    pub pools: &'static [LootPool],
}

pub type ChestLootEntry = LootEntry;
pub type ChestLootPool = LootPool;
pub type ChestLootTable = LootTable;
