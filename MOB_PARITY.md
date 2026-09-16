# Mob parity work — local vanilla 26.2

The user requested mobs next after the bounded block handler pass. Open block
engine dependencies and the original item queue remain in PARITY_PLAN.md.
Source-reviewed does not mean verified 1:1 gameplay. Use background tests while
continuing independent work, and reuse SPAWNING_PARITY.md and FLIGHT_PARITY.md.

## Batches

| Batch | Scope | Status |
| --- | --- | --- |
| M01 | Cow, pig, sheep, chicken | Initial behavior fixes; full species pass open |
| M02 | Remaining passive land mobs and breeding/taming | Pending; reuse prior age/bee work |
| M03 | Aquatic/flying mobs and movement | Pending; reuse flight work |
| M04 | Monsters, targeting and combat | Pending; reuse spawning/AI work |
| M05 | Villagers, golems, bosses and special mobs | Pending |
| M06 | Shared lifecycle, registry, data, drops and verification | Pending |

## M01 first changes

- Cow, pig, sheep and chicken wandering selects WaterAvoidingRandomStrollGoal.
- Cow/sheep/chicken temptation uses their food tags; removed static food fallbacks.
  Pig carrot-on-a-stick and food-tag goals remain separate at priority four,
  preserving their individual cooldowns and selection order.
- Chicken egg timer reads retain the constructor's randomized value when absent;
  ticking excludes dead/removed chickens. Built-in cold/warm/temperate variants
  produce blue/brown/ordinary eggs at the entity position, with egg sound and
  attributed ENTITY_PLACE. Drop plugin cancellation/item/count are honored.
- Pig defaults to the classic sound variant. Cow/pig/chicken sound variants save
  registry names, accept vanilla names on load and retain valid legacy numeric IDs.
  Spawn sound selection uses the level random stream, as the source does.
- Background entity test run 1: 157 passed, 0 failed.
- Pig mounting rejects occupied vehicles and secondary-use interactions. Sheep
  shearing ejects separate wool items one block above the sheep with the source
  velocity draws, then marks the sheep sheared and emits the event before tool wear.
  Already-sheared/baby sheep still consume a shears interaction.
- Sources: AbstractCow.registerGoals, Pig.registerGoals, Sheep.registerGoals,
  Chicken.registerGoals/aiStep/readAdditionalSaveData, gameplay/chicken_lay.json.

## Remaining M01/shared gaps

- Per-entity Java random streams (current species/goal code uses thread RNG),
  variant selection and dynamic registry behavior.
- Replace built-in egg selection with full reloadable gift-loot evaluation once
  entity-component predicates and random-sequence contexts are supported.
- Complete shearing/dye/milk/saddle/breeding/death-drop and effect review; shared
  steering, pathfinding, goal timing and metadata/protocol verification.
- Existing tests verify selected helpers, not full species behavior.

- Latest background entity run 3: **157 passed, 0 failed**, including the final
  sound-persistence and shearing/mounting changes. No gameplay parity claim.
