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

1. **D01: movement and inside effects.** Full piston movement side effects,
   remaining contextual voxel-grid construction and remaining
   specialized movement/inside-effect lifecycle paths. Step-up and entity/border
   collision gathering are implemented below; gameplay integration remains unproven.
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

Remaining for this pipeline: other direct entity-specific position changes, per-entity sound randomness/categories and full gameplay
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

## Support selection and movement replay continuation

- Shared support selection queries actual collision shapes under a thin feet box,
  chooses nearest block center and resolves exact ties by greatest Y, then Z, then X.
  Ground transitions and horizontal fallback follow Entity.checkSupportingBlock.
- Java/Bedrock player movement, ordinary server movement and vertical piston pushes
  update that shared support. Removed the player's non-air/guessed-block selection.
- Legacy movement/landing offsets retain fences/walls/gates where Java does, widen
  float offsets exactly, and fetch the state at the resulting Y instead of returning
  the old support state paired with a different coordinate.
- Movement history combines oldest paths at 100 pending entries, retains completed
  paths for replay and uses old position for the no-record fallback.
- Dropped items now run base entity ticking (fluid state/fire/portal updates), use
  tick count rather than despawn age for rest scheduling and replay prior contacts
  on skipped movement ticks. This does not certify all dropped-item behavior.
- Grounded stepOn callbacks now run before shared inside effects for all affected
  entities; removed the separate living-only late callback and extra below-block call.
- Regression checks cover nearest/tied support, fence footprints, bounded movement
  history, unchanged replay while new movement is pending, and fallback segments.
- Final background library run 3 (including shared stepOn): **459 passed, 0 failed**,
  with two previously separately passing socket tests excluded. No live world/client
  comparison.

## Voxel clipping, step-up and world borders

- Movement now clips using voxel coordinate grids and occupied boxes together,
  including entities already overlapping a partial block. Exported all 32,366 static
  state grids; checked that every existing collision box endpoint belongs to its grid.
- Added Entity.collide's grounded/landing branch, STEP_HEIGHT, sorted float step
  candidates, skipped height and first horizontal improvement rule. Piston movement
  uses the same collision query. Block queries retain the whole intersecting shape.
- Movement gathering includes collidable entities, excludes spectators/removed
  entities/shared vehicle trees, and includes the border only near its inner margin.
  Shared collision predicates distinguish boats, pushable vehicle contacts and
  conditional shulker/happy-ghast solidity. No full per-mob pass was performed.
- Border bounds follow tick interpolation, previous-tick extents, absolute clamping,
  rounded collision planes and interrupted size changes. Commands query current size.
  Outside-border damage uses the buffer and damage rate. Timed changes and settings
  save/load through each dimension's canonical data/minecraft/world_border.dat,
  including periodic autosave. Packet durations/warning times adapt at 1.21.11.
- Java oracle: 400 clipping/step cases and 180 border state transitions passed.
  Background run 9: pumpkin **464 passed**, pumpkin-data **73 passed**,
  pumpkin-protocol **111 passed**; two previously separately passing socket tests
  excluded. All 32,366 static states match Java clipping fingerprints for 81 motions
  each (2,621,646 simulated motions). This uses empty collision context at the origin.
  Final call-site refinement run 10 passed the same **648 tests** across the three
  crates, with the same two socket-test exclusions.
- Still unproven: exact optimized grids for dynamic moving-piston unions and other
  contextual shapes, collision/query ordering under live movement, full piston
  restitution and packet validation. No live server/client comparison this pass.

## Dynamic piston unions and shared climbing

- Moving piston shapes now join the stationary base and moving block's original
  voxel grids before Java's ordered union optimization. Epsilon-close planes retain
  the source merge priority; nearly aligned boxes snap to Java's cube grid.
  Movement gathering uses this complete shape directly rather than rebuilding it
  from translated world boxes.
- Java oracle fingerprints cover all coordinate values, box counts and ordered
  box geometry for **100,656 cases**: every state of ten representative blocks,
  all six directions, extension/retraction, source/non-source, nine progress values
  including float precision boundaries, and matching direction-specific NOCLIP.
- Retraction push geometry uses the moved base's facing, sticky type and short-head
  threshold. Shared piston filtering now also excludes Java's non-physical display,
  marker, interaction, area-effect-cloud and ominous-spawner entities.
