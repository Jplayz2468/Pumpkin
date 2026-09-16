# Unfinished parity work — survival priorities

Updated: 2026-09-16. Reference: Java Edition 26.2.

## Most important: match these before calling a survival session ready

This is the recommended order for the next work. Match Java's **ordinary player-visible
behavior** in these scenarios, including after reconnect/restart. Some code already
exists; an unchecked row means implementation or end-to-end evidence is still missing,
not that the entire feature is absent.

| Priority | Required survival behavior | Why it matters / remaining work | Close it with |
| --- | --- | --- | --- |
| **P0.1** | **Save/reload without lost or duplicated progress** | Save ordering, snapshots and retries have fixes, but live unload/reload, disconnect, concurrent saves and failure recovery remain unproven. | Two players store/move items, travel, disconnect and restart; compare inventories, blocks, entities, vehicles and pending work. Exercise a recoverable save failure. |
| **P0.2** | **Inventory, drops and crafting conserve items** | Full crafting/processing menu algorithms, recipe components/remainders, shift-click/overflow and transfer atomicity remain open. | Mine → collect → craft → smelt → store → transfer → break container; compare outputs, costs, remainders and total item counts with Java. |
| **P0.3** | **Combat, health and survival effects behave correctly** | Attribute/damage fixes are committed; complete live combat, equipment swaps, effects, blocking and projectile interactions are not certified. Hunger, eating, death/respawn and recovery need a combined gameplay audit. | Compare sword/bow/shield fights, armor wear, Strength/Weakness, eating, poison, drowning, burning and fall damage; verify death drops, XP and respawn. |
| **P0.4** | **Normal movement and building are reliable** | Shared collision/rays/falls are tested; specialized movement, contextual shapes and client integration still need proof. | Walk/jump/swim/climb/build around slabs, stairs, fences, scaffolding, water, ladders and moving blocks. Check hit targeting, placement, break drops and landing damage. |
| **P0.5** | **World time, chunks and common farms keep working** | Cross-chunk update order, block-entity integration and activation/readiness remain open despite scheduler/clock fixes. | Compare crops, fluids, furnaces, hoppers and a basic piston/redstone farm while crossing chunk boundaries, unloading, returning and restarting. No stalls or extra production. |
| **P0.6** | **Beds, portals and dimension travel preserve players and possessions** | Portal transfer/passenger code exists; complete transition timing, End return/credits/respawn, gateways and vehicle/client integration remain open. | Set spawn, die/respawn, travel to/from the Nether and End, reconnect during travel, and move a passenger vehicle. Check destination, ownership, inventory and duplicates. |
| **P0.7** | **Basic survival mobs make the world playable** | Common passive/hostile behavior, spawning/despawning, navigation, drops and multiplayer targeting still need full species/integration passes. **Queued; do not start until the user resumes mob work.** | Compare a day/night cycle: animals, breeding/food, hostile encounters, shelter, combat, drops and spawning around two players. |
| **P0.8** | **A real multiplayer client session stays synchronized** | Unit/oracle results do not establish menu, block, entity and vehicle packet behavior under play. | Run a short two-player Java session covering P0.1–P0.7; check ghost items/blocks, stale menus, duplicate entities and reconnect state. If Bedrock is offered, repeat its supported scenarios separately. |

**Not first-session blockers by themselves:** byte-identical formatting of malformed
custom locks, extreme floating-point fixtures, administrator/test-block tools, custom
registry reloads on a vanilla-only server, and cosmetic packet differences. Keep them
in the full parity backlog; promote any of them if an actual session exposes item loss,
incorrect progression, crashes or broken ordinary interactions.

Passing this survival gate is a useful milestone. It does **not** satisfy the original
objective of full core-engine/block parity with only mobs left, nor certify every item.

## Session findings — 2026-09-16 (working-survival triage)

