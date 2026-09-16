# Block and item parity checkpoint — 2026-09-15

## Status and constraints

**The requested 1:1 parity for all blocks and then all items is not complete.**
This checkpoint records source changes, not a passing parity result.

- Work branch: `codex/vanilla-spawning`. The initial continuation is in
  `6d0f8370`; the source work below continues that checkpoint.
- The user explicitly prohibited compilation and tests for this work. No Cargo
  check, build, test, server restart, or gameplay comparison was performed.
  Rustfmt was used to format the edited sources.
- Reference: the local vanilla Java 26.2 decompile in the hosting repository's
  `comparison/vanilla-src/26.2/src/net/minecraft/` and Pumpkin's 26.2 assets.
- The earlier handover is on the hosting repository's
  `codex/vanilla-spawning-lab` branch at `comparison/orchestration/HANDOVER.md`.
- Preserve the pre-existing modified `crates/pumpkin-plugin-wit` submodule.
  Do not commit to the hosting repository's `main` or merge the four unfinished
  `agent/mob/*` branches.

## Current execution plan

The user approved a bounded block-first batch pass with a separate shared-dependency
queue, followed by items. [PARITY_PLAN.md](PARITY_PLAN.md) is the current checklist;
this file remains the historical implementation log. Background tests are now
authorized as long as implementation continues without waiting for them.

## Latest continuation: falling blocks, sponges and bubble columns

- Falling blocks now retain fluid at takeoff while clearing carried waterlogging,
  persist their state/time/drop/damage flags and optional block-entity payload,
  reject occupied or unsupported landing cells, respect the entity-drop rule,
  time out after 600 ticks (100 outside height bounds), and preserve moving-piston
  landing deferral. Source-water/collision ray traversal catches fast concrete
  crossing a thin water layer; concrete hardens through the landing transition.
- Anvils now apply falling damage to living noncreative targets, damage their own
  anvil state probabilistically, and emit landing/break events. Falling blocks use
  their current post-collision velocity for drag and expose pickable hitboxes.
- Sponge traversal follows Java direction order, six-edge depth and 65 accepted
  nodes including the origin. It drains waterlogged blocks and bubble columns,
  drops kelp/seagrass loot and processes all neighbor notifications. Wet conversion
  uses flag 2; drying uses event 2009 and the level-random pitch. SpongeAbsorbEvent
  is retained before the first mutation.
- Water now schedules its own block tick when supported by the source tags,
  rather than a bubble-column tick that the scheduler would reject. Columns form
  or collapse upward in the same operation, use flag 2, and schedule five-tick
  shape reconciliation and fluid ticks. Source water uses actual fluid state.
- Bubble motion distinguishes clear collision/fluid space above, applies surface
  downward cap -0.9, ignores flying players and embedded arrows, and uses uncapped
  projectile acceleration except ender pearls. Surface particles consume level
  random draws. Inside columns reset living/falling fall distance; player air
  recovers through the existing breathing tick instead of instantly refilling on
  body collision. Boats use a 60-tick bubble timer and launch/eject velocities.
- Remaining: full entity random initialization, precise/swept collision callbacks,
  projectile subclasses and trident embedding, boat splash/physics details, mob
  breathing, portal duplication and other nonliving fall mechanics, strict NBT
  property codec errors, generalized bucket pickup interfaces, and custom tags /
  environment attributes. Source review/rustfmt/diff checks only; no tests/build.

## Latest continuation: archaeology blocks and continuous brushing

- Suspicious sand/gravel now schedule two-tick block updates on placement, state
  changes and neighbor shapes, reset brush progress after 40 idle ticks by two
  strokes every four ticks, and fall without restoring the suspicious block or
  dropping its contents. Falling destruction emits the source particles/event.
- Brushing uses ten successful strokes, a ten-tick cooldown, dust stages 0/1/2/3
  at counts 0/1/3/6, first-hit direction and the stored loot table/seed. Missing
  loot stays empty; the fabricated default archaeology loot was removed. Loot
  receives the tool, player type, origin and luck supported by the current engine.
  Completion drops from the brushed face with zero velocity, emits event 3008,
  then replaces the block. Partial progress is not persisted in NBT.
- The brush now starts continuous use on a block and strokes at ticks 5,15,...,
  checks the current view ray and intervening pickable entities, stops on a miss
  or changed held item, and damages the active hand only on completion. Java's
  item-on-block dispatch now carries the actual hand through a default-compatible
  trait method; Bedrock retains its main-hand wrapper. BlockBrushEvent remains.
- Falling-block creation retains the displaced fluid. Client brush animation
  supplies dust visuals while server random draws are consumed for the same
  visible built-in block families.
- Still open: loot-engine functions/random sequence fidelity and advancement
  triggers, ray hitbox margins/root-vehicle exclusions, nonplayer brush users,
  general falling-block landing/drop/damage/persistence behavior and Bedrock
  animation equivalence. Source review/rustfmt/diff check only, no build or tests.

## Latest continuation: eggs, coral, fire and state callbacks

- Sniffer eggs schedule every hatch stage, use the hatch-boost tag, sounds/events
  and level random delay, and spawn a baby sniffer. Turtle eggs use the day-timeline
  hatch chance, source trampling/landing order and egg-stack behavior, and spawn
  baby turtles with saved home positions. Frogspawn uses source-water support,
  source hatch delay/count/positions and persistent tadpoles without item drops.
- Same-block state changes now dispatch a separate `state_changed` callback so
  audited handlers can run Java onPlace behavior without recreating inventories.
  Eggs, coral plants/fans, falling blocks, anvils, dragon eggs, frosted ice,
  redstone torches, repeaters, comparators and fire have been migrated. Other
  handlers still need review before forwarding their legacy placement callback.
- Coral plants/fans include their own waterlogging in survival, accept full water
  on placement, use sturdy support faces and schedule fluid/death ticks on shape
  updates. Dead variants retain water ticks. Coral blocks schedule drying during
  item placement and neighbor updates. Death writes use source flag 2.
- Ice respects the prevents-ice-melting enchantment tag and creative destruction,
  uses source support and water neighbor notification; frosted ice uses level
  random delays, signed light thresholds and flag-2 aging, including the onPlace
  reschedule. Redstone uses the new state callback instead of manual duplicate
  torch/repeater updates; burnout uses world event 1502.
- Fire gates the whole scheduled operation by player radius, uses level random
  draws, old-age spread decisions, correct waterlogged fuel checks, source age
  flags and neighbor-state age preservation, source-position burnout and soul
  fire conversion. Unsupported placement performs the source survival check.
- Dragon eggs teleport on initial noncreative attack through a shared block
  attack callback (Java and Bedrock), use triangular offsets, border/build-height
  checks, flag-2 destination placement and five-tick falling updates. Breaking an
  egg no longer teleports an already-removed block.
- Limits: built-in dimension/biome attributes remain approximated by the existing
  environment adapter, entity creation still lacks full spawn-reason plumbing,
  world-border interpolation/edge semantics and remaining attack hooks are open.
  Source review and rustfmt only; no compilation or tests were run.

## Latest continuation: remaining vegetation and feature growth

- Bamboo now uses the source height count, leaf transitions, thickness, stop ages,
  light checks, fluid placement rules and passed neighbor state. Hanging moss uses
  the correct ceiling face support and breaking notifications.
- Sugar cane accepts adjacent tagged water **or** ice, resets each new segment to
  age zero and uses the age-update flags. Lily pads distinguish source water from
  flowing water, inspect the fluid above and handle all boat variants. Sea pickles
  use top-face support, source-water placement, Java random draws and the standard
  bonemeal item path rather than a duplicate click handler.
- Big dripleaves handle entity collision, projectile tilting, redstone resets,
  vibration-producing tilt changes and the correct sounds/update flags. Invalid
  stems collapse after one tick without creating a new leaf. Small dripleaves use
  exact half matching, player placement, support tags and fluid-preserving growth.
- Chorus growth uses Java horizontal order and client-only growth updates; broken
  chorus cascades notify neighbors. Projectile destruction uses impact-projectile
  tags, the projectile-breaking game rule, owner mob-griefing and player spawn
  protection. Huge mushroom faces use the supplied neighbor state.
- Saplings, propagules and azaleas have source bonemeal height/fluid checks and
  level-random success rolls. Tree selection uses the source float draws, including
  secondary mega-tree ordering. Failed mega growth restores the clicked sapling
  state to all four positions; single growth retains the original fluid. Removal
  and restoration use Java 260's mapped flags.
- Cached tree/fungus/mushroom generation now advances the existing level Legacy
  random stream rather than seeding an unrelated Xoroshiro generator. The lock is
  released before applying live updates. Cache fluid reads include pending states;
  writes reject out-of-height positions and preserve explicit flags and destruction
  operations, including drops from planted fungus replacement.
- Nether fungus now supports bonemeal growth on the matching nylium with the
  planted configured feature; support tags depend on the fungus being placed.
  Fungus generation uses replaceable states and the source hat-radius draw order.
- Mushroom growth shares the existing configured feature implementation, removes
  the small mushroom before generation and restores it on failure. Mushroom spread
  follows X-fastest traversal without extra loaded-position gates. Huge mushroom
  height bounds and the source's red-mushroom trunk-only clearance check are fixed.

These changes are source ports, not completed parity evidence. The feature adapter
still buffers mutations: neighbor callbacks, drops and their random draws are not
interleaved with feature generation as in vanilla. Tree shape/post-processing,
custom configured-feature providers, height-view type limits and chunk boundaries
remain shared feature-pipeline work. Broader collision, mining/drop ordering and
protocol paths still require review. No compilation or tests were run.

## Terrain, carpets and landing continuation

- Farmland now rechecks support at scheduled ticks, uses `MAINTAINS_FARMLAND`,
  changes moisture only when needed and sends client moisture updates. Drying and
  trampling use the shared collision displacement and attributed BLOCK_CHANGE.
  Trampling follows the random, living-entity, griefing and size gates before fall
  damage. Dirt paths accept fence gates and use the same displacement/event path.
- Landing callbacks now receive their block position; turtle eggs use it. Species
  fall-damage immunity no longer suppresses block landing effects (damage itself
  still checks immunity). The wider landing/movement pipeline remains open.