- Replaced the disabled climbing predicate with climbable tags, spectator/gliding
  exclusions and open trapdoors aligned with the ladder beneath them. Climbing is
  refreshed before travel and after moving; limits widen Java's 0.15F exactly.
  Sneaking players hold ladders/vines while retaining downward scaffolding travel.
- An absolutely clamped border with an empty rounded interior now produces one
  infinite collision shape, matching the complement of Java's empty interior.
- Verification: targeted Java piston comparison passed all 100,656 cases. Final
  background library run 6 passed **469 tests**, with the two previously separately
  passing socket tests excluded. No live gameplay or full mob pass was performed.

## Shared movement response

- Ordinary travel and piston displacement now share their post-collision phase:
  horizontal flags, support/ground state, living fall handling, velocity restitution,
  bounce events and horizontal block-speed factors. The former full restitution
  path was limited to controlled living entities; other entities now use it too.
- Piston pushes consume stuck-block multipliers and reset travel velocity without
  scaling the external push. Unobstructed displacement preserves the entity's own
  travel velocity; collision restitution changes that velocity rather than replacing
  it with the requested displacement. Non-living block bounce scales by 0.8F.
- Players retain separate horizontal-push ground/fall handling, but server-side
  Player.canSimulateMovement still allows their restitution. Riding-vehicle authority
  remains an explicit missing shared API; see next work below.
- Dropped-item friction uses the correct supporting-block offset and one float
  drag factor (removing the extra drag multiplication); residual downward ground
  velocity rebounds by -0.5 as in ItemEntity.tick.
- Movement emissions skip passengers. No-physics movement preserves onGround while
  clearing horizontal collisions. Position changes and movement recording honor
  Java's tiny-clipped-motion threshold.
- Java's actual private restitution method matches **800 seeded cases**, including
  non-living block bounce, suppression, gravity/drag compensation and NaN results.
  The oracle explicitly binds honey's suppresses_bounce tag because Bootstrap alone
  does not load datapack tags. Background library run 2 passed **471 tests**, excluding
  the two previously separately passing socket tests. Final call-site and item
  friction run 5 also passed **471 tests** with the same exclusions.

## Vehicle authority, fall-reset rays and shared air movement

- Added first-passenger controller selection with Java's Mob fallback, boats,
  saddled equines/camels/nautiluses, steering-item checks and happy-ghast harness/
  timeout checks. Existing equine saddle state is now exposed through Mob's hook.
- Client authority follows the controlling-rider chain with cycle protection;
  collision ground/fall handling and restitution use the new predicates. Players
  retain Java's server-side canSimulateMovement override. Living travel skips
  client-authoritative vehicles and player-controlled living vehicles step at least
  one block. Full ridden AI/packet integration remains unproven.
- Added Java's block-ray traversal and boolean voxel-box clipping for the movement
  fall-reset ray. A movement of at least one block checks up to eight blocks ahead
  against full resetting-block cubes and water height, including flowing water.
  Player-only portal exceptions respect the instant nether-portal gamerule.
  Living/falling-block counters reset before movement is applied.
- Vertical, below and minor collision state are now retained alongside horizontal
  state. The server's default minor-collision predicate is false; no-physics moves
  clear collision flags while leaving ground state alone.
- Shared air-drag hook connects bee/parrot omnidirectional movement and restitution.
  Ordinary living air movement now uses the same float speed/friction/input-vector
  helpers as controlled living movement, including friction/air-drag attributes.
  Levitation arithmetic no longer fuses Java's separate multiplication/addition.
- Verification: controller-policy branch checks and **600 Java ray traversal/clip
  cases** passed. Background library run 5 passed **474 tests**, with the same two
  previously separately passing socket tests excluded. No full mob pass or live
  server/client comparison was performed.

## Shared double fall-distance continuation

- Entity owns the canonical double fall-distance counter. LivingEntity and
  FallingEntity retain aliases to that same counter, so block effects and external
  changes no longer update disconnected stores. Base entity NBT writes the canonical
  double `fall_distance`; loading accepts old float/legacy keys after the canonical
  value. Removed the living-only dead-entity save/load clamp.