- **Fixed (P0.1):** a brand-new world never persisted. `write_world_info` created
  `level.dat_new` inside a world folder that does not exist on first boot; the
  resulting `NotFound` is mapped to `WorldInfoError::InfoNotFound`, aborting the
  whole write before `data/` was created. Every restart then re-entered the
  "no level.dat, creating a new world" path, resetting spawn, day time, game
  rules, world border and weather; terrain only looked stable because the seed is
  pinned in config. Fixed by creating the level folder first.
  Regression: `world_info::anvil::test::creates_the_level_folder_for_a_brand_new_world`.
  The pre-existing tests missed this because they all write into a `TempDir` that
  already exists. Verified live: fresh boot writes `level.dat` plus all ten
  `data/minecraft/*.dat` files, and a restart loads the world with no error and
  produces the `level.dat_old` backup.
- **Known gap (P0.6 / exploration):** `minecraft:exploration_map` is
  `LootFunctionKind::Unsupported` and `world/loot.rs` silently drops unsupported
  functions, so `chests/shipwreck_map`, `chests/underwater_ruin_big` and
  `chests/underwater_ruin_small` yield a blank map. Buried treasure therefore has
  no ordinary discovery path. Needs structure locating plus map decoration.
- **WIP/failing, isolated:** the malformed container-lock predicate codec now lives
  on branch `codex/lock-predicate-codec-wip` with a patch under `logs/`. It was
  aborting `cargo test` before five of the six packages ran. Ordinary container-lock
  enforcement on this branch is unchanged. Its two recorded defects still stand:
  custom-data predicates are saved as an NBT compound where Java saves an SNBT
  string, and a partial decode failure discards all restrictions.
- **Unverified:** the server sent Set Compression to a smoke client although
  `[networking.java.compression] enabled = false`. Not yet distinguished from a
  config-edit mistake; recheck before treating it as a bug.
- **Not exercised this session:** backlog items P0.2-P0.6 and P0.8. No two-client
  session was run; no live-client evidence is claimed.

## Workstation parity oracles — 2026-09-16

Eight Java 26.2 differential probes now live in `tools/vanilla/`, sharing
`WorkstationSupport.java` (headless registry bootstrap, every item's real
component map, banner patterns) and a Rust `test_support` replay module.
None of the 49 pre-existing probes covered a workstation.

| Station | Coverage | Result |
| --- | --- | --- |
| Anvil | 2419 cases | all match |
| Crafting | 1056 recipes | **7 bugs fixed**, all match |
| Grindstone | 669 cases | all match |
| Smithing | 630 cases | all match |
| Enchanting | 648 offers | all match |
| Stonecutter | 65 inputs / 319 recipes | all match, order included |
| Loom | 11 pattern sources | all match |
| Cartography | no fixture | **known gap, see below** |

**Fixed:** seven shaped recipes were uncraftable. Vanilla pads some patterns to
a 3x3 box, such as the mace's `[" # ", " I "]`; Java's `ShapedRecipePattern`
trims that at parse time, codegen stored it raw, and `recipe_matches` compares a
pattern against the bounding box of the placed items, which can never be wider
than the items. Mace, spyglass, creaking_heart and the four waxed chiseled
copper variants could not be crafted at all. Codegen now trims, as Java does.

**Known gap — cartography map-state guards.** Java's `setupResultSlot` needs
`MapItemSavedData`: it makes no offer without saved data, refuses to zoom a
locked map or one already at scale 4, and refuses to lock an already-locked map.
`CartographyTableScreenHandler` is built from a sync id and the player inventory
only, so it cannot reach the server's `MapManager` and offers all three
unconditionally. A player can waste paper zooming a maximum-scale map or re-lock
a locked one. Fixing it needs the map store plumbed into the handler. Current
behaviour is pinned by
`cartography_table_screen_handler::java_parity_tests::cartography_is_missing_javas_map_state_guards`,
which fails once the guards land.

