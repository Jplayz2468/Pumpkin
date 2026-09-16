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
                    kind: LootFunctionKind::SetDamage {
                        damage: LootNumberProvider::Uniform(
                            &LootNumberProvider::Constant(0f32),
                            &LootNumberProvider::Constant(1f32),
                        ),
                        add: false,
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
                    kind: LootFunctionKind::SetDamage {
                        damage: LootNumberProvider::Uniform(
                            &LootNumberProvider::Constant(-0.5f32),
                            &LootNumberProvider::Constant(0.5f32),
                        ),
                        add: true,
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
                        kind: LootFunctionKind::SetDamage {
                            damage: LootNumberProvider::Constant(0.2f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetDamage {
                            damage: LootNumberProvider::Constant(0.6f32),
                            add: true,
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
                        kind: LootFunctionKind::SetDamage {
                            damage: LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(0f32),
                                &LootNumberProvider::Constant(1f32),
                            ),
                            add: false,
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
                    kind: LootFunctionKind::SetPotion("minecraft:slowness"),
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
                        kind: LootFunctionKind::SetPotion("minecraft:healing"),
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetPotion("minecraft:poison"),
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
                    condition: LootCondition::None,
                    kind: LootFunctionKind::CopyState {
                        properties: &[("honey_level", 31u32), ("facing", 1u32)],
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
                    kind: LootFunctionKind::CopyState {
                        properties: &[("age", 49u32)],
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
                    kind: LootFunctionKind::CopyState {
                        properties: &[("waterlogged", 9u32), ("facing", 1u32)],
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
                    kind: LootFunctionKind::CopyState { properties: &[] },
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
                        kind: LootFunctionKind::SetPotion("minecraft:healing"),
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::CopyState {
                            properties: &[("honey_level", 31u32)],
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
