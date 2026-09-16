# Shared engine work — 2026-09-15

Current priority: fix block/shared-engine gaps before adding more mobs. Full mob
passes are paused. This is an implementation checkpoint, not a 1:1 certification.

## Implemented in this checkpoint

| Area | Concrete change |
| --- | --- |
| World block updates | MOVED propagates shapes; only SKIP_SHAPE_UPDATES suppresses them. Neighbor shape flags clear NOTIFY_NEIGHBORS and SKIP_DROPS. |
| Piston completion | Resolve neighbor shapes before checking air; restore unsupported moved blocks before destruction/drop handling; clear waterlogging; notify the completed block itself. |
| Moving collision | Actual moved-block/head shapes and stationary retracting bases, scoped direction-specific collision suppression, swept front faces, normal player pushes, slime velocity, honey carrying. |
| Entity displacement | Piston displacement clips against obstacles, preserves travel velocity and caps each axis at 0.51 per tick. Ordinary movement shares the axis-ordered box clipping already used by controlled mobs. |
| Contextual shapes | Scaffolding uses feet height, descending intent, distance and bottom state. The piston inside-effect path uses registered inside shapes rather than outlines, with independent fluid-height checks. |
| Honey | Side sliding thresholds/velocity, horizontal damping, fall-distance reset (including falling blocks), landing/slide sounds and entity status, player advancement interval. |
| Vault | Player eligibility/ranges, 14-tick unlocking, 20-tick opening/ejection intervals, configured built-in loot tables, display cycling/sync, used-item stat, failure cooldown, saved queue/deadline/history with oldest-player eviction above 128. |
| Commands | Successful-result count callback, execution timestamp guard and persistence, chain limit gamerule, preserved execution flags through editor changes, repeating scheduling while disabled. |

## Source evidence

Local Java 26.2: Level.setBlock; PistonMovingBlockEntity and PistonMath;
Entity.limitPistonMovement; ScaffoldingBlock.getCollisionShape; HoneyBlock;
VaultBlockEntity, VaultState, VaultConfig, VaultServerData and VaultSharedData;
BaseCommandBlock.performCommand and CommandBlock.executeChain.

## Verification

- Background full library run 6: 452 passed. Two existing local-socket tests failed
  with sandbox `Operation not permitted`; both passed when separately rerun outside
  the sandbox. This was not one completely green full-suite invocation.
- Added regression checks for partial moving shapes/scoped collision suppression,
  swept front faces, scaffolding top/bottom collision, vault history/in-flight queue
  persistence, command timestamp defaults/persistence and command success counting.
- Final background library run 7: **452 passed, 0 failed**, with only the two
  separately passing socket tests excluded.
- No live server/contraption comparison performed.

## Still open, in priority order

1. **D01: movement and inside effects.** Movement replay and remaining inside-effect lifecycle details; full piston movement side effects, step-up/entity
   collision interactions, nearest-support selection, scaffolding climbing.
2. **D02: loot engine.** Full function/predicate/component contexts, reloadable
   tables and persistent named random streams. Vault now uses the existing loot
   evaluator; unsupported evaluator behavior is not fixed by its new lifecycle.
3. **D03: world lifecycle.** Exact neighbor/scheduled update ordering at chunk
   boundaries, experimental redstone orientation and block-entity integration.
4. **D04: entity foundations.** Per-entity Java random streams, dynamic environment
   attributes, full persistent TNT ownership, piglin anger and linked entity
   lifecycles. This work precedes full per-mob passes.
5. **D05/D06: containers and clients.** Lock predicates, component-form names/output,
   raw comparator values, menu/protocol details and Bedrock verification.
6. **Remaining block systems:** creaking-heart/resin/protector lifecycle, structure
   save/load, portal transitions/passenger trees, rotated summon patterns,
   detailed spawner/trial-spawner/beacon behavior and exhaustive data/shape comparison.

Do not mark these remaining dependencies complete from passing helper tests.


## Swept inside-block traversal continuation

- Ported Java `BlockGetter.forEachBlockIntersectedBetween`, directional cell order,
  corner DDA, entering-face clipping and `AABB.collidedAlongVector`.
- Verified exact visited-cell order and step numbers against the unmodified Java
  server for 120 cases; fixtures preserve raw double bits to avoid parse rounding.
- Recorded ordinary server movement with its original axis order, accepted Java/
  Bedrock player motion, Java vehicle movement and thrown-item motion. The world
  drains pending movement after entity/player ticks; existing collision phases
  drain their own records. Teleports/world changes clear records.
- Inside-block dispatch traverses movement segments, deduplicates cells across
  records, honors the 16-iteration budget and checks the destination on exhaustion.
- Piston pushes apply their own swept effects immediately. Bubble columns receive
  Java's precise-contact flag. Cauldrons expose wall/content shape unions so crossed
  basins can trigger even when the entity has already left their endpoint box.
- Background library verification: 454 tests passed, with the two previously
  separately passing localhost socket tests excluded. Final run 6 passed after
  the vehicle/projectile recording and world end-of-tick drain changes.

Remaining for this pipeline: movement replay, other direct entity-specific
position changes, per-entity sound randomness/categories and full gameplay
verification of freeze/fire lifecycle. Swept geometry coverage is not proof that the
whole inside-effect pipeline matches Java yet.

## Ordered inside effects continuation

- Added Java's step collector: primary effects deduplicate per step, before/after
  callbacks remain ordered, and effects stop when the entity dies.
- Fire, powder snow, layered/lava cauldrons and water/lava contacts share the swept
  traversal pipeline. Fluid force/height calculations remain in base tick.
- Freezing increments in the collector; ignition clears freezing. Extinguishing
  preserves negative immunity timers; fire immunity and periodic lava/fire damage
  follow the shared source rules. Rain and extinguishing sound run after effects.
- Exact collector traces match the unmodified Java server for 100 sequences of
  80 operations. Background library run 3: **456 passed, 0 failed**, with the two
  previously separately passing socket tests excluded. No live gameplay run.
- Sound pitch currently uses world RNG; per-entity random streams and full sound
  categories remain an explicit D04 gap. This is not full engine certification.