**Known gap — damage clamping.** Java clamps damage to `[0, maxDamage]` in both
`getDamageValue` and `setDamageValue`. `set_damage` clamps only the lower bound
and `get_damage` does not clamp, so anvil repair diverges for a stack whose
damage exceeds its maximum. Unreachable in ordinary survival; reachable through
`/give` or a datapack. Not changed because `get_damage` is a hot accessor.

**Not covered by these oracles:** recipe remainders (`RecipeResult` carries
none, so bucket and bottle returns are untested), the banner layers the loom
produces, the grindstone's random experience half, and every menu lifecycle
concern — shift-click, drag, menu close and result-slot take paths. Those need
the integration tests, not oracles.

## Status and evidence

- **Known gap:** an implementation limitation is recorded.
- **Needs live verification:** fixes exist, but the combined gameplay path is unproven.
- **Queued audit:** coverage is incomplete; this is not an assertion of a known bug.
- **WIP/failing:** local work is unfinished and must not be counted as complete.
- Last completed checkpoint: `e9d04337`, attribute components/equipment calculations.
  Full background run: **1,087 tests passed**, with two localhost/socket tests excluded
  (previously passed separately). Three focused integration tests passed after the
  final animal-armor slot correction.
- The Java comparison fixtures are thorough **within their individual scopes**.
  Large fixture counts are not a full-server, full-mob or full-item certification.
- All 222 block Rust source files have a reviewed/carried disposition. This is source
  coverage, not 222 completed blocks or a reliable overall completion percentage.

## P1 — finish after the survival essentials, or sooner when players use them

### Menus, automation and progression

- [ ] **Known gap / queued audit:** crafting, anvil/repair, smithing, grindstone,
  enchanting, loom, stonecutter and cartography output/cost/consumption algorithms.
  Enchanting selection is implemented; live bookshelf refresh and menu lifecycle remain.
- [ ] **Known gap:** crafter recipe assembly must preserve components/remainders and
  crafting callbacks; resolve bulk versus individual insertion behavior.
- [ ] **Known gap:** exact hopper transfer atomicity/order, obstruction checks and
  partial-pickup client synchronization; container removal/tick ordering.
- [ ] **Known gap / queued audit:** per-item dispenser behavior, including the recorded
  missing sulfur-cube handling; transport/container-entity interaction paths.
- [ ] **Needs live verification:** all container families' names, pending loot,
  viewer accounting, double-chest behavior, locks, menus and comparator updates.
- [ ] **Known gap:** guarded-container piglin anger and remaining nonplayer opener rules.
- [ ] **Known gap:** complete loot entity/item/block-entity/equipment contexts,
  predicates and reward/advancement producers. Matching loot JSON is insufficient.
- [ ] **Known gap:** the remaining three built-in unsupported loot-function declarations
  are `exploration_map`. Older logs listing 292/206/135/etc. are superseded counts.
- [ ] **Queued audit:** ordinary fishing, maps/navigation items and progression rewards.

### Redstone, movement and environmental integration

- [ ] **Known gap / needs live verification:** exact neighbor/scheduled update order
  at chunk boundaries, equal restored tick ties, block-entity ordering and Java
  neighbor-set order; experimental redstone orientation/evaluator behavior.
- [ ] **Needs live verification:** real piston/slime/honey contraptions, moving block
  completion, entity displacement and inside effects together. Geometry fixtures pass;
  the entire contraption lifecycle is not certified.
- [ ] **Known gap:** remaining contextual collision/support/conductor shapes,
  context-free ray callers, specialized/direct entity motion and entity-hit selection.
- [ ] **Known gap / needs live verification:** ridden navigation, controller authority,
  boat/minecart/mount physics and vehicle packet paths; leash/knot lifecycle.
- [ ] **Needs live verification:** fire/freezing, water/lava, fall resets and landing
  effects across actual movement, teleports, vehicles and nonliving entities.