- Leaf placement reads actual source fluid, and decay drops resources then removes
  the block without treating natural decay as an attributed block destruction.
  Snowy block updates use the passed state. Grass/mycelium fluid checks include
  aquatic plants and full falling fluid rather than only bare source blocks.
- Grass, nylium and moss bonemeal now invoke configured/placed features with the
  level random stream. Grass delegates short-grass growth to its block behavior;
  biome selection includes nested selector features in declaration order. Removed
  duplicate hand-written nether vegetation and twisting-vine growth.
- Simple feature placement uses optional providers, handles double plants and pale
  moss carpets, applies flag 2, and schedules requested ticks. Generated block ticks
  survive conversion from proto-chunks; live caches retain scheduling order.
  Nether vegetation uses the source height bounds and flag 2.
- Pale moss carpet now shares its face support, low/tall side transitions, upper
  topper creation, base survival and bonemeal rules between live and generated
  placement. Ordinary wool/moss carpets lose support on shape update rather than
  after a delayed tick. Rooted dirt uses the source target checks. Netherrack uses
  cached light dampening (including transparent cubes and waterlogged states),
  X-fastest neighbor scanning and a Java boolean draw.

Feature buffering still defers callbacks and their random draws; world-generation
pale moss also lacks a separate level random stream from the feature random stream.
The proto-chunk fluid accessor, custom provider behavior, dynamic collision shapes,
full loot evaluation and chunk-edge scheduling remain shared gaps. Formatting and
`git diff --check` were performed; compilation and tests remain prohibited.

## Changes in this checkpoint

### Sculk sensors and shriekers

- Connected normal sensors, calibrated sensors, and shriekers to the vibration
  listener, block-entity ticking, and world game-event dispatch.
- Corrected the handover's calibrated radius: **16**, compared with 8 for the
  normal sensor. Calibrated active time is 10 ticks; normal active time is 30.
- Corrected candidate selection: nearer events win; frequency breaks a distance
  tie. Calibration filters frequency before selection, rather than output power
  after arrival.
- Added vibration NBT, UUID source attribution, particle delivery/reload, chunk
  saving notifications, and the requirement for adjacent chunks to be ticking.
- Replaced sampled occlusion rays with voxel traversal and corrected damping to
  cover dropped tagged items and wardens, rather than equipped wool.
- Added resonance, tendril-clicking events, water-sensitive sounds, step-on
  scheduling, redstone output directions, and waterlogged placement/update handling.
- Added source/context-aware emission for placement, breaking, movement, damage,
  death, interactions, tool transformations, and selected redstone actions;
  explosions also emit their game event. Plugin cancellation remains respected.

### Catalyst spreading, multiface blocks, walls, and stored bees (continuation)

- Connected catalyst death delivery independently of vibration travel/occlusion.
  Nearby catalysts are ordered by distance; the first consumes the living entity's
  XP once, including deaths without player kill credit. Zero-XP deaths still bloom.
  Added the eight-tick bloom reset and the default It Spreads advancement.
- Added runtime charge cursors, 32-cursor/1,000-charge limits, merge rules, decay
  delays, NBT, charge particles, substrate conversion, and sensor/shrieker growth.
  Movement uses the vanilla 18-neighbor enumeration and shuffle. Persisted missing
  facings and empty facings retain their distinct meanings.
- Added collision-volume displacement when sculk replaces partial terrain. Runtime
  vein growth uses same-position, same-plane, and wrap-around spread attempts.
- Fixed multiface support checks, existing-face waterlogging, nearest-looking
  placement order, and replacement. Removed the duplicate direct item interaction.
  Glow lichen bonemeal now spreads one face using the vanilla spread search.
- Added the missing five-XP block data for normal/calibrated sensors, catalysts,
  and shriekers, in both source assets and generated data. Ordinary sculk already
  had its one-XP entry. The shared silk-touch suppression still applies.
- Walls now use collision-face coverage for tall sides and the post, honor a post
  above before suppressing a post between tall sides, connect to copper bars,
  preserve unchanged side connections during neighbor updates, and accept water
  according to the actual fluid state.
- Replaced opaque hive occupants with the shared occupant codec, saved timers,
  ordinary/emergency release, blocked-exit checks, age/love updates, flower/hive
  positions, nectar delivery, work sounds, and save notifications.
- Added real `bees` component NBT and packet data, plus hive item component loading
  and drops. Silk-touch drops retain occupants/honey; normal hive drops do not
  duplicate occupants. Added creative occupied-hive drops and the default
  Total Beelocation advancement condition.
- Connected bottle/shears harvesting, smoke sedation, adjacent-fire release, and
  player-mining release. Smoke checks use the four-pixel collision column. Fixed
  the built-in dimension bee-stay rule so lack of skylight alone does not trap bees.
- All of the above is **source implementation, not verified behavioral parity**.
  No compilation, tests, server run, or gameplay comparison was performed.

### World-generation sculk and upstream events (further continuation)

- Reconciled world-generation sculk's neighbor enumeration with runtime/vanilla
  (X fastest, then Y, then Z), refreshed stationary cursor facings from the captured
  state, and shared collision-face coverage through `BlockState`.
- Fixed sculk's source-water gates in both paths, world-generation waterlogged and
  aquatic-plant fluid recognition, and the complete fire tag exclusion. Vanilla
  `FluidState.is(Fluids.WATER)` excludes flowing water; Pumpkin normalizes its fluid
  families, so the separate source flag must be checked.
- Added projectile launch events with saved `HasBeenShot`, shared thrown/arrow/
  trident impact events, and shulker-bullet impact events. Impact events use the
  resulting hit-block state and preserve projectile/owner context after removal.
- Added explicit entity context to vibration dispatch. Bee-exit events can retain
  their source before the bee is inserted into the world.
- Carried the acting player through generic container open/close hooks, including
  both halves of double chests. Chests, barrels, shulker boxes, and ender chests
  emit transition events immediately with player attribution; passive count
  reconciliation emits without a source.
- Added connected copper-chest scrape/wax-off particles and block-change events
  before transformation, plus the transformed-state context on the primary event.

### Explosion, container, and attribution follow-up after `0b249314`

- Explosion objects now retain the actual direct source through block hooks,
  loot context, plugin events, and vibration events, including sources already
  removed from the world. Updated TNT, TNT minecarts, creepers, withers/skulls,
  fireballs, end crystals, and wind-charge call sites.
- Explosion loot is evaluated before clearing the block entity, preserving
  shulker contents and allowing hive occupants to escape. Hives release for the
  five vanilla direct-source classes and anger nearby bees after explosion hits.
- Shears use the generated HARVEST_BEEHIVE loot table with tool/state/entity-type,
  origin, weather and time context, retain the harvest plugin event, and use normal
  block drop positioning. The shared loot engine still lacks full entity/block-NBT
  contexts and vanilla random-sequence support.
- Chest/barrel/ender-chest counts recheck every five ticks using the largest opener
  interaction range plus four. Both double-chest halves are recognized, busy
  inventory locks postpone the recheck, and stale closes cannot wrap the count.
  Shulker boxes retain their separate counter behavior. Tracked generic container screens
  now close when the original block entity disappears or the player leaves its
  interaction range; both halves of double chests must remain valid. Non-player
  users, spectator ender-chest tracking, and custom interaction-range attributes
  remain open.
- Shrieker attribution now checks actual player-controller eligibility before
  projectile/item owners. Boats, saddled mounts, steering items, and happy-ghast
  harness/still state are distinguished from arbitrary passengers.
- Dropped items preserve their thrower UUID in NBT. Player inventory drop hooks
  now honor the ownership flag; vibration projectile-owner fallback is restricted
  to projectiles. Item pickup restrictions/target-owner handling remain part of
  the broader item audit.
- No compilation or tests were run; source formatting and diff checks only.

### Other blocks

- Hay placement now uses the clicked face's axis.
- Unwaxed copper bars, chains, and lanterns combine weathering with the base
  block's placement, support, waterlogging, and neighbor behavior.
- Copper chest placement and neighbor changes synchronize oxidation/wax state;
  changing copper chest block IDs preserves the block entity and its inventory.
- Lantern support checks respect the placed hanging/standing orientation.
- Added water tick scheduling to the edited waterlogged connecting blocks and
  consolidated the connection-exception helper.
- Fixed `post_process_state` to update the block being processed rather than
  writing its state into neighboring positions. This helper currently has no
  production callers; that correction alone cannot affect gameplay.

### Items and combat

- Item entity interactions return an action result. Successful item fallback
  emits `ENTITY_INTERACT`; packet handlers propagate server swing results.
- Name tags set mob persistence through the existing `EntityBase::get_mob` path.
  Name tags and sheep dyeing check living health as well as removal state.
- Milking produces a milk bucket using the actual interaction stack. Shearing
  and brushing durability changes update that stack rather than being lost when
  the packet handler writes its copy back. Removed redundant item fallback
  handlers that could bypass the target entity's interaction rules.
- Powder-snow buckets use block placement, the bucket placement sound, and an
  empty-bucket result in Java and Bedrock placement handlers.
- Sweep selection uses the victim's inflated bounding box and excludes nonliving
  targets, allied teams, marker armor stands, and entities at least 3 blocks away.
- Replaced `BlocksAttacksImpl`'s marker with delay, reductions, damage-type holder
  sets, durability function, sounds, and cooldown scale. Corrected its network
  layout; item damage is three floats, not a discriminator. Added NBT persistence.
- Preserved `WeaponImpl.disable_blocking_for_seconds` in generated defaults, NBT,
  and packets, and connected it to player shield disabling. Blocking now reads
  its component, supports partial reductions and piercing-arrow bypass, and
  continues through the ordinary damage cooldown/accounting path.
- Fixed shield hand selection and avoided holding the active-hand lock across
  item break/stop-use callbacks.
- Synchronized generator code and generated item/damage-type data without running
  the generator. Damage types now retain their registry resource names separately
  from death-message keys.
- Archaeology reads saved loot before replacing/removing the brushable block entity.

### Bee lifecycle, container validity, and beacon continuation after `82b10324`

- Replaced the bee's unconditional player targeting and generic wandering with
  bee-specific attack, retaliation/universal anger, hive entry/search/return,
  flower validation/search/pollination, crop growth, and wandering goals.
