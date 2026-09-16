/* This file is generated. Do not edit manually. */
use pumpkin_util::loot_table::*;
pub static TABLES: &[LootTable] = &[
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "tool",
                        include: None,
                        exclude: &[],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "tool",
                        include: Some(&[]),
                        exclude: &[],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "tool",
                        include: Some(&["minecraft:custom_name", "minecraft:damage"]),
                        exclude: &["minecraft:custom_name"],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "tool",
                        include: None,
                        exclude: &["minecraft:damage", "minecraft:max_stack_size"],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "block_entity",
                        include: None,
                        exclude: &[],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyComponents {
                        source: "block_entity",
                        include: Some(&["minecraft:custom_name"]),
                        exclude: &[],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::CopyComponents {
                            source: "tool",
                            include: None,
                            exclude: &[],
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::CopyComponents {
                            source: "block_entity",
                            include: None,
                            exclude: &["minecraft:custom_name"],
                        },
                    },
                ],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::RandomChance { chance: 0.5f32 },
                    kind: LootFunctionKind::CopyComponents {
                        source: "tool",
                        include: None,
                        exclude: &[],
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Dynamic("minecraft:input"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Constant(0f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::CopyComponents {
                            source: "tool",
                            include: None,
                            exclude: &[],
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Constant(1f32),
                            add: false,
                        },
                    },
                ],
            }],
            rolls: LootNumberProvider::Constant(1f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[],
        }],
        functions: &[],
    },
];
