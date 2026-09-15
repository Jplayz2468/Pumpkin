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
