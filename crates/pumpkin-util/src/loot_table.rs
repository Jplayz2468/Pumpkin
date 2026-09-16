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
    AnyOf(&'static [Self]),
    Not(&'static Self),
    ToolItems(&'static [&'static str]),

    /// `minecraft:entity_properties` on `this` with `flags.is_baby`.
    ThisIsBaby(bool),
    EntityPresent(EntityTarget),
    EntityMainhandHasEnchantments(EntityTarget),
    EntityOnFire {
        target: EntityTarget,
        expected: bool,
    },
    EntityMainhandEnchantment {
        target: EntityTarget,
        enchantment: &'static str,
    },
    /// `minecraft:entity_properties` on `this` with `vehicle.entity_type`.
    ThisVehicleIs(&'static str),
    /// `minecraft:entity_properties` matching an entity type, e.g. the creeper's
    /// "killed by a skeleton" music-disc pool.
    EntityTypeMatches {
        /// Which entity the predicate is about: `this`, `attacker`, or `direct_attacker`.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Number providers preserve both integer and float sampling semantics.
#[derive(Clone, Copy, Debug)]
pub enum LootNumberProvider {
    Constant(f32),
    Uniform(&'static Self, &'static Self),
    Binomial(&'static Self, &'static Self),
}

#[derive(Clone, Copy, Debug)]
pub enum LootEntryKind {
    Item(&'static str),
    Empty,
    Alternatives(&'static [LootEntry]),
    Sequence(&'static [LootEntry]),
    Group(&'static [LootEntry]),
    TableReference(&'static str),
    InlineTable(&'static LootTable),
    Tag {
        items: &'static [&'static str],
        expand: bool,
    },
    Dynamic(&'static str),
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub enum LootRegistrySet {
    All,
    Tag(&'static str),
    Values(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub enum LootFunctionKind {
    SetInstrument(LootRegistrySet),
    /// Canonical NBT exported through Java DataComponentPatch.CODEC.
    SetComponents(&'static [u8]),
    SetName { name_json: Option<&'static str>, item_name: bool },
    EnchantRandomly {
        options: LootRegistrySet,
        only_compatible: bool,
        include_additional_cost: bool,
    },
    EnchantWithLevels {
        levels: LootNumberProvider,
        options: LootRegistrySet,
        include_additional_cost: bool,
    },
    SetEnchantments {
        enchantments: &'static [(&'static str, LootNumberProvider)],
        add: bool,
    },
    SetCount {
        count: LootNumberProvider,
        add: bool,
    },
    LimitCount {
        min: Option<LootNumberProvider>,
        max: Option<LootNumberProvider>,
    },
    ApplyBonus {
        enchantment: &'static str,
        formula: LootBonusFormula,
    },
    EnchantedCountIncrease {
        enchantment: &'static str,
        count: LootNumberProvider,
        limit: i32,
    },
    ExplosionDecay,
    FurnaceSmelt {
        use_input_count: bool,
    },
    SetStewEffect(&'static [(&'static str, LootNumberProvider)]),
    SetOminousBottleAmplifier(LootNumberProvider),
    SetDamage {
        damage: LootNumberProvider,
        add: bool,
    },
    SetPotion(&'static str),
    CopyComponents {
        source: &'static str,
        include: Option<&'static [&'static str]>,
        exclude: &'static [&'static str],
    },
    CopyState {
        properties: &'static [(&'static str, u32)],
    },
    ExplorationMap {
        /// Structure tag naming the search targets, e.g. `#minecraft:on_treasure_maps`.
        destination: &'static str,
        /// Map decoration registry name placed on the located structure.
        decoration: &'static str,
        zoom: i8,
        /// Search radius in chunk-region rings.
        search_radius: i32,
        skip_existing_chunks: bool,
    },
    /// Retain unsupported functions explicitly for the remaining component/function work.
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct LootFunction {
    pub condition: LootCondition,
    pub kind: LootFunctionKind,
}

/// An entry container. Composite entries expand into weighted singleton candidates.
#[derive(Clone, Copy, Debug)]
pub struct LootEntry {
    pub kind: LootEntryKind,
    pub weight: i32,
    pub quality: i32,
    pub condition: LootCondition,
    pub functions: &'static [LootFunction],
}

#[derive(Clone, Copy, Debug)]
pub struct LootPool {
    pub entries: &'static [LootEntry],
    pub rolls: LootNumberProvider,
    pub bonus_rolls: LootNumberProvider,
    pub condition: LootCondition,
    pub functions: &'static [LootFunction],
}

#[derive(Clone, Copy, Debug)]
pub struct LootTable {
    pub random_sequence: Option<&'static str>,
    pub pools: &'static [LootPool],
    pub functions: &'static [LootFunction],
}

pub type ChestLootEntry = LootEntry;
pub type ChestLootPool = LootPool;
pub type ChestLootTable = LootTable;