- Accumulation follows Entity.checkFallDamage exactly: each downward movement is
  cast to float then widened into the double sum; water suppresses accumulation.
  Landing includes its final downward displacement, keeps the counter visible to
  damage/combat callbacks, emits HIT_GROUND and clears it afterwards.
- Landing distances and block callbacks use doubles. CombatEntry deliberately keeps
  Java's float snapshot, and the existing WIT v0.1 float ABI converts only at its
  boundary. Mace and gliding calculations now consume the wider engine counter.
- Default, bed, hay, honey, slime, farmland and dripstone landing handlers route
  through shared causeFallDamage. This includes passenger propagation and falling
  blocks; removed the falling entity's separate post-move landing accumulation.
  Honey/bubbles/geyser resets and powder-snow collision read the common counter.
- Added precision/legacy-NBT regression checks and **1,200 Java accumulation cases**.
  Background run 4 passed **476 tests**, excluding the two previously separately
  passing socket tests. Final run 5, including all 1,200 oracle cases, passed
  **477 tests** with the same exclusions.
  This verifies bounded algorithms and compilation, not live landing/passenger play.

## Next shared-engine work

1. General block rays now share source-verified traversal and shape clipping (see
   checkpoint below). Remaining ray integration includes entity-AABB targeting,
   callers that still omit an entity context, and live world/client verification.
2. Complete ridden vehicle integration (controlled vehicle/navigation/packet paths),
   direct movement and portal/passenger transitions, remaining contextual shapes and
   live gameplay verification. SulfurCube's omnidirectional override belongs to its
   still-missing entity implementation; the shared hook is present.
3. Finish fall/inside-effect integration: splash effects, complete fluid/eye tracking,
   vehicle passenger boxes, remaining teleport transitions and auto-spin state.
   Landing dust, impulse fall-damage context, fluid refresh, common water/stuck
   resets and removal of proximity-based exemptions are implemented below.
4. Continue D02–D06 and remaining block-system gates listed above. Do not treat this
   bounded movement checkpoint as full engine or block parity.

## Shared block ray and fluid clipping checkpoint

- Replaced duplicate block DDAs and slab/full-cube fallbacks with Java traversal
  and `VoxelShape.clip`: original endpoints, short-segment rejection, inside probes,
  entering-face tie order, nearest component selection and exact hit positions.
- Removed synthetic water geometry from block outlines. Waterlogged blocks now
  participate through explicit NONE/SOURCE/ANY/WATER fluid selection and actual
  fluid shapes. The closer block/fluid hit wins; block geometry wins a distance tie.
  Java 26.2 caches a fluid state's first queried shape height; the shared ray cache
  preserves that behavior, including fall-reset rays. Live fluid physics retains
  uncached world height.
- Exported every nonempty Java interaction shape (59 states, seven shapes) for
  cauldrons, composters, hoppers and scaffolding. A closer interaction volume changes
  only the face of an existing block hit.
- Shared contextual collision boxes between movement and rays, including moving
  pistons, open shulkers, powder snow and scaffolding. Entity-aware outlines also
  honor held light blocks/scaffolding. Player/item targeting supplies this context.
- Bucket, bottle, water-placement and spawn-egg rays stop at intervening outlines
  and use the source fluid mode; boats use ANY and the actual intersection position.
  Water-placement items validate the existing block support rule after targeting.
  Spawn eggs still require a liquid block after the source ray; no mob pass was done.
- Explosion exposure and shared target tracking now use collision shapes and the
  tested entity's context instead of outlines or a solid-block prefilter.
- Java probes: 5,000 real block outline/interaction cases compare exact position
  bits, direction and inside flags; 1,000 block/fluid scenes cover the four fluid
  modes; the existing 600 traversal/water-height cases remain. Fluid-scene tests
  use recorded uncached Java heights and preserve its shape-cache query sequence;
  they do not certify live world fluid refresh. Bootstrap's
  water tag is explicitly bound to the two built-in water fluids in that probe.
- The Java probes exposed synthetic water outlines and missing fluid-shape caching.
  Final background run 6 passed **479 pumpkin + 73 pumpkin-data tests**, with only
  the two previously separately passing socket tests excluded. No live server,
  client or contraption comparison was performed.
- This closes the old general block ray fallback/selection gate. Entity ray slab
  semantics, remaining context-free callers, packet/menu integration and all other
  D01–D06/live-world gates remain open. Full mob passes remain paused.