- Added saved anger target/end time, nectar/search/stay-out timers, stochastic
  post-sting death, underwater damage, poison and stinger metadata/decay, and
  special flower feeding. Hive entry stores portable occupant NBT, detaches riding
  and leash state, inherits flower positions, and emits entry sound/game events.
- Added opt-in flying navigation through the existing Java navigation state,
  flight/look control behavior, pollination navigation pause, and bee flap events.
  Bee movement uses controlled speed, vertical drag and liquid-jump behavior.
  Tag-based temptation is now available to the shared goal.
- Extended container identity/range checks to dispenser/dropper/hopper/crafter,
  all cooking blocks, brewing stands, and lecterns (which also require a book).
  Spectator ender-chest menus now retain and clear their active chest reference.
  Player block range now uses its attribute, with transient creative range modifiers.
- Reconnected the previously unreachable beacon screen factory using its property
  delegate. Payment is per-menu, restricted to one tagged payment item, drops on
  close, and follows vanilla shift-click routing. Levels/effects synchronize via
  three menu properties. Registry IDs and null effect encoding are now distinct;
  speed (ID zero) works, invalid effects are rejected, and payment is consumed only
  for a valid selection. Beacon block entities no longer expose a payment inventory.
- Corrected beacon scan/effect/publish ordering and world-age scheduling; added
  activation, deactivation, ambient and selection sounds, default construction
  advancements, dirty-state tracking and dimension-height effect bounds.
- Source review and standalone rustfmt only; no compilation or tests. This batch
  does not certify all bee/beacon edge cases or whole-family parity. Shared path
  collision sampling, per-mob Java RNG fidelity, unloaded hive POI discovery,
  invulnerable/rejected-hit pollination interruption, custom environment/advancement
  predicates, block locks/names, and cross-version metadata remain review areas.

### Workstation validity and vegetation (continuation)

- World-backed crafting, enchanting, anvil, smithing, stonecutter, loom,
  cartography and grindstone menus retain the opening position and require their
  block and interaction range. Button/rename packets also check validity.
- Eyeblossom transformation now emits its old-state block-change context, uses
  the Java random stream and X-first neighbor order, and serializes trail target,
  color and lifetime for supported Java clients. Collision poison uses the bee
  attraction predicate and does not refresh an existing poison effect.
- Wither roses honor general damage invulnerability/removal. Wither/skeleton
  wither immunity and dragon effect rejection now apply through the shared effect
  path. Enchantment-driven damage immunity remains a shared gap.
- Bush/firefly bush and flowerbed bonemeal behavior is implemented; dry-grass
  spreading shares the vanilla direction order and world-random shuffle.
  Flowerbeds use vegetation support tags; segmented placement preserves the
  opposite player-facing direction and respects secondary use.
- Hanging roots use the ceiling face and upward-neighbor gate. Spore blossoms
  use downward center support, unstable-center exclusions and the water gate.
  Leaf litter and short-grass growth now recheck their proper support.
- Double plants distinguish survival from placement room, require matching species
  and opposite halves, and create their upper half only on player placement.
  Tall seagrass uses full-water and correct half checks. Ordinary seagrass checks
  full water on placement and schedules water ticks while surviving updates.
- Source comparison and rustfmt only; no compilation or tests. These changes do
  not close the full vegetation audit: double-plant mining/drop ordering, broader
  liquid-container handling, dynamic support shapes and protocol compatibility
  remain shared review areas. Client-local ambient animation stays client-owned.

### Crops, growing vines, and nested random updates (continuation)

- Ordinary crops require raw light level eight to survive in the live world;
  growth retains its separate level-nine gate. Crop moisture uses `grows_crops`
  and optional moisture data. Added ravager trampling with mob-griefing gating.
- Pitcher crops now use the moisture-based growth roll, light/build-height checks,
  safe upper-space checks for random and bonemeal growth, valid lower-half lookup,
  opposite-half survival and vanilla update flags. Fixed torchflower age-zero
  conversion and melon/pumpkin stem support selection, fruit tags, random direction
  order and update flags.
- Kelp, twisting vines, weeping vines and cave vines now share the growing-plant
  lifecycle: randomized placement ages, head/body transitions, one-tick support
  destruction, natural growth rolls, head discovery and bonemeal growth. Kelp
  checks full-water placement and schedules fluid ticks; cave-vine transitions
  preserve berries and natural growth rolls their berry chance.
- Sweet-berry growth uses above-block brightness and its block-change event.
  Sweet/glow berry harvesting uses the built-in tables, plugin harvest callbacks,
  picking sounds, update flags and attributed block-change events. Corrected cocoa
  placement/support state selection and growth flags, and cactus's three-high
  early return and explicit growth neighbor notification.
- Non-player block destruction now retains the source for loot and game events.
  Broken blocks restore their actual fluid state, including aquatic plants.
- Added a shared Java random adapter for block/fluid random ticks and the bamboo
  and stem bonemeal callbacks. It locks each draw, allowing nested shape updates
  to consume the same world stream without recursively locking a held mutex.
- Source review, rustfmt and diff whitespace checks only; no compilation or tests.
  Light data for non-world accessors, shared block-interaction loot context/random
  sequences, double-plant mining/drop order, liquid-container rules, block update
  flag mapping, custom block data and client-controlled mount movement remain
  broader dependencies. This is not a full block-family or item parity closure.

## Original queue: source-edit status

These are implementation statuses, **not verified closures**.

| Queue entry | Status |
| --- | --- |
| `copper_bars_chains_weathering` | Implemented, including lantern wrapper |
| `hay_block_axis` | Implemented |
| `copper_chest_neighbor_sync` | Implemented; inventory preservation also fixed |
| `use_on_entity_result` | Implemented through registry and packet handlers |
| `persistence_via_entitybase` | Implemented using the existing mob accessor |
| `powder_snow_bucket_pipeline` | Implemented for player placement |
| `blocks_attacks_component` | Component/codec/common blocking/weapon cooldown implemented |
| `sweep_aabb` | Box and basic target eligibility corrected |
| `exception_helper_duplication` | Consolidated |
| `fishing_loot_hardcoded` | Open; see dependency below |
| `bow_attack_interval_dynamic` | Open; belongs to the remaining mob work |

## Why the old coverage percentages are insufficient

The block inventory has 1,196 IDs grouped into 265 families. Registration is not
behavioral equivalence. Some reported gaps come from macro/tag registrations,
base-block handlers, fluids, or data-only blocks. Rotated pillars are handled by
`LogBlock`'s tag plus explicit IDs; liquid behavior uses the fluid registry;
lightning rods and bars have registrations the old scanner misses.

Conversely, the registered catalyst lacked runtime spreading before this
continuation. Other registered families still have behavior gaps. Do not turn
scanner results into “100% parity.” The earlier `progress.py` also hardcodes the
item count at 40/51 and uses disposable agent branch names as review evidence.
Those branch names were removed during the authorized merged-worktree cleanup,
so they must not be used to reconstruct audit completion.

## Remaining work, in the user's block-then-item order

1. **Finish the block audit and implementations.** Runtime catalyst spreading,
   multiface behavior, wall geometry, and stored-bee release now have source ports,
   but this does not close the full family audit. Exact XP rewards still depend on
   the shared mob reward/enchantment implementation. Dynamic collision contexts,
   chunk-edge behavior, and protocol-version differences remain unverified.
2. **World-generation sculk** now shares neighbor ordering and collision-face
   coverage with runtime, but the adapters remain separate. Proto-chunk boundaries,
   post-processing/tick scheduling, dynamic shape contexts, and the rest of the
   feature pipeline still require source comparison.
3. **Bee lifecycle has a source implementation**, including entry, pollination,
   flight and neutral anger. Its remaining shared boundaries are listed above;
   loaded block entities currently supply hive discovery instead of the full POI
   manager. Full custom environment attributes, loot context and random-sequence
   fidelity remain shared gaps.
4. Finish sculk's upstream game-event coverage. Fishing-hook impact geometry and
   event delivery still need a full port. Player container interaction-range
   rechecks are implemented; non-player users and the broader lifecycle remain open.
   Movement coverage is for living entities with bee flapping added; nonliving
   movement, other species' flapping and movement sound/effect ordering need review. Shrieker controller/item attribution
   has a source port, but underlying mount equipment/control implementations and
   legacy/Bedrock vibration particles still need their broader audit.
5. Audit every remaining block family against its actual vanilla implementation,
   including data-driven drops. A registration or a source edit is not enough
   evidence to mark a family 1:1.
6. **Then finish every item class and shared component path.** The earlier 40/51
   claim has no reliable per-class completion ledger. Specific known gaps:
   - Fishing still hand-builds its loot. The shared loot engine cannot yet express
     nested loot-table entries, quality weights, fishing/open-water and biome
     predicates, and all required item functions. Replacing the fishing helper
     with the current engine would silently remove valid catches.
   - Brush continuous-use ticks, offhand wear, progress decay and table selection
     are ported above; shared ray geometry, loot functions and NPC users remain open.
   - Spawn-egg offspring still use generic entity construction/baby metadata,
     rather than the ageable offspring factory and eligibility checks.
   - Sweep damage scaling, enchantment effects, knockback, and movement gating
     require review beyond the target-box correction.
   - Specialized mob reactions to blocked attacks (notably ravager stun), custom
     component edge cases, protocol-version differences, and the broader item
     component/interaction pipeline still need comparison.
7. The earlier regression/gameplay comparison remains unperformed. Compilation and
   testing remain prohibited unless the user changes that instruction.

## Continued source port: inherited block effects and placement

- Amethyst buds/clusters and budding amethyst inherit projectile chimes; bud
  support uses the stored facing, placement uses source water, and growth accepts
  full falling water without incorrectly waterlogging the new bud. Cactus lava
  adjacency now reads the fluid state.
- Added the shared spawn-after-break callback with normal/explosion ordering.
  Infested variants spawn silverfish through this callback (including explosions),
  respecting block drops and the prevents-infested-spawns enchantment tag. Block
  loot positions and experience providers now draw from the level random stream.
- Redstone ore activates on attack/use/uncautious steps, consumes the server's
  exposed-face random draws, and allows adjacent block placement/merging. Removed
  activation from mere entity overlap. Placement queries now expose cursor and
  clicked-position context without requiring a fabricated protocol packet.