- [ ] **Known gap:** feature-cache growth buffers callbacks/RNG instead of fully
  interleaving them; configured providers/height views, unloaded light access,
  liquid-container handling and double-plant mining/drop order remain.
- [ ] **Known gap / needs live verification:** sculk/worldgen stream interactions,
  vibration/client integration and bee/hive/warden dependencies. Earlier source work
  is preserved; complete species behavior is not certified.

### Specialized survival block systems

- [ ] **Known gap:** creaking-heart protector creation/removal, resin production,
  linked lifecycle and player-caused explosion XP.
- [ ] **Known gap:** fully rotated summon-pattern search, copper-golem summoning/chest
  conversion, summon advancements and wither body-yaw data.
- [ ] **Known gap / queued audit:** detailed spawner/trial-spawner algorithms and
  persistence, beacon effects/beam/menu/client behavior, malformed vault configuration.
  Vault timing/ejection/history code and targeted beacon fixes already exist.
- [ ] **Known gap:** gateway processor/pearl/exit-generation integration, End
  transition/credits/respawn details and portal spawn-finalization reasons.
- [ ] **Known gap / needs live verification:** sign/hanging-sign editing/chaining,
  filtered text/click actions/permissions, banner/skull/light item and pick-block
  components, profiles/sounds, rotations/mirrors and context-sensitive replacement.

## P2 — full fidelity, custom data, administration and uncommon edge cases

### Shared runtime and persistence

- [ ] Per-entity/player Java RNG ownership and exact random draw ordering, including
  sound/effect draws and process-dependent stochastic map iteration.
- [ ] Dynamic environment attributes and reloadable registries/tags/loot/recipes.
- [ ] Shared server-wide dynamic clock definitions and exact cross-dimension clock
  activation/pausing; remaining chunk-holder readiness and portal-expiry details.
- [ ] Full dependent-generation recovery and fatal I/O-worker recovery; save-command
  result/flush semantics, crash durability, atomic multi-file state and plugin/snapshot
  concurrency. Ordinary save/reconnect safety stays P0, not deferred here.
- [ ] Remaining linked-entity ownership/lifecycle behavior; TNT-minecart ignition
  damage-source snapshots. Ordinary TNT/projectile UUID ownership is already implemented.

### Components, text and exact arithmetic

- [ ] Remaining component decoders, validation, hashes and exact predicates, including
  valid/custom forms not covered by the completed component batches.
- [ ] Complete fallible item/component save-call migration and error reporting;
  nested failure/hash handling beyond the covered jukebox/container cases.
- [ ] Arbitrary inline sound identifier validation and dynamic registry-holder behavior.
- [ ] Raw comparator values outside the current `u8` interfaces.
- [ ] Java same-operation attribute modifier iteration order and shared modifier IDs
  across simultaneous equipment slots; generated prototype override display text.
- [ ] Unsupported text/hover forms, entity text resolution, filtering-service
  integration and remaining component-form names/command output.
- [ ] Remaining non-block particle payloads, sound-category/randomness details,
  cross-version metadata and Bedrock translation/rendering. Essential synchronization
  for any publicly supported client remains P0.8.

### Administrative blocks and exhaustive coverage

- [ ] Structure block SAVE/LOAD/CORNER operations, template application and mode/state
  initialization; jigsaw/test-instance editors and test-runner lifecycle.
- [ ] Command/editor protocol and remaining component output behavior. Success
  callbacks, tick guards, chain limits and disabled scheduling are already implemented.
- [ ] Game-master placement/components, barrier bucket restrictions, test-block cloning
  and invisible/contextual shapes.
- [ ] Exhaustive constructor/inheritance/runtime handler routing for every block ID,
  property/default comparison and remaining contextual collision/occlusion shapes.
- [ ] Final all-block/all-item live integration and cross-version coverage. Registry
  presence, static shape tests and matching data files alone do not close this gate.