## Landing fluid refresh and current integration

- `LivingEntity.fall` refreshes fluid contact before reading/accumulating distance
  when previously out of water. Landing block-change callbacks use the on-block
  position rather than the entity's feet cell.
- Water and stuck-block speed changes reset the canonical counter for every entity.
  Lava halves that counter once in base tick after fluid/fire processing, not during
  each fluid refresh. This includes non-living entities and avoids reducing it again
  when landing refreshes fluid contact.
- Removed the legacy radius-based immunity near ladders/cobwebs/powder snow/slime
  and feet-cell exemption. Actual swept reset rays, inside effects, fluid contact
  and landing block callbacks now decide the effect instead of nearby block names.
- Fluid scanning uses ceil(max)-1 bounds, requires the surrounding X/Z chunk margin
  to be loaded, and derives contact from positive fluid depth. It keeps current
  accumulation order and uses the full Java lava-current scale.
- Current response matches `EntityFluidInteraction.Tracker`: accumulated-current
  epsilon, player averaging/non-player normalization, Java division/normalization
  threshold, and the minimum impulse test on the impulse itself rather than the
  entity's velocity. Contact/reset state updates precede current application.
- A probe invokes Java's actual private Tracker on real Entity/ServerPlayer objects.
  All **1,200 exact-bit current cases** passed in background library run 2, along
  with the existing suite. Final background run 3 also passed **480 tests**, with
  only the two previously separately passing socket tests excluded, including the
  subsequent stuck-block reset and ray-direction normalization edits.
- Still open: landing/splash particles, impulse damage context, teleport reset paths,
  full eye-fluid/boat passenger-box integration, dynamic FAST_LAVA environment
  attributes, and live chunk-edge/landing/vehicle verification. This is not complete
  D01 or whole-engine parity; full mob passes remain paused.


## Landing dust and particle payloads

- Living landing dust now uses the pre-accumulation fall distance, safe-fall
  attribute, Java float/double constants, capped count and support-edge position.
  The original on-block state is retained across the block-change callback.
- Added a block-particle sender with the block-state payload. The packet codec
  remaps that payload for each client's block registry (including legacy block
  metadata packing/name form); previously only the particle type was remapped.
- World particle senders now share ServerLevel's strict center-distance recipient
  limit: 32 blocks normally, 512 with the distance override.
- **128 real Java 26.2 particle packet byte strings** match exactly. Client-state
  mapping has regression coverage; final background run 3 passed **480 pumpkin +
  113 pumpkin-protocol tests**, excluding the same two socket tests.
- No live particle render verified. Mace extra-landing dust, splash effects,
  non-Block particle payloads and Bedrock particle translation remain separate
  gates, along with the listed impulse/fluid/vehicle and D02–D06 work.


## Shared impulse fall protection and landing effects

- Living entities persist the source impulse impact position and grace counter,
  including the legacy-named `current_explosion_impact_pos` NBT field. Grace ticks
  after movement. Disabling new protection clears grace while retaining the impact
  until the normal reset path, matching Java's setter semantics.
- Fall distance is limited by the impact height before passenger propagation and
  damage calculation. Landing above the impact resets immediately; landing below
  respects grace, and positive fall damage clears the context. Player distance
  statistics use the original distance before this adjustment.
- Player wind-charge explosion hits, mace impact-height reuse/vertical velocity,
  grounded-mace extra landing dust and enchantment post-impulse grace are connected.
  The enchantment motion update now addresses the target player rather than a
  different enchantment owner.
- Reset hooks cover accepted player landing/liquid/climbing/spectator/gliding
  movement, stuck blocks (including flying-player behavior), game-mode changes,
  respawn, ender pearls and successful random-consume teleports. The auto-spin hook
  uses the existing stub and still needs the actual riptide/auto-spin state engine.
- Fall sounds use calculated damage and play before damage application, with the
  player's/monster's/default sound family and the actual block fall sound. Silent
  entities suppress both sounds. Per-mob overridden fall behavior still belongs to
  the paused mob passes.
- **1,000 real Java LivingEntity/MaceItem transition and fall-clamp cases** match,
  including NBT roundtrips. Final background run 4 passed **481 tests**, with the
  same two previously separately passing socket tests excluded. This does not
  verify live wind-charge/mace/teleport/packet sequencing or all advancement state.