- Snow layers use ordinary placement/consumption and merge only to eight layers;
  removed the incorrect ninth-layer conversion to a snow block. Neighbor survival
  returns air immediately, and melting uses drop-resources then removal. Slab
  merging distinguishes adjacent targets, handles the exact 0.5 boundary, and
  reads source water for waterlogging.
- Note block attack is dispatched through the shared Java/Bedrock hook, with
  vibration source attribution. Top-instrument items pass to placement; pling and
  all trumpet instruments use base-block/tunable rules. Custom skull sound is
  read from the skull entity, and playback uses the block center.
- Source review and standalone formatting only; no compilation or tests. Sound
  packet seed/range delivery, complete placement contexts, loot random sequences,
  and inherited behavior across all remaining families are still open.

## Continued source port: speleothems and geysers

- Pointed dripstone and sulfur spikes now share the SpeleothemBlock source
  algorithm: facing/support, exact tip/frustum/base/middle updates, sneak merging,
  water scheduling, delayed unsupported destruction/falling and trident breaks.
  Removed whole-column mutation from placement/player-break callbacks.
- Added natural growth with the source probability, search bounds, random order,
  substrate/water requirements, and sulfur's shorter maximum length. Added
  dripstone source-fluid transfer, mud-to-clay conversion, obstruction scans,
  delayed cauldron filling and the matching world/game events.
- Falling speleothems use the source tip-only damage amount and falling-stalactite
  damage type, plus each subclass's broken-on-landing sound. Upward dripstone tips
  use the stalagmite fall-distance offset/multiplier via a shared typed fall-damage
  helper. The inherited living fall pipeline still has broader pending review.
- Potent sulfur now preserves the factory-created block entity and executes its
  activation callback on same-block state changes. Deactivation retains old-state
  context; countdown NBT always writes the source field. Geyser launch uses source
  float constants, fall-distance capping and velocity synchronization; players
  retain client-authoritative force simulation. Dynamic collision contexts and
  player-controlled vehicle authority remain shared gaps.
- Corrected shared placement ordering: choose the actual placement state before
  checking survival, including facing and halves, and reject failed AIR placement.
- Formatting and source inspection only; no compile/test/gameplay execution.
  Custom speleothem tags, dynamic collision/shape offsets, placement callbacks,
  fall mechanics and protocol behavior still prevent a blanket 1:1 claim.

## Continued source port: composting, fire and redstone triggers

- Composters stop consuming at level seven, emit success/failure fill events and
  used-item stats, schedule maturation from placed/same-state callbacks, play
  ready/empty sounds, and extract produce at the source's fixed height with two
  level-random float offsets. Added source attribution to compost state events.
- Spawner XP uses spawn-after-break, the level random stream and block-drops gate,
  with the existing plugin experience event; it is no longer player-only.
- Fire/soul fire share portal/survival placement behavior. Fire ignition clears
  freezing, uses the level random stream, and delegates fire damage immunity to
  the entity damage handler. Portal-axis selection draws one bounded integer.
  Full inside-block effect aggregation remains a shared ordering gap.
- Introduced player-will-destroy before drops/removal. Fire extinguishing and
  shears disarming now occur there, rather than after the block has disappeared.
- All four unwaxed lightning rods use weathering behavior. Rod strikes notify the
  attached block with the rod as source and always schedule/play strike effects;
  removal notifications also run during piston moves. Observers reset an imported
  powered state without a pending tick and notify correctly when removed/moved.
- Daylight detector inversion events include player/new-state context, release
  the abilities lock before callbacks, and use the existing Minecraft cosine helper.
- Pressure plates use center/rigid support, shape-update removal, trigger-ignoring
  entity filtering, source-attributed activation, and plugin-adjusted output for
  sound/recheck decisions. Added the shared entity trigger predicate for bats,
  displays, markers, interaction entities, ominous spawners and marker armor stands.
- Tripwire uses its attached/unattached detection box, pending-tick gate, precise
  recheck/release cadence and pre-removal disarming without double shears wear.
  Fixed wire/hook removal guards that previously always returned. Hook placement
  uses directional support and nearest-looking choices; line updates preserve
  intervening replacements, recheck a removed source hook, and notify both sets
  of neighbors with the correct source block. Sounds use centered positions.
- Source inspection, formatting and diff whitespace review only. No compilation,
  tests or gameplay runs. Custom support shapes, redstone experimental orientation,
  effect aggregation, automation inventory wrappers and all unaudited families
  remain open; these edits do not establish full 1:1 parity.

## Continued source port: rail connectivity and power

- Replaced custom rail placement/connection locking with RailState's ordered
  connections, soft-connection pruning, slope precedence and redstone-dependent
  junction selection. Placement first chooses the horizontal default, then the
  on-place callback connects rails. Ordinary rails reconsider three-way junctions
  when notified by a signal-source block.
- Powered/activator rails share the source's depth-eight search, slope transitions,
  same-block/axis checks and powered-chain requirement. Removed explicit recursive
  neighbor rewrites and the incorrect claimed rule about powering a middle rail.
- All rails use rigid support, source-water placement, water neighbor ticks and
  slope/straight removal notifications with the original source block. Detector
  rails notify connected rails directly and always refresh comparator outputs;
  container minecart contents now supply their analog signal.
- Command minecarts still lack a command executor/success-count implementation.
  Dynamic support shapes, custom rail subclasses/tags, world neighbor scheduling,
  minecart movement and protocol behavior remain broader pending work. No builds
  or tests were run; review was against the local 26.2 source plus formatting.

## Continued source port: dried ghast and jukebox lifecycle

- Dried ghasts now spawn the baby at the bottom center, inspect actual source water
  on placement, use setPlacedBy for placement sounds, and attribute hydration
  events to the old block state. Bucket waterlogging supplies the distinct wet
  placement sound without replaying it for an already waterlogged block.
- Jukebox playback resumes from saved elapsed ticks, includes the twenty-tick
  ending grace period, emits periodic play events/note particles, and updates
  neighbors on start/stop. Hopper insertion/removal uses the same synchronous
  state and playback callbacks as player interaction; stack size and destination
  restrictions follow the single-item container. Record ejection uses world RNG
  and the source offset, including replacement/explosion removal.
- Added weak world binding and removal hooks for block entities; World::load now
  creates the owning Arc itself. Normal removal and chunk unload emit the jukebox
  stop event. Hooks run after releasing live entity map guards. Player placement
  now carries the held stack and applies components before setPlacedBy, including
  jukebox RecordItem and saved playback data. Preserved plugin playback methods.
- This remains source/format review only: no compilation, tests, or gameplay.
  Dynamic jukebox song registries, generic typed block-entity item-data application,
  source sound RNG, fluid-flow waterlogging, factory spawn reasons and the wider
  unaudited block/item pipeline remain open.

## Continued source port: bells and projectile hit faces

- Bells now use actual support faces for placement and single/double-wall
  transitions, ring before powered-state changes, use the source sound volume
  and pitch, attribute game events to the ringing entity, and send the ring
  block event. Correct-side player hits still consume when ringing fails.
- Ported the cached living-entity search, registered HEARD_BELL_TIME memory
  updates, five-tick resonance start, forty-tick resonance duration and sixty-tick
  raider glowing effect. Weak cached references avoid owning world/entity cycles.
  Unported mob brains still cannot act on bell memories.
- Projectile block callbacks now carry the collision face for arrows, tridents
  and generic projectiles. Bells apply hit-height/face restrictions to projectiles.
  Targets score in the struck face's plane, use twenty ticks for tridents as well
  as arrows, and clear unsupported pre-powered placement states with source flags.
  Removed target's incorrect direct power and unconditional arrow bullseye award;
  the built-in award now requires strength fifteen and thirty horizontal blocks.
- Explosion callbacks now distinguish block-triggering explosions so wind charges
  can ring bells. General explosion behavior, dynamic support/collision contexts,
  custom advancement predicates and client/protocol effects remain shared gaps.
  Only source inspection, formatting and whitespace review; no compilation/tests.

## Continued source port: lecterns, campfires and container drops

- Lectern page changes and all inventory removals now drive the source two-tick
  pulse, reset HAS_BOOK, emit attributed state events and notify below with the
  lectern as source. Removed late broken-block book scattering; pre-removal drops
  use the facing offset. Corrected no-book interactions, HAS_BOOK comparator
  gating, page clamping (including zero-page books), save fields and menu insertion
  restrictions. Taking the book respects the player's mayBuild ability.
- Lecterns receive saved book/page item data and distinguish creative gamemaster
  HAS_BOOK placement. Added a separate automation-inventory hook so hoppers and
  droppers do not treat the lectern menu as a container; droppers use the shared
  container search, including combined chests and container entities.
- Campfires accept food while unlit, consume full-slot food interactions, initialize
  cooking on insertion and emit completion/insertion events. Completion retains
  source timer values and falls back to the input if its recipe vanished; cooldown
  clamps against the stored total. Client data contains items, not cooking timers.
  Source-water placement, burning-projectile ignition and bucket extinguishing
  now use their proper transitions; neighbor shape updates no longer force LIT off.
  Shovel extinguishing uses both source event points without the extra wrong sound.
- Shared container scattering now uses world random, separate arithmetic operations,
  source stack splits/triangular velocity and zero pickup delay. Campfire results
  and all four removal slots use this path, including empty-slot RNG consumption.
- No compilation, tests or gameplay. Written-book command/text resolution, menu
  container-vs-block-entity clear semantics, dynamic recipe/feature registries,
  generic typed item NBT application, item initialization RNG, client/persistent NBT
  separation and full damage/enchantment/effect aggregation remain shared work.

## Continued source port: mounted switches and persistent block-entity data

- Buttons, levers and grindstones now use the ordered placement-direction search;
  survival checks use the resulting state's attachment rather than packet-facing
  guesses. Buttons search the actual pressed/unpressed outline for the first
  arrow, replacing full-cube detection and the extra projectile-hit shortcut.
- Button press ordering, source neighbor notifications, material-specific click
  sounds and player sound exclusion follow the source. Levers return success and
  use the source's null ringing entity for ordinary use. Triggering explosions
  activate buttons/levers; removal notifications retain the original block source.