## Stopped work: malformed container-lock decoding — WIP/failing

Do not count this work as finished or spend the next survival-focused batch polishing
it unless a real use case elevates its priority.

- Uncommitted implementation: `crates/pumpkin/src/item/predicate_codec.rs`, its module
  registration, and `crates/pumpkin/src/block/entities/container_lock.rs` integration.
- Reference/fixture: `tools/vanilla/LockCodecOracle.java` and
  `crates/pumpkin/src/item/lock_codec_cases.json` (286 cases).
- Java `TagValueInput` accepts partial codec results: preserve valid restrictions
  from malformed records instead of simply dropping the entire lock.
- Latest observed test: `/tmp/pumpkin-lock-4.log`,
  `item::predicate_codec::tests::container_lock_decode_fallback_and_evaluation_match_java`.
  It **fails**: `{predicates:{custom_data:'{a:1}'}}` saves custom data as a compound;
  Java saves the SNBT string `"{a:1}"`.
- Required follow-up: fix the representation, verify strict versus partial recovery,
  mixed valid/invalid fields and nested predicates, and assess inherited component
  codec limitations. Then run affected checks before committing these changes.
- The ordinary 12-family lock enforcement was committed earlier; this WIP concerns
  malformed/custom codec fidelity. The current checkout is not the all-green checkpoint.
- Preserve the separately dirty `crates/pumpkin-plugin-wit` submodule; do not stage it.

## Mob and item scope is still outstanding

- **Mobs remain paused.** Basic survival mob readiness is listed above for planning,
  not authorization to start a full species pass.
- Cow/pig/sheep/chicken have initial fixes; bees have substantial prior work. Full
  passive/breeding/taming, aquatic/flying, monster/combat, villager/golem/boss and
  shared lifecycle/drop/registry passes remain in [MOB_PARITY.md](MOB_PARITY.md).
- The full item inventory remains in [PARITY_PLAN.md](PARITY_PLAN.md): placement and
  interaction tools; crafting/processing/repair menus; food/potions/continuous use/
  equipment; weapons/projectiles/transport/spawning/maps/fishing; registry/component
  reconciliation. Existing item fixes do not close these whole batches.

## Spend effort on gameplay evidence first

1. Work down P0; choose one concrete failure or one missing session check at a time.
2. Reproduce the same actions on Java 26.2 and Pumpkin. Compare visible results,
   inventory counts and saved/reloaded state; record the actual discrepancy.
3. Fix the shared cause and add the smallest meaningful regression check. Run it in
   the background while doing useful independent work, as requested.
4. Avoid repeating broad suites or growing edge-case matrices without a new change,
   failure or unresolved risk that justifies them.
5. Close checklist entries only with evidence matching their scope. Keep known gaps
   separate from verification gaps; do not turn source review into a parity percentage.

## Detailed source records

This file consolidates and prioritizes the known remaining work; it is not a fresh
exhaustive audit. Later completed checkpoints supersede older “remaining” statements
in the historical logs. Newly discovered gaps should be added here.

- [ENGINE_GAPS.md](ENGINE_GAPS.md): shared engine changes, evidence and limits.
- [BLOCK_ITEM_PARITY.md](BLOCK_ITEM_PARITY.md): block-family implementation history.
- [PARITY_PLAN.md](PARITY_PLAN.md): B01–B14, D01–D07 and I01–I05 inventories.
- [PARITY_BLOCK_DATA_AUDIT.md](PARITY_BLOCK_DATA_AUDIT.md): data evidence and limits.
- [PARITY_VANILLA_BLOCK_SOURCES.md](PARITY_VANILLA_BLOCK_SOURCES.md): Java source coverage.
- [MOB_PARITY.md](MOB_PARITY.md), [SPAWNING_PARITY.md](SPAWNING_PARITY.md),
  [FLIGHT_PARITY.md](FLIGHT_PARITY.md): preserved mob/spawning/flight scope.