- Remaining engine gates stay open; in particular fluid/eye/vehicle interaction,
  splash effects, auto-spin, full teleport/passenger transitions, D02–D06 and the
  outstanding block-system/live-world comparisons. Full mob passes remain paused.

## Fluid tracker, boat passenger contact and splash continuation

- Shared fluid tracking now retains eye contact separately from body contact and
  previous-tick eye contact. `isUnderWater` uses Java's previous-eye/current-body
  rule; lava contact respects `firstTick`. Player respawn clears these transient
  fields. Existing lava consumers use the shared accessor.
- Fluid interaction boxes normalize crossed endpoints as Java AABB does, including
  zero-sized entities. A boat clips its passenger's fluid box above the hull unless
  submerged. Boats and minecarts now call the base fluid/fire/portal tick. Boats
  reset their underwater counter immediately out of water and eject after 60 ticks
  of actual hull submersion, rather than treating any water contact as submersion.
- Splash entry emits the source sound and SPLASH vibration event, with controller
  velocity, source float volume/pitch and player self-exclusion. Ordinary entity
  sound silence and the Player override are respected. Server Level.addParticle is
  a no-op: splash particle arguments still advance the entity random stream.
- Added an entity-owned legacy random stream for these shared effects, extinguish
  pitch and push-out speed; float push-out arithmetic and immediate fire-immune
  extinguishing now match source. This is a foundation, not completion of D04's
  remaining random consumers or initialization order across subclasses.
- The actual Java fluid tracker matches 500 controlled scenes, including eye/box,
  current and chunk-query order. The oracle initializes the chunk's `fluidCount`
  (not merely `tickingFluidCount`) and explicitly binds fluid tags. Actual Java
  splash execution matches 300 volume/pitch/following-RNG cases. Background run 3
  passed **483 tests**, with the same two separately passing socket tests excluded.
- World flow now includes empty neighboring cells above a lower fluid channel,
  uses NORTH/EAST/SOUTH/WEST order, performs height subtraction in float precision,
  and uses Java's division/epsilon normalization. Added 1,600 actual Java block
  neighborhoods exercising the production registry/height/motion/face data.
  Final background run 6 passed **484 tests**, with the same two socket exclusions.
  These comparisons do not exercise a live chunk loader, passenger tick ordering,
  network playback, or full vehicle physics.
- Remaining: complete vehicle status/buoyancy/control/packet integration, dynamic
  FAST_LAVA attributes, swimming/auto-spin state, remaining RNG consumers, full
  teleport/passenger transitions, live chunk-edge/client gameplay, and the other
  D02–D06/block-system gates. No full mob pass was performed.

## Auto-spin/riptide shared lifecycle

- Added living auto-spin lifetime, damage/weapon context and synced flag. The push
  phase checks the union of pre/post travel boxes before ordinary entity pushing,
  hits the first living result, stops and rebounds velocity by -0.2. An empty query
  plus horizontal collision stops the spin; a query containing only non-living
  entities deliberately does not. Expiry clears the flag, damage and weapon.
- Player pose and accepted-movement impulse reset now read actual auto-spin state
  instead of the constant-false stub. Respawn clears it. Shared living flag changes
  preserve other flag bits atomically and avoid unchanged metadata updates.
- Trident launch uses Java lookup-table trig and float arithmetic, adds the impulse
  to existing velocity, starts 20 ticks at damage 8, and applies the grounded
  1.1999999F lift through normal collision movement. It schedules velocity sync.
- Player touch attacks use the retained spin weapon and damage, with the currently
  equipped attack-speed attribute. A hotbar swap no longer damages the replacement
  item: durability follows the retained weapon UID within the player's inventory.
  Trident release addresses its active hand and rejects a replaced active stack.
- The enchantment generator/data now retain TRIDENT_SOUND lists. Selection uses the
  highest applicable enchantment level, clamps to the list length, and falls back
  to the throw sound. Launch/release awards the trident-used stat.
- Actual Java LivingEntity.checkAutoSpinAttack matches 500 lifetime/contact/rebound
  cases. Another 1,000 numeric cases check the source launch expression using real
  Java Mth; they do not invoke the complete TridentItem release lifecycle. Background
  run 2 passed 486 tests with the same two socket exclusions, before the final
  active-hand/sound-selection/flag integration checks.
  Final background run 4 passed **487 engine tests** after those changes; run 3
  also passed **73 data tests**, and the updated code generator passed cargo check.
  The same two previously separately passing socket tests remain excluded.
