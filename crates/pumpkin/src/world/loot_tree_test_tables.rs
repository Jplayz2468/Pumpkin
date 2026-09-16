/* This file is generated. Do not edit manually. */
use pumpkin_util::loot_table::*;
pub static TABLES: &[LootTable] = &[
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:diamond"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 0.3f32 },
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:coal"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 0.7f32 },
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:gold_ingot"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Uniform(
                &LootNumberProvider::Constant(2f32),
                &LootNumberProvider::Constant(5f32),
            ),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Sequence(&[
                            LootEntry {
                                kind: LootEntryKind::Item("minecraft:diamond"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 0.7f32 },
                                functions: &[],
                            },
                            LootEntry {
                                kind: LootEntryKind::Item("minecraft:coal"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 0.4f32 },
                                functions: &[],
                            },
                            LootEntry {
                                kind: LootEntryKind::Item("minecraft:gold_ingot"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 1f32 },
                                functions: &[],
                            },
                        ]),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(4f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Group(&[]),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(5f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Group(&[LootEntry {
                            kind: LootEntryKind::Item("minecraft:diamond"),
                            weight: 1i32,
                            quality: 0i32,
                            condition: LootCondition::RandomChance { chance: 0.35f32 },
                            functions: &[],
                        }]),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(5f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Group(&[
                            LootEntry {
                                kind: LootEntryKind::Item("minecraft:diamond"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 0.35f32 },
                                functions: &[],
                            },
                            LootEntry {
                                kind: LootEntryKind::Item("minecraft:coal"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 0.35f32 },
                                functions: &[],
                            },
                        ]),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(5f32),
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
                kind: LootEntryKind::InlineTable(&LootTable {
                    random_sequence: Some("minecraft:ignored_for_nested_context"),
                    pools: &[
                        LootPool {
                            entries: &[
                                LootEntry {
                                    kind: LootEntryKind::Item("minecraft:diamond"),
                                    weight: 1i32,
                                    quality: 0i32,
                                    condition: LootCondition::RandomChance { chance: 1f32 },
                                    functions: &[
                                        LootFunction {
                                            condition: LootCondition::None,
                                            kind: LootFunctionKind::SetCount {
                                                count: LootNumberProvider::Uniform(
                                                    &LootNumberProvider::Constant(2f32),
                                                    &LootNumberProvider::Constant(8f32),
                                                ),
                                                add: false,
                                            },
                                        },
                                        LootFunction {
                                            condition: LootCondition::None,
                                            kind: LootFunctionKind::SetCount {
                                                count: LootNumberProvider::Uniform(
                                                    &LootNumberProvider::Constant(-1.8f32),
                                                    &LootNumberProvider::Constant(2.3f32),
                                                ),
                                                add: true,
                                            },
                                        },
                                    ],
                                },
                                LootEntry {
                                    kind: LootEntryKind::Item("minecraft:coal"),
                                    weight: 1i32,
                                    quality: 0i32,
                                    condition: LootCondition::RandomChance { chance: 0.5f32 },
                                    functions: &[],
                                },
                            ],
                            rolls: LootNumberProvider::Binomial(
                                &LootNumberProvider::Constant(5f32),
                                &LootNumberProvider::Constant(0.6f32),
                            ),
                            bonus_rolls: LootNumberProvider::Constant(0.0),
                            condition: LootCondition::None,
                            functions: &[LootFunction {
                                condition: LootCondition::None,
                                kind: LootFunctionKind::SetCount {
                                    count: LootNumberProvider::Uniform(
                                        &LootNumberProvider::Constant(0f32),
                                        &LootNumberProvider::Constant(3f32),
                                    ),
                                    add: true,
                                },
                            }],
                        },
                        LootPool {
                            entries: &[LootEntry {
                                kind: LootEntryKind::Item("minecraft:gold_ingot"),
                                weight: 1i32,
                                quality: 0i32,
                                condition: LootCondition::RandomChance { chance: 1f32 },
                                functions: &[],
                            }],
                            rolls: LootNumberProvider::Constant(2f32),
                            bonus_rolls: LootNumberProvider::Constant(0.0),
                            condition: LootCondition::None,
                            functions: &[],
                        },
                    ],
                    functions: &[LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(0f32),
                                &LootNumberProvider::Constant(2f32),
                            ),
                            add: true,
                        },
                    }],
                }),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::SetCount {
                        count: LootNumberProvider::Uniform(
                            &LootNumberProvider::Constant(1f32),
                            &LootNumberProvider::Constant(3f32),
                        ),
                        add: true,
                    },
                }],
            }],
            rolls: LootNumberProvider::Constant(2f32),
            bonus_rolls: LootNumberProvider::Constant(0.0),
            condition: LootCondition::None,
            functions: &[LootFunction {
                condition: LootCondition::None,
                kind: LootFunctionKind::ExplosionDecay,
            }],
        }],
        functions: &[LootFunction {
            condition: LootCondition::None,
            kind: LootFunctionKind::SetCount {
                count: LootNumberProvider::Constant(1f32),
                add: true,
            },
        }],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[
                LootEntry {
                    kind: LootEntryKind::Item("minecraft:diamond"),
                    weight: 2i32,
                    quality: 3i32,
                    condition: LootCondition::RandomChance { chance: 0.65f32 },
                    functions: &[],
                },
                LootEntry {
                    kind: LootEntryKind::Item("minecraft:coal"),
                    weight: 4i32,
                    quality: -2i32,
                    condition: LootCondition::RandomChance { chance: 0.75f32 },
                    functions: &[],
                },
            ],
            rolls: LootNumberProvider::Uniform(
                &LootNumberProvider::Constant(1.2f32),
                &LootNumberProvider::Constant(3.8f32),
            ),
            bonus_rolls: LootNumberProvider::Uniform(
                &LootNumberProvider::Constant(0.5f32),
                &LootNumberProvider::Constant(2f32),
            ),
            condition: LootCondition::AnyOf(&[
                LootCondition::RandomChance { chance: 0.2f32 },
                LootCondition::Not(&LootCondition::RandomChance { chance: 0.4f32 }),
            ]),
            functions: &[],
        }],
        functions: &[],
    },
    LootTable {
        random_sequence: None,
        pools: &[LootPool {
            entries: &[LootEntry {
                kind: LootEntryKind::Item("minecraft:diamond"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::RandomChance { chance: 1f32 },
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Constant(300f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::LimitCount {
                            min: None,
                            max: Some(LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(3f32),
                                &LootNumberProvider::Constant(10f32),
                            )),
                        },
                    },
                    LootFunction {
                        condition: LootCondition::RandomChance { chance: 0.5f32 },
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Binomial(
                                &LootNumberProvider::Constant(12f32),
                                &LootNumberProvider::Constant(0.45f32),
                            ),
                            add: true,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::ExplosionDecay,
                    },
                ],
            }],
            rolls: LootNumberProvider::Constant(4f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Dynamic("minecraft:sherds"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[LootFunction {
                            condition: LootCondition::None,
                            kind: LootFunctionKind::SetCount {
                                count: LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(8f32),
                                ),
                                add: false,
                            },
                        }],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(2f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:diamond"),
                        weight: 0i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(4f32),
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
                kind: LootEntryKind::Alternatives(&[
                    LootEntry {
                        kind: LootEntryKind::Empty,
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::None,
                        functions: &[],
                    },
                    LootEntry {
                        kind: LootEntryKind::Item("minecraft:emerald"),
                        weight: 1i32,
                        quality: 0i32,
                        condition: LootCondition::RandomChance { chance: 1f32 },
                        functions: &[],
                    },
                ]),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::None,
                functions: &[],
            }],
            rolls: LootNumberProvider::Constant(4f32),
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
                kind: LootEntryKind::Item("minecraft:diamond"),
                weight: 1i32,
                quality: 0i32,
                condition: LootCondition::RandomChance { chance: 1f32 },
                functions: &[LootFunction {
                    condition: LootCondition::None,
                    kind: LootFunctionKind::SetCount {
                        count: LootNumberProvider::Constant(300f32),
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
];