- Block-entity add/update now writes persistent NBT separately from optional client
  update tags. Reduced client tags can no longer overwrite saved cooking progress
  or omit a jukebox/lectern merely because it has no block-entity update packet.
- Source and format review only; no builds/tests. Experimental redstone orientation,
  custom block-set definitions, generic event/shape ordering and protocol-version
  coverage remain open alongside all other unaudited families and item paths.

## Continued source port: doors, trapdoors and gates

- Door hinge selection uses the actual upper-left/upper-right neighbors and exact
  center-hit comparisons. Placement checks upper space and initial power; only
  setPlacedBy creates the upper half. Survival distinguishes upper/lower halves.
  Shape updates synchronize both state and door material, including copper aging,
  instead of manually overwriting the other half on every interaction/removal.
- Door pre-break handling suppresses the lower-half drop when an upper half is
  destroyed creatively or with an unsuitable tool. Hand-openable copper doors now
  participate in the historical isWoodenDoor query. Sounds/events and redstone
  updates follow source order, with wood-specific sounds and random pitch.
- Trapdoors distinguish clicked-block replacement from side placement, use actual
  source water, schedule water on toggles/power/shape updates and emit open/close
  events. Gates initialize their wall attachment, preserve it on irrelevant shape
  updates, and send state changes before sounds/events. All three families handle
  eligible unpowered wind-charge toggles; copper wrappers delegate the new hooks.
- Corrected a flag mapping while reviewing these paths: Java flag 8 is immediate
  client rendering, whereas Pumpkin's numeric 8 suppresses drops. The campfire and
  shovel changes no longer mistake that rendering hint for drop suppression.
- World sound packets now use their own legacy random stream, matching Level's
  separate soundSeedGenerator rather than consuming gameplay random. Initial RNG
  seeding, sound range filtering and protocol-specific effects remain broader gaps.
- Source/format review only, no builds or tests. Dynamic/custom door types, exact
  contextual support shapes, placement replacement predicates, non-player event
  attribution and the remaining block/item pipeline are still open.

## Continued source port: candles, cakes and hand interaction persistence

- Candle stacking now uses ordinary block-item placement instead of a custom
  consume path. Only an empty hand with mayBuild extinguishes a candle; candle
  cakes distinguish hits above the cake from eating. Both candle families now
  inherit burning-projectile ignition and triggering-explosion extinguishing,
  including the sound/event ordering. Waterlogged candles schedule water ticks;
  bucket water extinguishes them. Support checks allow waterlogged supports, and
  ordinary candles retain the source's no-op support-loss shape behavior.
- Cake food changes use invulnerability/food eligibility and clamp saturation,
  including creative eating. Eating emits EAT before mutation and BLOCK_DESTROY
  on the final slice. Candle cakes become a once-bitten cake before dropping their
  original loot. Cake/candle-cake support loss removes them in the downward shape
  update. Adding a candle consumes normally, sounds before replacement, and awards
  its event/statistic. Ignition no longer rejects adjacent fire solely because the
  clicked block contains water; clicked waterlogged candles remain unlightable.
- Java block use now persists mutations of its copied hand stack before returning.
  Empty-hand fallback only runs for main-hand interactions. Item-use statistics
  moved from every attempted click to successful item use/placement, preserving
  the explicit stats awarded by block handlers. Cooldowns also gate block-item
  fallback. Source review and formatting only; no compilation or tests.
- Full loot RNG/context, dynamic shapes/tags, client pick stacks, liquid-flow
  container callbacks, ignition item details, adventure predicates, advancement
  triggers and Bedrock hand dispatch remain broader dependencies/open work.

## Continued source port: flower pots and chiseled bookshelves

- Flower pots consume inserted plants, return removed plants to inventory (or drop
  them), and emit the source block-change events. Invalid insertion falls through
  to the main-hand empty-hand action; an empty pot consumes that action. Potted
  eyeblossoms now reuse their transition particle and long switch sound.
- Bookshelves retain their automatically created block entity. Insertion uses the
  insertion sound/statistic; removal sounds before inventory delivery and emits
  the player event. Hit slots use 26.2's equally divided face, replacing old pixel
  cutoffs and correcting the middle horizontal boundary.
- Bookshelf inventory mutations update all six occupied properties, the last slot,
  neighbors and the state-context game event for player and automation access.
  Containers accept only bookshelf books, cap insertion at one, remove the whole
  requested slot, and require a destination that can fit it. Persistent last-slot
  NBT retains its integer width. Placement restores container components; removal
  scatters all original stacks without firing live shelf mutation callbacks.
- No compilation/tests. Shared loot component collection/pick-block handling,
  generic automation transactions and item/drop initialization are still open;
  this source pass is not a declaration of complete block or item parity.

## Continued source port: decorated pots and decoration components

- Decorated pots face the player's placement direction, preserve source water,
  schedule water ticks and retain their automatically created block entity.
  Insertion compares item components, respects creative materials, awards the item
  stat, wobbles, bends sound pitch by fullness, sends dust and emits BLOCK_CHANGE.
  Failed/empty-hand use has the source fallback and negative wobble ordering.
- Pot inventory is now available to automation, persists dirty/comparator state,
  and scatters its contents during pre-removal. Assigned loot table/seed survives
  NBT and unpacks on access through the existing container-loot engine; unassigned
  pots stay empty. Full loot-function/random-sequence parity remains outstanding.
- Tool/tag checks and eligible projectiles crack pots before destruction. Shared
  player destruction now reads property changes made by playerWillDestroy before
  evaluating loot. Removed the unconditional extra four bricks/shatter callback.
  The built-in dynamic sherd alternative drops the four actual decorations when
  cracked; intact pots copy decorations and spill contents separately.
- Pot decoration components now retain four ordered item IDs, read/write NBT, and
  serialize/validate their bounded network list. Updated the generator and its
  checked-in default by hand without running code generation. Placement restores
  decoration/container components; saves and drops preserve decorated sides.
- Source review/rustfmt/diff checks only, no builds or tests. Generic dynamic loot
  suppliers/custom tables, component hashes and protocol-version remapping,
  pick-block/crafting component propagation, loot RNG and precise generic callback
  ordering remain open. All-block/all-item parity is still incomplete.

## Continued source port: wooden shelves and item-use effects

- Unpowered shelves swap the selected whole stack with the hit third of the
  front face, preserve the creative empty-slot copy, and distinguish take/place/
  swap sounds. Powered shelves swap the corresponding final three/six/nine hotbar
  slots across their connected chain. Offhand and other faces pass; empty powered
  swaps consume. Inventory changes and client updates now follow those actions.
- Shelf placement captures power/source water. Power transitions maintain the
  source's left-first chains of at most three, using cached neighbor parts to
  prevent connection callbacks from rebuilding a chain. Disconnect/removal and
  water updates, activation sounds/events and back-face comparators are handled.
- Shelf block entities now retain their original contents, restore container
  components, send item/alignment updates and emit the requested event after
  mutation. Automation uses the same stack limits, change notifications and
  persistent dirty state; removal scatters original contents without live events.
- Replaced the empty use-effects component placeholder with its sprint, vibration
  and speed fields in NBT, the wire codec and generator. The checked-in default
  references were updated manually, preserving all seven nondefault spear entries
  from assets. Shelf removal consults the retrieved item's vibration setting.
- No compilation, tests or code generation. Shared item ownership/visual rotation,
  component hashes, custom environment/shape behavior, broader movement use-effect
  application, loot/pick-block handling and the rest of the parity ledger remain.

## Continued source port: respawn anchors and their explosion dependencies

- Uncharged anchors now pass without exploding or displaying a sleep message.
  Main-hand use defers to offhand glowstone while charging is possible; other
  items fall through to setting spawn/exploding. Charging emits its source/state
  event before sound and consumption. Spawn saves zero rotation and unchanged
  spawn consumes. Both use and respawn check the position's environment attribute.
- Invalid-dimension anchors remove themselves without mining drops and use the
  bad-respawn-point damage source, ignition and source water-neighbor resistance
  override. The central air cell is included in resistance calculation, so the
  water shield can actually stop terrain rays while retaining entity damage.
- Shared explosions collect rays before entity damage, use world legacy random,
  source float direction/step arithmetic and actual fluid states, and retain air
  positions for fire. Ordinary Java hash-bin ordering and list shuffling replace
  the arbitrary map iteration. Target states are read again during destruction.
- Damage now scales by the full diameter, includes the source's minimum damage at
  zero exposure, carries explosion position/source, and applies living explosion
  knockback resistance. Block drops collect/merge up to the source stack limit
  before spawning, then ignition draws from the same level random stream.
- Source review/format/diff checks only, no builds/tests. Large/pathological Java
  treeified hash-bin ordering, entity/projectile hit reactions, explosion loot
  sequence fidelity, generic damage attribution and respawn dismount collision/
  danger checks remain open. Bed use/removal still needs its matching source pass.

## Continued source port: beds, sleep checks and respawn positions

- Bed heads now form only from player placement; shape updates synchronize
  occupancy or remove an invalid half. Creative foot destruction suppresses the
  head drop, replacing recursive double-break callbacks. Head placement checks
  replacement and the source world-border block coordinate.
- Bed use resolves/validates the head, uses the environment bed rule and shared
  bad-respawn explosion, handles occupied beds before eligibility, and wakes the
  first intersecting sleeping villager. Range uses bottom centers and a two-block
  vertical limit. Creative players bypass the Monster query; parched replace the
  incorrect ocelot/phantom entries, and zombified piglins require anger at player.
- Sleep awards the stat before advancement, dismounts riders, reports disabled
  night skipping, and the world respects sleeping percentages over 100. Common
  suffocation overrides cover leaves, glass, grates, paths, farmland, mud and soul
  sand. Bed bounce uses the current 0.75 restitution coefficient.
- Bed/anchor respawn searches follow the source offset order and safe/unsafe
  passes, including yaw-selected bed sides, bunk beds, collision floor heights,
  climbable/open-trapdoor exclusions and player invalid-spawn tags. Candidate
  bounding boxes use type dimensions and border checks. Forced anchors preserve
  charges; eligible beds/anchors precede generic forced positions. Respawn faces
  the bed/anchor, and generic forced respawn retains pitch.