- Still open here: shoulder-entity release (the player shoulder storage/lifecycle
  is absent), complete shared item-reference identity when a spinning weapon leaves
  the inventory, spatial-query ordering, sound recipient/attached-entity packets,
  Bedrock spin metadata and live client collision/launch verification. These are
  explicit integration gates, not grounds to call D01/D04/D06 complete. Full mob
  passes remain paused.

## Shared sound delivery

- World registered, component and custom positional sounds now use ServerLevel's
  strict 3D recipient radius at the original double position. Normal range is 16
  blocks, scaled by float volume above one; direct SoundEvents honor fixed range.
  Removed the chunk-square approximation and unbounded component/custom broadcast.
  Player-excluded sounds use the same filter and still draw one world sound seed.
- A canonical registry probe confirms all built-in Java 26.2 SoundEvents use
  variable range. Direct component events retain independent fixed-range values.
- Added a shared entity-attached sound sender. Riptide attaches to the player and
  a thrown trident attaches to the projectile. Pre-1.14 clients receive positional
  fallback using the same seed and recipient filter.
- Generic entity sound calls now use the entity's category and Silent flag, plus
  Player's local-prediction exclusion/override. Arrow/trident impact calls use this
  path instead of chunk broadcasts with a constant zero seed; randomized ground
  and arrow-hit pitch advances the shared entity RNG.
- **320 actual Java 26.2 packet byte strings** (160 positional + 160 attached)
  match, including inline fixed-range events, negative positions/entity IDs, float
  fields and seeds. Recipient regression checks cover vertical/diagonal/exact-range
  edges and filtering before packet-coordinate truncation.
- Background run 1 passed **488 engine + 114 protocol tests**. Run 2 also passed
  **488 engine tests** after generic entity/projectile sender integration; the same
  two previously separately passing socket tests were excluded.
- Still open: Bedrock sound translation/live playback, older-client direct-holder
  named-sound fallback and category/name compatibility, remaining entity sound
  overrides/RNG consumers, and full projectile behavior/spatial lifecycle. Packet
  and radius checks do not certify live client audibility or the complete engine.

## Shared projectile rays and border impacts — 2026-09-16

- Removed three duplicated block/entity collision loops from thrown items, arrows
  and tridents. They now use contextual block shapes and their actual entering
  faces, instead of guessing a face from coordinates in a whole block.
- Shared AABB rays use Java's strict entering-surface clipping, axis order and
  epsilon. VoxelShape's separate short inside probe remains intact. Projectile
  queries include players, exclude removed/spectator candidates, use the swept
  projectile box plus one block, and widen the age-dependent float margin exactly.
  Entity rays end at the selected block hit and retain the first equal-distance
  candidate in the existing world-query order.
- Queries run before movement; movement stops at the selected impact. Arrow and
  trident movement is recorded for shared swept inside effects. Border rays follow
  CollisionGetter's endpoint clamp and direction selection, including the float
  epsilon and very narrow borders. Border hits keep their marker so arrows and
  tridents reverse without lodging, consuming their hit flag or emitting land events.
- Java fixtures cover 1,200 nearest-entity queries/age margins, 500 border rays and
  200 actual Arrow border deflections (velocity, yaw and subsequent RNG state).
- Remaining projectile foundations: owner/vehicle-tree immunity instead of the
  five-tick shortcut, complete pickability/deflection predicates, arrow's many-hit
  query and same-tick piercing, movement/drag/gravity/base-tick ordering, previous
  rotation interpolation, other projectile families, and immediate inside-effect
  dispatch relative to hit callbacks. The shared spatial index still orders
  entities differently from Java's sections. These checks do not certify complete
  projectile, block or engine parity; full mob passes remain paused.
- Background run 5 passed **491 engine tests**, with the same two separately
  passing socket tests excluded. Final review also corrected thrown-projectile
  construction to update its bounding box/block/chunk coordinates, and kept the
  swept search expansion on the original velocity to avoid subtraction rounding.
