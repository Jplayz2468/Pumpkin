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
                    kind: LootFunctionKind::EnchantRandomly {
                        options: LootRegistrySet::All,
                        only_compatible: true,
                        include_additional_cost: false,
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
                    kind: LootFunctionKind::EnchantRandomly {
                        options: LootRegistrySet::Tag("minecraft:in_enchanting_table"),
                        only_compatible: true,
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantRandomly {
                        options: LootRegistrySet::Values(&[
                            "minecraft:sharpness",
                            "minecraft:unbreaking",
                            "minecraft:sharpness",
                        ]),
                        only_compatible: false,
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantRandomly {
                        options: LootRegistrySet::Values(&[]),
                        only_compatible: true,
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantRandomly {
                        options: LootRegistrySet::Values(&["minecraft:mending"]),
                        only_compatible: true,
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantWithLevels {
                        levels: LootNumberProvider::Uniform(
                            &LootNumberProvider::Constant(1f32),
                            &LootNumberProvider::Constant(50f32),
                        ),
                        options: LootRegistrySet::All,
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantWithLevels {
                        levels: LootNumberProvider::Constant(30f32),
                        options: LootRegistrySet::Tag("minecraft:in_enchanting_table"),
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantWithLevels {
                        levels: LootNumberProvider::Constant(0f32),
                        options: LootRegistrySet::Values(&[]),
                        include_additional_cost: true,
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
                    kind: LootFunctionKind::EnchantWithLevels {
                        levels: LootNumberProvider::Constant(30f32),
                        options: LootRegistrySet::Values(&[
                            "minecraft:thorns",
                            "minecraft:sharpness",
                            "minecraft:efficiency",
                        ]),
                        include_additional_cost: false,
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
                    kind: LootFunctionKind::SetEnchantments {
                        enchantments: &[(
                            "minecraft:sharpness",
                            LootNumberProvider::Constant(3f32),
                        )],
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
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[(
                                "minecraft:sharpness",
                                LootNumberProvider::Constant(3f32),
                            )],
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[(
                                "minecraft:sharpness",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(-5f32),
                                    &LootNumberProvider::Constant(260f32),
                                ),
                            )],
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
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[
                                ("minecraft:mending", LootNumberProvider::Constant(1f32)),
                                ("minecraft:sharpness", LootNumberProvider::Constant(300f32)),
                            ],
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[(
                                "minecraft:sharpness",
                                LootNumberProvider::Constant(0f32),
                            )],
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
                    kind: LootFunctionKind::SetEnchantments {
                        enchantments: &[],
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
                        kind: LootFunctionKind::EnchantRandomly {
                            options: LootRegistrySet::All,
                            only_compatible: false,
                            include_additional_cost: true,
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
                        kind: LootFunctionKind::EnchantWithLevels {
                            levels: LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(1f32),
                                &LootNumberProvider::Constant(50f32),
                            ),
                            options: LootRegistrySet::All,
                            include_additional_cost: true,
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
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[(
                                "minecraft:sharpness",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(5f32),
                                ),
                            )],
                            add: true,
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
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Constant(130f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetEnchantments {
                            enchantments: &[(
                                "minecraft:sharpness",
                                LootNumberProvider::Constant(2f32),
                            )],
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
                functions: &[
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetCount {
                            count: LootNumberProvider::Constant(130f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::EnchantRandomly {
                            options: LootRegistrySet::All,
                            only_compatible: true,
                            include_additional_cost: false,
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
                        kind: LootFunctionKind::EnchantRandomly {
                            options: LootRegistrySet::Values(&["minecraft:sharpness"]),
                            only_compatible: false,
                            include_additional_cost: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::EnchantRandomly {
                            options: LootRegistrySet::Values(&["minecraft:sharpness"]),
                            only_compatible: false,
                            include_additional_cost: false,
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
                        kind: LootFunctionKind::EnchantWithLevels {
                            levels: LootNumberProvider::Constant(30f32),
                            options: LootRegistrySet::Tag("minecraft:in_enchanting_table"),
                            include_additional_cost: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::EnchantWithLevels {
                            levels: LootNumberProvider::Constant(30f32),
                            options: LootRegistrySet::Tag("minecraft:in_enchanting_table"),
                            include_additional_cost: false,
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