- Saved respawn data now retains yaw/pitch using the current keys while reading
  legacy angle fields. Same-position bed use updates saved rotation. Typed reads
  from synchronized entity data let wake-up use the actual sleeping bed, clear
  occupancy safely, and choose the same stand-up positions.
- No compilation/tests. Remaining: Mth atan2 approximation, dynamic shulker
  suffocation/collision, complete generic restitution, exact block replaceability
  contexts, custom environment rules/messages, persistent universal mob anger,
  broader villager sleep AI and sleeping-position NBT, world-border interpolation,
  and other shared engine/item gaps. This does not establish complete 1:1 parity.

## Continued source port: shulker boxes and moving collision geometry

- Removed placement-time block-entity recreation. Closed boxes now require
  clear space for their opening lid; already animating boxes remain accessible.
  Use opens the menu before awarding its stat, and destruction notifies comparators.
  Creative breaking of a nonempty box drops its carried contents at block center.
- Server lid states now process open-count events, advance by source float steps,
  issue start/end shape and neighbor updates, and push intersecting entities while
  opening with the source direction/delta box. Ignore-push entity types and marker
  armor stands are excluded. Open/close events precede centered, volume-0.5 sounds
  with level-random pitch. Removed entities ignore viewer refreshes.
- World movement/empty-space/dismount queries now include dynamic shulker boxes
  and search the neighboring cells containing overhanging collision geometry.
  Bed suffocation checks read the shulker's actual closed state.
- Boxes preserve custom names and deferred loot tables/seeds through NBT/item
  components. Menu access unpacks with player luck/context; spectators cannot
  generate unopened loot. Inventory access unpacks once, raw saves do not unpack,
  and empty inventories omit Items. No-update removals no longer mark dirty;
  setters clamp to the inventory/item limit, and menu validity checks identity.
- Source review/rustfmt/diff checks only; no compilation or tests. Still open:
  complete lock predicates, piglin anger, loot-generation advancement and exact
  loot RNG, dynamic support/outline/raycast geometry, mover-type-specific motion,
  all item entity initialization randomness, negative/large open counts and
  protocol/Bedrock-specific updates. These are source ports, not parity closure.

## Continued source port: brewing stands and furnace families

- Brewing/furnace/smoker/blast-furnace placement no longer recreates block entities.
  Menus open before their use statistic, all return the source success result,
  and removals notify comparators. Cooking entities scatter contents and award
  per-recipe XP in their removal callback, covering explosions and replacements.
- Brewing completion waits until a following tick to start another cycle, returns
  ingredient crafting remainders (including dragon-breath bottles), and sends
  brewing world event 1035. Container mixes precede potion mixes and a successful
  built-in mix creates fresh potion contents. Missing-potion container recipes
  still consume the ingredient as in the source.
- Brewing insertion accepts actual ingredients (including blaze powder), restricts
  bottle slots to the four supported items and empty slots, and keeps the source
  sided extraction rules. Every nonempty bottle slot affects state; the first
  loaded tick reconciles presence with flag 2. Fuel/timer changes mark comparator
  output dirty, and menu property writes update the timers.
- Furnace output acceptance compares components and the complete result count;
  crafting adds that count. Input changes choose the correct smelting/blasting/
  smoking recipe and use the source 200-tick fallback. Missing recipes while lit
  retain progress. Fuel remainder lookup retains the original fuel item. XP
  rounding consumes level random only for fractional results; removal scatters
  inventory before spawning each recipe's XP at block center.
- All four inventories accept carried Container components, clamp inserted
  stacks, validate menu identity/distance, and keep no-update removals distinct.
  Client chunk tags are empty; full timers/recipes/items stay in persistent NBT.
- No builds/tests. Remaining: signed furnace counter/plugin ABI migration,
  identity-map recipe iteration and exact orb RNG, recipe/component/datapack
  completeness, XP recipe awards/advancements, custom names/locks, immediate
  comparator ordering, block-entity placement/pick components and other shared
  engine gaps. Complete block/item parity is still open.

## Continued source port: conduit activation, attacks and breathing effects

- Conduits now run their source server behavior: every 40 world ticks they
  require the complete 3x3x3 water volume, count the three intersecting frame
  rings using the four valid blocks, activate at 16 and hunt at 42.
- Frame size sets the integer-stepped effect radius; wet players within the
  source block-position distance receive 260 ticks of conduit power. Wet Enemy
  candidates are selected with level random; retained targets use alive/range
  checks without reselection that tick. Attacks deal four magic damage with the
  source sound, and target changes send block-entity updates.
- Activation/deactivation, 80-tick ambient and delayed short sounds use source
  timing and random order. Only Target UUID persists; activation is recomputed.
  Placement checks full water, schedules water after shape updates and preserves
  automatically created block entities. Monster/Enemy classification and water
  or rain checks are shared with entity logic.
- Player breathing now recognizes conduit power and breath of the nautilus.
  Air refills by four, nautilus alone holds underwater air, invulnerability does
  not instantly refill it, and oxygen bonus can defer depletion. Drowning uses
  negative air down to -20, sends the source particle event, and respects the
  damage rule without freezing the air counter. NBT retains negative air; old
  separate counter saves still load. Java receives signed air; Bedrock remains
  bounded to its display range.
- No compilation/tests. Remaining: exact entity RNG/ordering and weather/fluids,
  generic nonplayer breathing and underwater vehicle dismount, effect/protocol
  differences, and previously recorded block/item dependencies. This source
  implementation does not establish complete parity.


## Latest continuation: cauldron interactions and filled-result exchange

- All four cauldrons accept the three full buckets, including replacing an already
  full cauldron. Lava/powder-snow emptying underwater consumes the interaction
  without changing the hand or block. Bucket pickup requires full layered levels.
- Bottles produce water potions with explicit fresh potion contents. Only plain
  water potions refill water/empty cauldrons. Centered sounds, source item/custom
  statistics, null-source fluid events and lower-level BLOCK_CHANGE events follow
  the corresponding source handlers, including the water handler's consumed-stack
  ITEM_USED lookup. Existing cauldron-level cancellation hooks cover each mutation.
- Dye washing requires CAULDRON_CAN_REMOVE_DYE and precedes individual item
  handlers. Banners lose only their last pattern; colored shulker boxes preserve
  their component patch on a single uncolored result. Creative washing retains
  the original and grants the cleaned result on every use.
- Shared ItemUtils filled-result exchange now compares item AND components across
  the inventory, preserves creative originals, discards uninserted limited creative
  results, replaces exhausted survival hands and drops overflow without ownership.
  Existing bucket-stack exchange delegates to this helper.
- Filled cauldron collision callbacks filter the wall/content-shape union. Water
  extinguishes fire; burning entities melt powder snow to water before lowering
  it, with player spawn protection and projectile-owner interaction rules. Lava
  clears freezing, ignites for 15 seconds and applies four lava damage. Negative
  fire-immunity countdowns survive extinguishing. Lava drip plugin levels report 3.
- Still open: the engine's swept collision/effect ordering and deduplication,
  vanilla lava-hurt sound gating/entity random stream, broader inventory insertion
  semantics and the legacy main-hand bucket-use path. These are source changes,
  not a completed parity result. Rustfmt and diff whitespace review only; no
  compilation, tests or gameplay runs.


## Continued source port: barrels and ender-chest lifecycle

- Removed placement-time block-entity replacement for barrels and ender chests;
  placed container components now survive placement. Barrel removal notifies
  comparators, and both interactions open the menu before awarding the statistic.
- Barrels retain CustomName and unresolved LootTable/Seed in saved NBT, resolve
  loot only on inventory access/menu creation using available player luck/context,
  block spectators from generating unopened loot and restore container/name/loot
  item components during placement. Built-in block drops copy only the name;
  contents scatter separately. Empty saves omit
  Items; client update tags no longer expose the inventory. Menu titles use names.
- Barrel slot writes clamp item/container limits, whole-stack removal preserves
  removeItemNoUpdate behavior, and detached barrels stop viewer transitions.
  Barrel/ender-chest sounds use the level random pitch rather than a newly seeded
  generator. Removed ender chests no longer refresh viewer sounds/events.
- Ender-chest use requires an existing correctly typed block entity; it no longer
  fabricates one during a click. Waterlogging accepts the actual water fluid type
  and schedules water on neighbor updates. Only the redstone conductor above
  obstructs opening; vanilla does not apply the ordinary-chest cat restriction.
  Private ender-chest slot insertion now clamps stack limits.
- Remaining: shared lock item predicates, piglin guarded-container anger,
  exact loot-engine functions/random streams, nonplayer viewers and scheduled
  block-tick versus block-entity-tick recheck ordering. Existing player menu
  validity already checks the live block-entity/inventory identity and range.
  Source review/rustfmt/diff checks only; no compilation or tests.


## Continued source port: ordinary, trapped and copper chests

- Double-chest inventory/menu/comparator selection now shares the source neighbor
  combiner: same block identity, opposite non-single halves, matching facing and
  correct block-entity type. Invalid partners fall back to a single inventory;
  only a valid blocked partner blocks both halves. Right-half inventory comes
  first. Sitting cats in the one-block space above now obstruct normal chests.
- Removed manual placement/break partner edits and replacement BE creation;
  existing source shape callbacks reconcile halves. Waterlogging reads the actual
  fluid, sneaking uses the secondary-use flag, removal notifies comparators, and
  the open statistic follows menu opening only when a menu provider exists.
- Chest/trapped-chest storage now binds its world, persists custom names and
  unresolved loot alongside actual contents, and resolves loot on inventory
  access or menu creation with available player luck/context. Merely requesting
  an obstructed menu no longer generates loot. Spectators cannot generate either
  unopened half. Double-chest titles prefer the first half's custom name, then
  the second's. Hopper discovery uses the same combiner without lid obstruction.
- Added component restoration, source stack limits, empty client update tags and
  detached-viewer guards. Built-in chest/barrel drops copy only custom_name and
  scatter contents separately; the barrel implementation was corrected during
  this review to avoid retaining a second copy of its contents in the dropped item.
- Chest sounds use the level random stream and double-chest midpoint; copper
  hinges use normal/weathered/oxidized sounds. Copper oxidation requires a live
  chest BE. Its active-viewer check still uses the tracked count, so stale-count
  timing remains open. Trapped-chest experimental redstone orientation is open.