- Final background run 6 passed **491 engine tests** after the construction/search
  refinements, with those same two socket exclusions. No live client/server
  comparison was performed.

## Projectile owner collision and target eligibility

- Replaced the five-tick owner exemption and three entity-type exclusion lists
  with Projectile's shared owner-group rule. The complete root-vehicle/passenger
  tree is protected until no pickable member intersects the projectile's swept
  box plus one block. Leaving that range latches permanently; duplicate checks
  in the same tick are suppressed. Removed owners resolve as absent.
- Added shared pickability and projectile-hit predicates. Dead living entities,
  spectators, marker armor stands and Interaction targets no longer absorb these
  projectiles; falling blocks/TNT, pickable decorations, vehicles, shulker bullets
  and tagged redirectable projectiles use their source eligibility rules.
  Removed the obsolete collides_with_projectiles flag from thrown constructors.
- Arrows/tridents check owner range before their movement query; the shared world
  lifecycle checks remaining projectile paths after their tick. Arrow pierced-ID
  exclusion remains local. This does not fix each projectile's complete tick order.
- Entity NBT now saves the true LeftOwner latch and restores it with the transient
  check guard reset. This does **not** restore the owner's identity: current
  projectile owner fields still use runtime IDs. UUID owner persistence and
  cross-dimension owner lookup remain required, along with PvP team rules,
  deflection callbacks, dragon-part targeting, arrow many-hit/piercing queries,
  and the separate fishing-hook/shulker-bullet query implementations.
- Actual Java Projectile.checkLeftOwner/canHitEntity fixtures cover **1,200 cases**
  of repeated checks, tick/reset boundaries, owner absence, pickable passenger
  boxes, permanent exit and same/different vehicle groups. Rust persistence checks
  cover absent/true LeftOwner data and resetting the transient guard. Background
  run 2 passed **493 engine tests**, with the same two socket-test exclusions.
  Source inspection supports target-type dispatch; no full live-world hit test or
  full mob pass was performed.
- Final background run 3 passed **493 engine tests** after excluding removed
  owners from resolution, with the same two socket-test exclusions.

## Projectile owner UUID references

- Removed per-projectile runtime owner-ID storage and duplicate getter overrides.
  Constructors now capture UUID identity in the shared entity owner reference.
  Owner NBT uses Java's four-int Owner UUID representation, including unresolved
  owners. Loading resets the cached handle without dropping that identity.
- Added current-world-first lookup across server worlds, including players. Live
  cached owners remain usable through world moves; removed/expired cached handles
  trigger UUID resolution again. Runtime IDs are derived from the resolved entity.
  Weak cached handles avoid creating owner/projectile reference cycles.
- Owner collision, permissions, damage attribution, pearl teleport lookup, egg and
  firework callbacks, sculk vibration/shrieker attribution, TNT ignition, bells,
  target statistics and explosion player attribution use resolved owner objects.
  Shoot events retain the owner object as context across dimension boundaries.
- Java spawn packets now carry the resolved projectile owner ID; ownerless fishing
  hooks use their own ID, while other ownerless projectiles use zero. This updates
  the common sender; it does not certify older-client object-data conversions.
- Arrow/trident/spit/hook constructors now update position, bounding box and cached
  block/chunk coordinates together when moving to their owner-relative spawn point.
- Actual Java EntityReference fixtures cover **600 stateful cache/lookup cases**,
  removed and replacement owners, a wrong UUID, and exact UUID codec integers.
  Rust checks also cover saving an unresolved owner, resolution after a runtime-ID
  change, missing/malformed Owner data and clearing the transient cache on load.
  Background run 3 passed **495 engine tests** with the same two socket exclusions,
  before the final spawn-packet owner-data adjustment.
- Remaining linked-engine work: primed TNT and evoker-fang owner persistence,
  complete owner-changing deflection/restore callbacks, firework attachment identity,
  older-client/Bedrock ownership packets, and live cross-dimension save/reload tests.
  Java keeps a strong cached object; our weak cache relies on live world/entity
  handles retaining non-removed entities. Unregistered owner construction/lifetime
  remains an integration boundary. Full mob passes remain paused.
- Final background run 4 passed **495 engine + 114 protocol tests** after the
  spawn-packet adjustment, with the same two socket-test exclusions. No live
  cross-dimension server/client test was performed.
