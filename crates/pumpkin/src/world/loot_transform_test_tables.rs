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
                    kind: LootFunctionKind::FurnaceSmelt {
                        use_input_count: true,
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
                    kind: LootFunctionKind::FurnaceSmelt {
                        use_input_count: false,
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
                            count: LootNumberProvider::Constant(128f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::FurnaceSmelt {
                            use_input_count: true,
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
                        kind: LootFunctionKind::FurnaceSmelt {
                            use_input_count: true,
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
                    kind: LootFunctionKind::SetOminousBottleAmplifier(LootNumberProvider::Uniform(
                        &LootNumberProvider::Constant(-3f32),
                        &LootNumberProvider::Constant(8f32),
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
                    kind: LootFunctionKind::SetOminousBottleAmplifier(
                        LootNumberProvider::Constant(3.5f32),
                    ),
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
                        kind: LootFunctionKind::SetOminousBottleAmplifier(
                            LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(-3f32),
                                &LootNumberProvider::Constant(8f32),
                            ),
                        ),
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
                    kind: LootFunctionKind::SetStewEffect(&[
                        (
                            "minecraft:poison",
                            LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(1f32),
                                &LootNumberProvider::Constant(4f32),
                            ),
                        ),
                        (
                            "minecraft:saturation",
                            LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(1f32),
                                &LootNumberProvider::Constant(4f32),
                            ),
                        ),
                        (
                            "minecraft:instant_health",
                            LootNumberProvider::Constant(2f32),
                        ),
                        (
                            "minecraft:instant_damage",
                            LootNumberProvider::Constant(3f32),
                        ),
                    ]),
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
                        kind: LootFunctionKind::SetStewEffect(&[
                            (
                                "minecraft:poison",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:saturation",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:instant_health",
                                LootNumberProvider::Constant(2f32),
                            ),
                            (
                                "minecraft:instant_damage",
                                LootNumberProvider::Constant(3f32),
                            ),
                        ]),
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetStewEffect(&[
                            (
                                "minecraft:poison",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:saturation",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:instant_health",
                                LootNumberProvider::Constant(2f32),
                            ),
                            (
                                "minecraft:instant_damage",
                                LootNumberProvider::Constant(3f32),
                            ),
                        ]),
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
                    kind: LootFunctionKind::SetStewEffect(&[]),
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
                        kind: LootFunctionKind::SetStewEffect(&[
                            (
                                "minecraft:poison",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:saturation",
                                LootNumberProvider::Uniform(
                                    &LootNumberProvider::Constant(1f32),
                                    &LootNumberProvider::Constant(4f32),
                                ),
                            ),
                            (
                                "minecraft:instant_health",
                                LootNumberProvider::Constant(2f32),
                            ),
                            (
                                "minecraft:instant_damage",
                                LootNumberProvider::Constant(3f32),
                            ),
                        ]),
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
                        kind: LootFunctionKind::SetOminousBottleAmplifier(
                            LootNumberProvider::Constant(4f32),
                        ),
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::FurnaceSmelt {
                            use_input_count: true,
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
                        kind: LootFunctionKind::FurnaceSmelt {
                            use_input_count: true,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::SetOminousBottleAmplifier(
                            LootNumberProvider::Uniform(
                                &LootNumberProvider::Constant(0f32),
                                &LootNumberProvider::Constant(4f32),
                            ),
                        ),
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
                            count: LootNumberProvider::Constant(-1f32),
                            add: false,
                        },
                    },
                    LootFunction {
                        condition: LootCondition::None,
                        kind: LootFunctionKind::FurnaceSmelt {
                            use_input_count: true,
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:speed",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:slowness",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:haste",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:mining_fatigue",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:strength",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:instant_health",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:instant_damage",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:jump_boost",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:nausea",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:regeneration",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:resistance",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:fire_resistance",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:water_breathing",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:invisibility",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:blindness",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:night_vision",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:hunger",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:weakness",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:poison",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:wither",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:health_boost",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:absorption",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:saturation",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:glowing",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:levitation",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:luck",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:unluck",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:slow_falling",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:conduit_power",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:dolphins_grace",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:bad_omen",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:hero_of_the_village",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:darkness",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:trial_omen",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:raid_omen",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:wind_charged",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:weaving",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:oozing",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:infested",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
                    kind: LootFunctionKind::SetStewEffect(&[(
                        "minecraft:breath_of_the_nautilus",
                        LootNumberProvider::Constant(107374184f32),
                    )]),
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