- Shared barrel/chest/shulker cleanup preserves tag-provided pending loot when no
  loot component overrides it, clears raw inventory without generating loot, and
  marks partial removal dirty only when a stack was removed.
- Remaining: lock predicates/notifications, piglin anger, exact loot functions and
  RNG, nonplayer opener accounting, shared update/effect order and client protocol
  details. Source inspection, rustfmt and diff whitespace review only; no build,
  compilation, test or gameplay commands were run.


## B01: bounded workstation source pass

- Reviewed eight workstation handlers; fixed menu-before-stat order, horizontal
  loom placement and enchanting provider/transmitter tag geometry. Enchanting
  names now survive placement, NBT, block drops and menu titles; no click-time
  entity is fabricated and no placement-time entity is replaced.
- Fletching remains a plain pass-through block. Grindstone survival/placement and
  the existing stonecutter facing/path behavior match the inspected local methods.
- Menu algorithms and live enchanting bookshelf refresh are explicitly queued in
  I02, shared shapes/registry coverage in B14. This is a first source pass, not a
  claim of 1:1 completion.
- Background focused block-entity tests were authorized and launched. The first
  build exposed a prior PotDecorations NBT string conversion error, now fixed.
  A second attempt is running; there is no passing test result yet.

- Background verification update: regenerated stale absolute asset includes left by
  an old worktree cache; replaced the removed tall-flower tag with the actual
  DoublePlantBlock families. Compilation now reaches the main crate and reports
  prior import/API/type integration failures. D07 tracks these separately; tests
  have not passed.

## D07: background compiler integration repairs

- Fixed stale module/trait imports, fluid-state access, entity-type comparisons,
  callback Arc access, level nextLong support and narrowed plugin ABI adapters
  across the earlier block/bee ports. No plugin submodule edits.
- Focused block-entity tests are rebuilding in the background. The previous
  attempt failed during compilation; no passing test result is claimed.

## B02: inventory automation source pass

- Compared HopperBlock, DispenserBlock, DropperBlock and CrafterBlock plus their
  relevant BE methods and DefaultDispenseItemBehavior against local 26.2 source.
- Preserved existing inventories on placement; initialized new BEs from actual
  state; fixed menu-before-stat order/type checks and removal comparator updates.
- Dispenser/dropper slot selection now uses vanilla reservoir sampling on level
  RNG; empty droppers fail-click and empty dispensers emit block_activate.
- All three ejectors share source position/velocity and zero pickup delay.
  Dropper/crafter insertion respects the destination face, and crafters find
  automation containers (including double chests and inventory entities).
- Hopper collection covers Y=11/16 through Y=2 and entity-entry callbacks, skips
  removed entities, and reports pickup success only for complete absorption.
  Container entity selection uses level RNG; crafter automation balances slots.
- Open dependencies are listed under B02 in PARITY_PLAN.md; this does not claim
  complete container, recipe or per-item dispenser parity.
- D07 repairs passed 28 existing block-entity tests and then all 123 existing block
  tests. The B02 changes are rebuilding in a background test run.

## B03: special gameplay block source pass

- Compared eight block handlers to local 26.2; removed placement-time BE replacement
  for spawners, trial spawners, vaults, hearts and wither skulls. Beacon use now
  succeeds without a BE and opens a correctly typed menu before its statistic.
- Hearts require tagged pale-oak logs/wood aligned to their axis, activate from
  environmental creaking activity, and schedule shape checks one tick later.
  Neighbors no longer forcibly reset an awake heart. Removed unconditional extra
  heart drops/sounds; added natural-heart player-break XP and comparator removal.
- Vault uses its own placement properties, only accepts nonempty-hand interaction
  while active and checks configured key components/count. Its larger reward/state
  implementation remains incomplete and is explicitly queued.
- TNT emits attributed prime_fuse, awards item-use statistics, checks projectile
  interaction permission, preserves owner player credit and uses level-RNG short
  fuses. Unstable priming now runs before destruction without prematurely removing
  the block.
- Upright iron/wither patterns require their air corners. Clear events carry each
  original state, neighbor notifications occur after spawning, golems use +0.05Y
  and player-created iron state; withers use base+0.55Y, orientation and peaceful
  restrictions. Rotated/copper patterns and entity lifecycle remain open.
- B02 passed all 123 existing block tests. B03 is rebuilding in the background;
  see PARITY_PLAN.md for its unresolved dependencies. No 1:1 completion claim.

## B04: signs, banners, heads and light source pass

- Compared all four sign variants, banner variants, skull variants and light with
  local 26.2 block and standing/wall item sources. Removed BE replacement from
  sign, banner and skull placement.
- Banners now select standing/wall variants in placement direction order, test the
  correct support and immediately return air on support shape loss.
- Signs now use nearest-direction standing/wall selection, horizontal beam support
  for wall-hanging placement, center support for ceiling signs and source-aligned
  chain attachment/rotation. Wall-hanging survival retains vanilla's inherited
  behavior; removed permissive leaf/sign shortcuts.
- Sign edit checks and commands use the player's filtered text selection; styled
  plain text remains editable. Editor ownership clears on BE ticks when players
  disconnect/leave range. Wall-sign front/back uses the signboard center; successful
  applicators emit block_change with player/state context.
- Light levels require game-master permission, update listeners only, and no longer
  incorrectly cycle through block replacement placement. Item block-state components
  remain in the item pass. Skull neighbor updates reject a replaced block.
- B03 passed all 123 existing block tests. Initial B04 changes also passed 123; the
  final text/editor corrections are rebuilding in the background. Dependencies
  remain explicit in PARITY_PLAN.md; no complete 1:1 claim.

## B05: portals and gateways source pass

- Compared NetherPortalBlock, EndPortalBlock, EndGatewayBlock and EndPortalFrameBlock
  to local 26.2; retained the frame's facing/eye comparator and path behavior.
- Removed End portal/gateway BE replacement; added the End portal's Y=6/16..12/16
  inside shape and alive/nonpassenger eligibility to all portal handlers.
- Nether delay uses player invulnerability and clamps negative rules; neighbor
  preservation tests portal block identity rather than exact state equality.
- Portal piglins consult the environmental attribute and shared spawn-floor
  predicate, use nonspectator horizontal distance from chunk center and enter the
  normal entity-save lifecycle.
- B04 final corrections and B05 changes each passed all 123 existing block tests
  in background runs. Transition/gateway/entity dependencies remain queued.

## B06: administrative and invisible block source pass

- Compared barrier, structure void, command blocks, jigsaw, structure block, test
  block and test-instance block sources (six Rust handlers). Barrier/source-water
  placement and structure-void's default server handler are retained.
- Editor access checks concrete BE types and creative+game-master permission.
  Jigsaw front now follows the clicked face; mirror axes match vanilla.
- Command/jigsaw/test placement preserves existing BEs. New command/test BEs take
  chain-auto/mode from block state. Player command placement initializes feedback
  defaults only without custom BE data and processes the initial power edge.
- Command execution refreshes comparators; narrow signal output saturates instead
  of wrapping. Structure placement records its player author.
- Corrected sign placement to test source WATER specifically, matching the Java
  fluid-type comparison. B06 passed all 123 existing block tests in the background.
  Remaining execution, structure and protocol work is listed in PARITY_PLAN.md.

## B07: redstone and piston source pass

- Reused the earlier rail, switch, comparator/repeater, torch, sensor and target
  ports. Reviewed remaining redstone handlers, wiring and piston resolver/events.
- Copper bulbs evaluate power after placement. Daylight detectors preserve their
  BEs. Added stale-callback guards and removed-entity filtering for plates/tripwire.
- Wire removal now runs for all replacements with the piston-move exception,
  evaluates old-state power without restoring a removed wire, advertises signal
  capability, and respects mayBuild when changing dot/cross shape.
- Repaired diode removal's always-true early return and source-block notifications.
- Piston head removal drops its fitting base, with creative pre-removal suppression;
  removed the base handler's redundant deletion of unrelated moving pieces.
- Sticky resolver visits newly appended branches. Retraction uses the event type,
  actual pulled position/direction and extending moving-piece finalization. Piston
  sounds use level RNG; state-attributed activation/deactivation events now emit.
  Placement preserves an existing mover and player placement checks power again.
- Aligned local move/retraction flags with the available shape/BE side-effect flags.
  Shared MOVED flag semantics and simplified moving-BE collision remain explicit
  dependencies; this is not a claim of complete piston or redstone parity.
- Background runs 13–15: 123 block tests passed each. Full library run: 446 passed,
  two localhost-bind tests failed under sandbox restrictions; those exact two
  passed on the authorized unsandboxed rerun. All 448 passed across the two runs.
  Final flag-only follow-up is included in the next background block run.

## B08–B12: building fixes and carried source reviews

- Building source pass: ladders now validate their stored facing, choose context
  directions and retain/schedule water. Lanterns reject unsupported placement and
  validate the selected orientation. Torches share standing/wall selection order.
- Stairs use shape callbacks and the exact half-height boundary; slabs schedule
  water ticks, including unwaxed copper wrappers. Fence/pane/bar connections update
  only the notified side. Fences now bind the player's existing leashed mobs using
  the shared lead helper, even with an empty hand.
- Scaffolding now considers horizontal support even above another scaffold and
  updates distance/bottom on a scheduled tick. Newly unsupported supported blocks
  drop; already-distance-seven blocks become falling entities.
- B09 carries the documented vegetation, crop and growing-plant ports; checked
  the simple root/flower/cactus-flower, beetroot/nether-wart and shared module paths.
  B10 carries bee/sculk passes, B11 terrain/physics passes, and B12 all documented
  container/furnishing passes. Their existing limitations stay in PARITY_PLAN.
- Background block runs 16–18 each passed all 123 tests. Existing unit coverage
  does not verify every changed gameplay path.
- User priority changed: move to mobs after the bounded block pass, keeping the
  original item queue and unresolved block engine dependencies for later work.

## B13/B14 transition to the requested mob priority

- Reused inherited amethyst, carpet, infested and pillar work. Checked remaining
  handlers/module routes; retained incomplete honey/landing/effect behavior as
  explicit shared entity work.
