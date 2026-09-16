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
                    kind: LootFunctionKind::SetName {
                        name_json: Some("\"café 🐝\""),
                        item_name: false,
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
                    kind: LootFunctionKind::SetName {
                        name_json: Some(
                            "{\"text\":\"styled\",\"color\":\"red\",\"bold\":true,\"italic\":false,\"extra\":[\" plain\",{\"text\":\" suffix\",\"underlined\":true}]}",
                        ),
                        item_name: false,
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
                    kind: LootFunctionKind::SetName {
                        name_json: Some(
                            "{\"translate\":\"example.message\",\"with\":[\"arg\",{\"text\":\"two\",\"italic\":true}],\"color\":\"gold\"}",
                        ),
                        item_name: true,
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
                    kind: LootFunctionKind::SetName {
                        name_json: Some(
                            "{\"text\":\"styled\",\"color\":\"red\",\"bold\":true,\"italic\":false,\"extra\":[\" plain\",{\"text\":\" suffix\",\"underlined\":true}]}",
                        ),
                        item_name: true,
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
                    kind: LootFunctionKind::SetName {
                        name_json: Some(
                            "[{\"text\":\"one\",\"color\":\"blue\",\"extra\":[\" first\"]},{\"text\":\"two\",\"bold\":true}]",
                        ),
                        item_name: true,
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
                    kind: LootFunctionKind::SetName {
                        name_json: None,
                        item_name: false,
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
                    kind: LootFunctionKind::SetName {
                        name_json: Some("\"\""),
                        item_name: true,
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
                        kind: LootFunctionKind::SetName {
                            name_json: Some(
                                "{\"text\":\"styled\",\"color\":\"red\",\"bold\":true,\"italic\":false,\"extra\":[\" plain\",{\"text\":\" suffix\",\"underlined\":true}]}",
                            ),
                            item_name: true,
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Tag(
                        "minecraft:regular_goat_horns",
                    )),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Tag(
                        "minecraft:screaming_goat_horns",
                    )),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Tag(
                        "minecraft:goat_horns",
                    )),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:sing_goat_horn",
                        "minecraft:ponder_goat_horn",
                        "minecraft:sing_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[])),
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
                        kind: LootFunctionKind::SetInstrument(LootRegistrySet::Tag(
                            "minecraft:goat_horns",
                        )),
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
                        kind: LootFunctionKind::SetInstrument(LootRegistrySet::Tag(
                            "minecraft:goat_horns",
                        )),
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetName {
                            name_json: Some(
                                "{\"translate\":\"example.message\",\"with\":[\"arg\",{\"text\":\"two\",\"italic\":true}],\"color\":\"gold\"}",
                            ),
                            item_name: true,
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:admire_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:call_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:dream_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:feel_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:ponder_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:seek_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:sing_goat_horn",
                    ])),
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
                    kind: LootFunctionKind::SetInstrument(LootRegistrySet::Values(&[
                        "minecraft:yearn_goat_horn",
                    ])),
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