- Carving pumpkin seeds uses the interaction loot table and source launch geometry,
  velocity, sound, shear attribution and used-item statistic. Removed the custom
  vine item-use path so normal placement handles consumption/context; added slime
  stepping slowdown and its sneaking landing guard.
- Data check: all 265 block-tag and 1,113 block-loot JSONs match local vanilla.
  Asset default/shape references and ID uniqueness are structurally valid. See
  PARITY_BLOCK_DATA_AUDIT.md for scope; full runtime/shape/inheritance parity is open.
- Builds 19–20 exposed one private ItemEntity field access in carving; corrected to
  the EntityBase accessor. The corrected follow-up runs in the background.
- The bounded block handler pass is checkpointed. This does not complete block
  1:1 parity; the user requested mobs next, so open block dependencies and the item
  queue remain documented while work moves to mobs.

- Corrected final block background run 21: **123 passed, 0 failed**.


## Shared engine continuation — 2026-09-15

User priority changed to shared-engine/block dependencies before further mob work;
no full mob pass is underway. See ENGINE_GAPS.md for the implemented scope,
source references, verification and remaining dependencies.

- Fixed shape-update propagation flags and piston completion/drop/waterlogging.
- Added dynamic piston geometry, collision-clipped pushes, per-tick movement limits,
  slime velocity and honey carrying. Normal pushes include players.
- Shared axis-ordered collision clipping now also covers ordinary entities;
  scaffolding collision uses entity feet/descending context.
- Honey has source-based side sliding, fall-distance reset, sounds/status and
  advancement checks. The older inside-effect dispatcher used by piston pushes
  now uses the registered inside shape and independent fluid-height checks.
- Vaults now activate/detect eligible players, generate table-based rewards,
  unlock after 14 ticks, open/eject on 20-tick intervals, synchronize display data,
  throttle failures and persist ordered rewarded-player history and queued items.
- Command blocks count successful execution callbacks, track/persist last execution,
  stop repeated chain execution, respect max_command_sequence_length and keep
  repeat scheduling when command execution is disabled.

These changes do not close the exhaustive block or engine parity gates.


### Swept inside-block engine work

The endpoint-only inside-block loop now traverses recorded movement segments using
the Java 26.2 cell algorithm. Direct unmodified-server oracle evidence covers 120
cases of exact ordering/iteration numbers. Bubble precise-contact and filled
cauldron shape unions are wired into the shared dispatcher. Teleports clear
recorded paths; piston motion applies its own swept path immediately.

Step-based effect aggregation and ordinary fluid-phase integration remain open;
see ENGINE_GAPS.md and tools/vanilla/README.md. No full mob pass or final parity
certification was performed.

### Shared inside effects (engine-first continuation)

Fire, powder snow, layered/lava cauldrons and water/lava now defer shared effects
through Java's step-ordered collector. Primary effects deduplicate per step while
before/after callbacks retain order. Exact collector traces match 100 Java oracle
cases; background library verification passed 456 tests (two socket tests excluded).
Movement replay and other engine gates remain open in ENGINE_GAPS.md; mob passes
remain paused.

### Shared support and movement replay

Unified shape-based support selection, exact tie order, fence/wall offsets and
state-at-offset lookup. Movement records are bounded and reusable; resting item
entities replay their previous inside contacts and run base entity ticking. stepOn
now precedes inside effects for all affected grounded entities. Regression library
final run 3 passed 459 tests; see ENGINE_GAPS.md for integration evidence and the
remaining engine gates. Full mob passes remain paused.

### Voxel grids, step-up and borders

Preserved static voxel grids for every block state and used them for collision and
step-height selection. Shared movement now gathers entity and border collisions.
Border interpolation, saved settings, outside-border damage and packet timing are
implemented. Java step/border oracles and background checks passed; dynamic shape
construction and live gameplay remain explicit gates in ENGINE_GAPS.md. Mob passes
remain paused.

Static collision verification now covers 81 motions per block state (all 32,366),
matching Java output fingerprints. This verifies static clipping geometry and grids,
not each block's gameplay lifecycle or contextual/dynamic shape behavior.


### Dynamic piston grids and climbing

Piston collision unions now preserve source grid priority, optimized box order and
float-boundary snapping. 100,656 Java shape fingerprints pass. Shared climbing is
restored for tagged blocks and aligned open trapdoors, with the scaffolding/sneak
exception and exact Java float movement cap. Degenerate border collision is fixed.
Post-move side effects and the other engine gates remain open in ENGINE_GAPS.md;
full mob passes remain paused.


### Shared collision response

Ordinary and piston movement share landing/restitution/velocity effects, including
non-living block bounce and stuck-block reset. 800 Java restitution reference cases
match. Ridden-vehicle authority and full world integration remain open; this is a
shared-engine checkpoint, not a mob pass or a claim of complete block parity.


### Movement authority and fall-reset paths

Shared controller selection now determines server/client movement authority and
ridden step height. Fast movement checks its path for fall-resetting blocks, water
and qualifying player portals. Added collision flags and unified ordinary/controlled
air input and friction math. 600 Java ray cases and the 474-test background library
run passed; two socket tests were excluded as documented in ENGINE_GAPS.md.
Full mob passes remain paused. Double fall-distance persistence, general ray APIs,
vehicle/portal integration and the other engine/block gates still need work.


### Canonical fall-distance counter and landing dispatch

Entity/living/falling-block code shares a double counter and canonical
`fall_distance` NBT, with legacy saves accepted on load. Landing includes final
movement and preserves distance through damage recording. Shared landing dispatch
covers passengers and falling entities, including block-specific distance changes.
Java accumulation and precision regression checks accompany the implementation;
remaining fluid/particle/impulse/live-world gates are explicit in ENGINE_GAPS.md.


### Shared block ray targeting

Block rays now use Java shape clipping with explicit fluid modes, separate
waterlogged fluid surfaces, interaction-face overrides and contextual shapes.
Boats use exact intersections; fluid-using items respect intervening outlines.
Explosion visibility/shared target tracking use collision shapes. Java outline
and fluid-scene fixtures accompany this checkpoint; see ENGINE_GAPS.md for scope
and verification limits. Full mob passes remain paused.


### Landing fluid contact and currents

Landing refreshes fluid contact, water/stuck blocks reset the shared entity counter,
and lava reduces it once per base tick. Removed proximity-based fall immunity in
favor of actual contact/swept effects. Current response and chunk-margin gating now
follow Java's tracker; 1,200 real Java current cases cover the math. Remaining
particle/impulse/vehicle/live-world work is listed in ENGINE_GAPS.md.


### Landing dust packets

Added ordinary landing dust with the landing block state and Java positioning/count
rules. World particle recipients use the source distance limit, and block-particle
payloads use each client's state registry. 128 real Java packet fixtures match;
remaining mace/splash/other-particle/Bedrock/live-render gates stay open.


### Impulse-limited falls

Shared impulse context now covers wind-charge/mace protection, grace/persistence,
landing/reset hooks, mace dust and source fall sounds. Enchantment motion targets
the affected entity. 1,000 Java context cases and the 481-test background run pass;
ENGINE_GAPS.md retains the remaining integration and full-parity gates.

### Shared fluid and passenger contact

Fluid/eye tracking, boat passenger clipping, underwater ejection, base vehicle
fluid/fire ticks and splash sound/vibration are connected. Shared world currents
now include downward channels through empty neighbors and Java float/normalization
semantics. See ENGINE_GAPS.md for bounded oracle coverage and remaining vehicle,
RNG, environment, lifecycle and live gameplay gates; this is not whole-block parity.

### Shared auto-spin state

Riptide now starts the living spin lifecycle, adds its launch impulse, uses ordinary
collision for ground lift, dispatches touch attacks, and clears state on expiry or
contact. Player pose and impulse fall protection use that state. Trident release
retains active-hand identity and uses enchantment-selected sounds. See ENGINE_GAPS.md
for verification and the remaining shoulder/item-reference/client integration gates.

### Shared sound radius and source attachment

Block/item/component sounds now share Java's strict 3D recipient radius and direct
fixed-range handling. Trident launch sounds follow their source entity; generic
entity/impact sounds use source category, silence and sound-seed rules. Byte-level
Java packet comparisons and boundary checks are recorded in ENGINE_GAPS.md; legacy
client/Bedrock compatibility and live playback remain open.


### TNT ownership and shared reload motion

Primed TNT now persists fuse/block/power/owner and carries living-owner identity
through chain reactions and explosion damage. Priming RNG, gravity/drag/bounce,
explosion height and teleport-specific portal protection follow the inspected
Java paths. Chunk loading preserves saved motion; numeric codec and motion limits
are shared by all entities. ENGINE_GAPS.md records the differential fixtures and
remaining world-transfer, TNT-minecart and live-gameplay gates. Full mob passes
remain paused.


### Portal world and passenger transitions

Non-player portal transfers now move world membership through UUID-preserving
entity replacement; nested passengers and projectile/TNT owner references follow.
Shared relative motion rotates with portal axes, and block shape pushes preserve
riding relationships. Temporary portal loading/ticking tickets support arrivals
without nearby players. The two-world integration test and Java transition
comparisons are documented in ENGINE_GAPS.md, alongside remaining ticket
persistence, End rules, scheduling and client gates. Full mob passes remain paused.

### Ticket persistence and queued block work

Portal/forced tickets now persist and keep distinct entity/block ticking areas.
Block entities, scheduled work and block events use the block area; inactive work
is retained. Scheduled queues respect Java's container merge and independent
block/fluid limits, and fluid collection follows block callbacks. Block events
retain their target type and deduplicate while pending. ENGINE_GAPS.md records
Java differential and World integration evidence, plus remaining ticket-only
entity loading/readiness and live lifecycle gates. Full mob passes remain paused.

### Ticket-only saved residents

Chunk tickets now load and activate saved entity storage without a viewer.
Scheduled block/fluid callbacks wait for that activation, so ticket-driven block
work no longer runs before saved residents are available. Concurrent storage
requests share one result; nested passenger data and repeated root snapshots are
covered by restart checks. ENGINE_GAPS.md records remaining complete-snapshot,
passenger-tree unload, player-vehicle and chunk-holder readiness work. Full mob
passes remain paused.
