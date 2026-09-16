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
2. **D02: loot engine.** Full function/predicate/component contexts and reloadable
   tables. Named random streams are implemented in the checkpoint below. Vault now uses the existing loot
   evaluator; unsupported evaluator behavior is not fixed by its new lifecycle.
3. **D03: world lifecycle.** Exact neighbor/scheduled update ordering at chunk
   boundaries, experimental redstone orientation and block-entity integration.
4. **D04: entity foundations.** Per-entity Java random streams, dynamic environment
   attributes, remaining linked-entity ownership/lifecycles, piglin anger and entity-specific
   random draw ordering. This work precedes full per-mob passes.
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


## TNT identity, explosions and saved motion

- Primed TNT saves/restores Java's signed-short fuse, displayed block state,
  clamped explosion power and lowercase `owner` UUID. Unknown block names fall
  back to default TNT; malformed properties retain the block's valid defaults.
  Owner lookup uses the shared UUID resolver across worlds and accepts living
  entities only. Chain reactions propagate the actual owner rather than a boolean.
- Explosions now keep direct source and indirect living owner separately for
  damage attribution, damage type, block rewards and chained TNT. This does not
  implement TNT-minecart ignition DamageSource snapshots.
- Launch happens at priming, using the world Java random stream, rather than in
  metadata initialization. Chain-reaction launch consumes its random draw before
  the shortened-fuse draw. Loading/summoning no longer adds a random impulse.
- TNT stores gravity-adjusted velocity before movement, applies float air drag
  before grounded bounce, decrements/synchronizes the signed fuse, and explodes at
  Java's height offset. Teleport callbacks select the portal-preserving explosion
  calculator; its transient flag is not serialized, matching Java.
- Removed the shared chunk loader's unconditional velocity reset. Saved motion
  uses numeric NBT coercion, partial Vec3 codec results and Java's per-axis ten-block
  limit. Missing/invalid vectors reset motion rather than retaining an old value.
- Evidence: actual PrimedTnt load/save oracle covers **612 cases**; actual
  ValueInput/Vec3 codecs with Entity.load's source limit cover **240 motion cases**.
  Final background run 3 passed **497 engine tests**, excluding the same two
  previously separately passing localhost socket tests. No live gameplay run.
- Remaining: live TNT chain/reload tests, non-player cross-world transfer and
  portal transition/passenger integration, TNT-minecart source/RNG/physics work,
  other linked-entity identity and projectile lifecycle gaps. The current generic
  non-player teleport changes coordinates but does not transfer world membership;
  the portal calculator alone does not close that engine gap. No full mob pass.


## World transfers, passenger transitions and portal motion

- Non-player transitions now recreate the entity in the destination world from
  saved NBT, preserve its UUID, allocate a new runtime ID and remove the source
  with ChangedDimension. Both worlds' live lists, trackers and spawn accounting
  are updated. Removed entities in an already-collected tick list are skipped.
  Ordinary removal no longer overwrites an earlier removal reason.
- Transfers walk every passenger level, retain position/yaw/pitch offsets, detach
  cross-world riders before transfer and remount successful arrivals on the new
  vehicle. Player transfers await the existing world/client path before remounting.
  Concurrent requests for the same entity are guarded; entities pause ticking
  while their transfer awaits I/O. Loaded requests complete synchronously.
- Portal cooldown/processor and projectile/TNT cached owner references survive
  replacement. Removed owner handles resolve to the new entity by UUID. TNT's
  post-transition callback retains its portal-resistant blast behavior. This does
  not add missing Vex owner or ItemEntity reference-cache implementations.
- Added PositionMoveRotation relative-position, rotation, velocity and passenger
  arithmetic. Nether axis changes use +90 degrees in both directions and rotate
  motion using Java's float lookup-table operations. End entry retains/rotates
  motion with its source flags. Portal player packets now carry the resulting
  movement; post-transition portal sound targets the arriving player.
- Block shape pushes use a separate synchronous position-only path, retaining
  riding relationships without triggering TNT's dimension-transition flag.
  Portal contact timing now compares the old count before incrementing it.
- Portal arrivals install/refresh a 300-tick destination ticket, load at radius 3
  and activate the central 3x3 entity-ticking footprint without nearby players.
  Expiration removes the loading ticket and active footprint; overlapping arrivals
  refresh/union correctly.
- Evidence: **768 actual Java PositionMoveRotation/passenger-transition cases**
  compare exact floating-point bits. A real two-World integration test transfers
  TNT, a falling-block rider and a nested arrow, checking source removal, new IDs,
  stable UUIDs, saved fuse/power, rotated velocity, mounts, owner re-resolution,
  same-world identity and immediate block-push/request behavior. Final background
  run 5 passed **501 engine tests**, with the two previously separately passing
  localhost socket tests excluded. No live client connection was used.
- Still open: persistent portal tickets and distinct block-only ticking outside
  the central 3x3; portal search/generation scheduling relative to synchronous
  Java ticks; complete End return/respawn/credits rules; spectator/camera transfer,
  player tracker/streaming/cancellation ordering, rider client acknowledgments and
  older Java/Bedrock live packet verification. Shared serialization limitations
  still affect entity replacement where a type's own saved payload is incomplete.
  This closes the missing basic world transfer, not all portal or engine parity.

## Persistent tickets, activation ranges and queued world work

- Portal tickets and ordinary `/forceload` tickets now save/load each dimension's
  canonical `data/minecraft/chunk_tickets.dat`. Portal refreshes merge by chunk
  and level; loading restores the remaining lifetime. Autosave runs after releasing
  the world-time lock. Unknown/malformed entries are retained for round-trip safety.
- Entity and block activation use separate trackers: portal level 30 activates
  3×3 entity chunks and 5×5 block chunks; forced level 31 activates the center for
  entities and a 3×3 block area. Player block ticking extends one ring beyond
  entity ticking, with Java's level-zero clamp. Block entities use the block area.
- Scheduled block/fluid work only drains eligible block chunks. Inactive loaded
  queues age and retain overdue ticks. Collection merges each chunk's earliest
  trigger head by priority/sequence, and preserves work beyond the separate
  65,536 block/fluid budgets. Fluids are collected after block callbacks and their
  current type is checked before delivery.
- Block events deduplicate while queued, retain their original block type, permit
  reentrant scheduling after removal, and defer inactive-chunk events until a later
  drain. Events for a replacement block are discarded. The existing one-million
  event emergency cap and chunk-recipient broadcast remain Pumpkin differences.
- Entity cleanup now respects full-chunk loading tickets in addition to player
  watchers, and rechecks retention before asynchronous entity removal. This keeps
  portal/forced-ticket residents from being unloaded merely because viewers leave.
- Evidence: 128 actual Java LevelTicks traces (48 ticks, four chunks, 25 varying
  activation/budget steps each); 182 actual TicketStorage codec/activation cases;
  real World integration for ticket load, separate rings, deferred/deduplicated
  events, scheduled eligibility, ticket-aware storage retention and periodic save.
- Final background run 5: **504 engine tests and 227 world tests passed**, with
  the same two previously separately passing localhost socket tests excluded.
  No live client/contraption comparison or full mob pass was performed.
- Remaining D03: entity storage loading/activation is still tied to the player
  chunk receiver; ticket-only startup must load and activate saved residents before
  scheduled callbacks, matching Java's entity-loaded/readiness gate. Portal expiry
  still lacks Java's pause while a chunk holder is not ready for saving. Nonstandard
  saved forced-ticket levels, global tick time/reload delays, equal restored order
  ties across chunks, live neighbor/redstone ordering and client event recipients
  remain unverified. These are shared engine gates; full mob passes remain paused.

## Ticket-only entity loading and activation

- Every newly active block area and completed full-chunk load requests entity
  storage through the world lifecycle. Player arrival uses the same path; players
  no longer own saved-entity activation or append untracked entities themselves.
  Completed reads are consumed by world ticking before marking the chunk ready.
- Scheduled block/fluid callbacks and ordinary entity ticks require activated
  entity storage. Readiness follows the identity of the loaded storage instance,
  so removing/replacing a cached chunk cannot leave an old ready flag effective.
  Unretained or stale load completions leave their serialized data untouched.
- Concurrent reads and empty-chunk creation share one storage result per chunk.
  Request cancellation cannot replace a shared result, and cached receiver delivery
  awaits capacity instead of dropping entries beyond its 64-slot channel. Shutdown
  still cancels stalled receivers. Read failures stay unready and are logged;
  corrupt storage is not inserted into the cache as an empty successful load.
- Saved residents register through the entity tracker/spawn accounting path,
  preserving UUIDs and motion. Recursive Passengers load and mount before chunk
  readiness is published; projectile owners resolve through the loaded UUIDs.
  Saves write vehicle roots with nested Passengers, skip standalone rider entries
  and replace an existing snapshot of the same root instead of appending a copy.
- Evidence: a disk-backed restart with only a persisted forced ticket loads TNT,
  a falling-block rider and a nested arrow without players, verifies pending-tick
  gating, identity, mounts, owner resolution, velocity/fuse and repeated snapshots.
  A storage concurrency check verifies 64 requests share the same instance and a
  cached receiver delivers all 130 requests. Final background run 3 passed **505
  engine and 228 world tests**, excluding the same two previously separately passing
  localhost socket tests. No full mob pass or live client session was performed.
- Still open D03/D04: complete snapshots must remove old positions/deleted roots,
  periodic entity autosave must use those snapshots, and unloading must save/remove
  a whole passenger tree together across chunk boundaries with correct removal
  reasons. The existing asynchronous unload/save race and plugin-cancelled mounts
  need integration work. Player RootVehicle still saves only Attach, not Java's
  embedded Entity payload. Exact chunk-holder readiness/portal expiry pausing,
  global scheduled tick time, client pairing order and broader engine/block gates
  remain open. Ticket-only startup loading is implemented, not full lifecycle parity.

## Complete entity snapshots and passenger-tree unloads

- Manual save and shutdown now rebuild chunk entity snapshots from current live
  vehicle roots instead of appending per-entity records. Activated chunks include
  empty snapshots, removing deleted/moved roots. Unactivated storage keeps unrelated
  saved residents; tracked snapshot ownership removes earlier records written by
  this world without erasing residents that have not been made live.
- Periodic autosave captures and writes live entity snapshots as well as blocks.
  Roots whose storage read is still pending request that read and trigger a follow-up
  snapshot when storage becomes available. Snapshotting/unloading runs outside the
  world-time lock, since entity serializers may read the clock.
- Entity writes capture immutable NBT under a submission lock and drain through one
  ordered writer. Later in-memory changes cannot alter a queued image, and an older
  queued save cannot overwrite a newer submitted save. Entity storage stays cached
  until its unload snapshot is successfully written; renewed tickets/replaced cache
  entries cancel removal. Cache removal emits a lifecycle notification so a renewed
  active chunk requests storage again rather than retaining a stale ready flag.
- Unload selects vehicle roots by their chunk and traverses their full passenger
  trees, including riders across chunk boundaries. Selection, snapshotting and live
  removal happen without an intervening I/O await. Removal records UnloadedToChunk
  through the shared tracker/accounting/observer path, then releases strong mount
  links. Passenger-only chunks do not independently remove their riders. Trees with
  players or an in-flight teleport are deferred until their owning lifecycle can
  handle them.
- Evidence: restart checks cover moved/deleted roots, preservation of unactivated
  residents, cross-chunk nested riders, saved passenger trees and removal reasons.
  A fresh storage reader sees periodic autosave before shutdown. Storage tests cover
  immutable ordered writes, read-after-unload persistence and renewed tickets during
  unload. Source basis: PersistentEntitySectionManager.storeChunkSections,
  processChunkUnload and Entity.shouldBeSaved; these tests use actual Pumpkin Worlds
  and storage, not a Java live-server comparison.
- Final background run 5 passed **508 engine tests and 230 world tests**, with
  the same two previously separately passing localhost socket tests excluded.
- Still open: Java's embedded player RootVehicle payload and player-owned tree save/
  unload rules, complete plugin cancellation/transfer/save concurrency, detailed
  entity payloads, exact chunk-holder readiness/portal expiry pausing and scheduled
  world-time restoration. Error reporting from manual saves still needs propagation
  beyond logging. Whole-server live lifecycle/client checks and the other D01–D06
  and block-system gates remain unproven. Full mob passes remain paused.


## Player-owned vehicle persistence and ordered player files

- Player RootVehicle now embeds the complete non-player vehicle tree and the
  immediate attachment UUID. Exactly-one-player trees belong in the player file;
  roots with zero or multiple player passengers remain eligible for chunk storage.
  Disconnect saves before dismount/removal, uses UnloadedWithPlayer for the owned
  tree, releases mount links and increments the leave statistic once.
- Join restores the embedded tree after initial player placement, resolves Attach
  only within that newly loaded tree, and discards an unattached new tree. Duplicate
  UUIDs never cause an existing unrelated entity to be discarded. Old Pumpkin
  Attach-only files retain their compatibility path.
- Player saves reserve a per-player generation before capturing NBT. Older queued
  snapshots cannot overwrite a newer submitted save. Compression writes a temporary
  file before atomic replacement. This does not yet provide Java's dat_old recovery
  or serialize every login/plugin/transfer/snapshot lifecycle against disconnect.
- Evidence: two real World helper tests cover nested payloads, one/multiple-player
  ownership, chunk exclusion, restore, conflicts and removal reasons. Player-type
  markers are used without network clients; actual login packet ordering remains
  unverified. A storage test checks stale snapshot rejection and independent player
  writes. Final background run 3: **510 engine and 231 world tests passed**, with
  the same two previously separately passing localhost socket tests excluded.
- Source basis: ServerPlayer.saveParentVehicle/loadAndSpawnParentVehicle,
  PrepareSpawnTask, PlayerList.remove and Entity.shouldBeSaved in local Java 26.2.
  Shared clock/scheduled-time restoration, chunk-holder readiness, broader payloads,
  plugin cancellation/concurrency and the remaining D01–D06/block gates remain open.
  Full mob passes remain paused.


## World clocks and saved scheduled work

- World loading restores level.dat Time and the overworld clock rather than
  starting both at zero. Clock persistence includes total_ticks, partial_tick,
  rate and paused; DataVersion is written at the saved-data root, with compatibility
  for Pumpkin's older nested version field. Loaded custom clock entries retain
  these fields. Manual/server saves, shutdown and overworld autosave capture the
  current clock state; portal ticket expiry starts from the restored game time.
- Block/fluid queues bind to the level's absolute game clock when full chunks are
  published. Empty queues and inactive queues use the same clock, and block/fluid
  collection observes the same game tick. Restored relative delays are unpacked
  once at publication; time spent unloaded does not consume that stored delay.
- Both chunk-publication paths register existing scheduled work in the level's
  pending-work index. Restored ticks therefore run without needing a fresh schedule
  request in the same chunk. Index removal rechecks current storage so concurrent
  publication/scheduling does not lose a newly queued chunk.
- Saved delays stay signed, preserving the trigger order of overdue work. Chunk
  readers filter ticks outside the containing chunk and clamp saved priorities to
  Java's supported endpoints. Scheduling/draining marks chunks dirty, and pending
  queues keep changing relative-delay snapshots eligible for saving.
- Evidence: 64 unmodified Java LevelChunkTicks pack/unpack/restart traces, including
  negative delays, priority/order, budgets and a later reload time. A storage test
  writes a completed chunk, starts a fresh Level, fetches it through the actual
  chunk pipeline, then checks restored indexing, eligibility, signed NBT delays,
  out-of-chunk filtering, fluid timing and serialization after draining. A real
  World/level.dat restart checks game age, fractional rate, pause/advance-time rules,
  custom clock preservation and saved-data envelope placement.
- Final background run 6: **511 engine and 235 world tests passed**, with the
  same two previously separately passing localhost socket tests excluded. No full
  mob pass or live client session was performed.
- Remaining: one shared server-wide clock manager with dynamic clock definitions,
  exact cross-dimension activation/clock behavior, all malformed numeric codec
  cases, equal restored order ties across chunks, chunk-holder readiness/portal
  expiry pausing, save error propagation and snapshot/plugin concurrency. The
  broader D01–D06 and individual block-system integration gates remain open.


## Save completion, errors and write retries

- Region writes now flush before reporting success even while a player watches
  the region. Serialization and file writes share an exclusive region lock.
  Dirty state is claimed atomically; write/serialization errors mark affected chunks
  dirty again so an unsuccessful save cannot silently remove retry eligibility.
- Manual chunk saves submit through the existing scheduler/writer FIFO and await a
  completion result. Empty requests still wait behind prior jobs and retry pending
  failures. The writer retains failed chunk payloads even after their live holder
  disappears; newer submissions replace older failed payloads at the same position.
  Save requests reject shutdown rather than treating a closed writer as success.
- Entity writes return their actual errors through snapshot saving. World.save
  collects ticket, border, entity, POI, custom-data and chunk errors while attempting
  the remaining work. Server.save_all collects failures across worlds; the plugin
  API returns World.save's result. Success feedback in the existing asynchronous
  save command therefore follows completed writes, including watched regions.
- level.dat replacement/backup failures and auxiliary saved-data failures now reach
  the caller. Independent auxiliary files are still attempted when one fails.
- Evidence: a watched region is readable through a fresh file manager before the
  original watcher leaves; a filesystem failure returns an error and leaves dirty
  state, and a later request retries the payload after its holder is removed.
  Entity write errors preserve cached data for retry. A real World save reports
  multiple file errors while still saving blocks. Metadata tests cover auxiliary
  file and level.dat backup/replacement errors. Final background run 4 passed
  **512 engine and 238 world tests**, excluding the same two previously separately
  passing localhost socket tests. No live client session was performed.
- Limits: ordinary save-all and flush still share one async executor, whose command
  result is returned before completion; synchronous command-result parity and
  distinct Java flush behavior remain open. Full crash recovery, fatal scheduler
  failures, read-after-failed-unload coordination, proto-chunk recovery, atomic
  multi-file snapshots and broader plugin/save concurrency are not certified here.
  The other shared engine/block gates remain open; full mob passes remain paused.


## Pending block-save snapshots and failed-load isolation

- Chunk writes capture an independent serialized image before I/O. The image keeps
  block states and relative scheduled-tick delays fixed across retries. Source
  mutations after capture remain separate and dirty; newer save submissions replace
  the older image for that position. Failed serialization retains its source for a
  later capture attempt.
- Pending images are shared with the chunk reader. A load following a failed save
  reads a copy of that image before consulting disk. Its tick queues bind to the
  loading world's clock without changing the retained retry image. Successfully
  written entries are removed only after disk completion.
- Read errors no longer take the missing-chunk path that creates fresh terrain.
  Every requested chunk receives a failure when its region cannot be opened; reads
  retry with a short delay and preserve the requested dependency stage. Missing
  storage still permits generation, including after a previous I/O error is repaired.
- A scheduler exit/unwind closes unaccepted save requests with an error and rejects
  later submissions, preventing save futures from waiting on a stopped scheduler.
- Source basis: local Java IOWorker.store/loadAsync supplies copied pending NBT
  before disk reads and propagates read errors. Retaining failed images for retry
  extends Pumpkin's recovery policy; it does not certify Java failure semantics.
- Evidence: a real failed write followed by the actual chunk-load pipeline sees the
  saved image, ignores later changes to the old source, restores remaining delays
  at a later world time, and accepts a newer replacement snapshot. Batch read-failure
  checks cover all requested positions, failure notifications without new proto
  terrain, and recovery to missing-storage results. An unwind check verifies pending
  save completion closes and later saves reject the stopped scheduler. Final
  background run 4 passed **512 engine and 241 world tests**, excluding the same
  two previously separately passing localhost socket tests. No live client session
  or full mob pass was performed.
- Remaining: full dependent-generation recovery under live load, fatal I/O-worker
  recovery, command result/flush semantics, crash durability and atomic multi-file
  state, plus the other shared engine/block gates. Full mob passes remain paused.


## Named random sequences and continuous loot streams

- Named sequences now use Java's identifier MD5 hash, XOR before Stafford mixing,
  signed salt and world-seed flags. Xoroshiro bounded integers use the unsigned
  **32-bit** rejection threshold. This shared correction also applies outside loot.
- Server startup reads `data/minecraft/random_sequences.dat`. Normal metadata saves
  and shutdown write salt/defaults and both state words in Java's gzip NBT shape.
  Writes replace a temporary file; errors propagate through the metadata save result.
  Invalid existing sequence data aborts startup instead of being silently replaced.
  This is canonical saved-data support, not a full NBT codec/coercion or datafix pass.
- Generated tables retain `random_sequence` and ordered, conditional empty entries.
  Runtime loot chooses an explicit nonzero seed using Java's legacy generator, then
  the named server sequence, then the level generator. Block drops, harvests, vaults,
  trial spawners, brushable blocks, containers, minecart inventories, `/loot`, and
  shared living/vehicle drops use this path. `/random` without a name uses the level
  stream. No individual mob pass was performed.
- Generation and container filling share one source continuously. Entry conditions
  run per roll, zero-weight entries still evaluate their conditions, a single valid
  entry skips the weighted draw, and empty outcomes retain their original ordering.
  Splitting preserves list order and skips Java `Mth.nextInt` degenerate-range draws;
  empty loot still shuffles available slots. Random-source locks are released before
  inventory notifications.
- Java evidence: 96 sequence cases cover seeds, signed salts, all flag combinations,
  large-bound rejection, saved states, resumes and resets. A real Java-written gzip
  file loads and continues in Rust. Another 256 cases exercise actual Java LootPool
  and LootTable raw generation, available-slot shuffle and item splitting with both
  random sources, conditional/zero-weight/empty entries and occupied/full containers.
  The loot probe binds the vanilla stack-size component for its few fixture items;
  it does not start a ServerLevel or test arbitrary item components.
- Verification: final background engine run 3 passed **518 tests**. The preceding
  full run also passed **241 world and 66 utility tests**; their source did not change
  afterward. Both engine runs excluded the same two localhost socket tests that
  passed separately in earlier checkpoints. No live client session was performed.
- Remaining D02: full loot functions, predicates and contexts; nested/composite entry
  execution, luck/quality/bonus rolls, table reloads and component handling. Named
  sequence selection across a live server session and multi-file crash consistency
  remain unverified. Other D01–D06/block gates also remain open; mobs stay paused.


## Structured loot entries and ordered numeric functions

- Replaced the flattened generated loot list with the source entry tree. Nested
  named/inline tables keep their own pools and rolls, execute with the existing
  context/random source, and use a path-scoped recursion guard. Alternatives stop
  on the first successful expansion, including zero-weight/empty outcomes;
  sequences retain earlier candidates when a later child fails. Groups preserve
  Java's empty/one-child/multiple-child expansion behavior. Tags retain expand vs
  emit-all behavior, and dynamic entries consume supplied drops.
- Functions now execute in source order at entry, pool and table levels. Enclosing
  functions run immediately on each nested result before the next nested draw.
  Implemented constant/uniform/binomial providers, quality/luck weights, bonus rolls,
  conditional/additive set-count, count limits, explosion decay and enchantment
  count/bonus formulas. Java 26.2 NumberProvider integer conversion uses Math.round;
  weight and bonus-roll conversion uses floor. Integer counts stay wide through
  functions and split into maximum-sized item stacks at the final output boundary.
- Generic any-of/inverted conditions preserve their evaluation order. Unsupported
  entry conditions now fail instead of becoming unconditional; missing predicate
  implementations still need work. Item/tag tool matching covers the actual
  amethyst harvest tag. Decorated pots supply sherds through the shared dynamic
  entry path; their handwritten alternative branch is removed. `/loot kill` supplies
  attacker and target type facts used by the shared evaluator.
- Source inventory: all **1,356** built-in tables preserve all **2,578** entry nodes;
  every built-in table reference resolves. **1,124** numeric/decay function
  declarations now map to executable function types. The other **292** declarations
  remain explicit unsupported function records, including component/enchantment,
  smelting, map and metadata work. This inventory is not an exhaustive behavior proof.
- Evidence: `LootTreeOracle` uses the real Java 26.2 codec/evaluator on 12 JSON tables.
  The production Rust code generator compiles those exact inputs. **576 cases**
  compare raw items/counts and the next random value across both RNGs, nested
  function order, alternatives/sequences/groups, luck, conditional numeric functions,
  binomial counts, explosion decay, dynamic drops, empty/zero-weight candidates and
  counts above 255. Rust checks cover recursion scope, stack splitting, the actual
  generated decorated-pot table and the amethyst tool tag. Final background run 5:
  **522 engine, 241 world, 66 utility tests passed**, with the same two previously
  separately passing localhost tests excluded. No live client or full mob pass.
- Remaining D02: the 292 other function declarations, full predicates and entity/item/
  block-entity contexts, reloadable tables/tags and end-to-end server comparisons.
  Enchanted-count handling currently uses the supplied attacker/tool facts rather
  than a complete equipment/context model. Tag expansion is source-ported but not
  covered by the new Java fixture matrix. The other D01–D06/block gates remain open.


## Loot damage, potion contents and block-state copying

- Added ordered `set_damage`, `set_potion` and `copy_state` execution: **28 damage,
  48 potion and 10 block-state declarations** now execute through the shared loot
  pipeline. This reduces the remaining unsupported function declarations to **206**.
- Damage uses Java's remaining-durability fraction, additive mode, clamping and
  floor conversion. Empty, unbreakable or component-ineligible items skip both the
  mutation and provider sampling. Explicit zero damage replaces the component.
- Setting a potion preserves existing custom color, effects and name; empty-stack
  updates use the empty component baseline. State copying merges selected values
  while retaining unrelated values, filters properties absent from the declared
  block, and compares actual Java Property identities on the context block. Honey
  levels now use the normal function; the duplicate handwritten hive copy is gone.
- `BlockPropertyIdentityOracle` exports Java Property.equals groups for **1,196
  blocks, 2,060 property assignments and 121 identities**. The generated runtime
  lookup distinguishes same-named properties with different domains, such as wheat
  and sugar-cane age. An exhaustive name check agrees with the current Rust block
  registry; this check does not prove every state transition or property value.
- `LootComponentOracle` supplies 11 JSON tables compiled by the production emitter.
  **704 actual Java cases** compare damage/components and following RNG output with
  both random sources, missing/zero durability components, unbreakable/empty items,
  additive damage, metadata preservation, absent block context, unknown properties,
  shared properties and incompatible same-name properties. Fixture item prototypes
  and initial components are explicitly bound; no ServerLevel or live client is used.
- Final background run 1 passed **524 engine, 241 world and 66 utility tests**, with
  the same two previously separately passing localhost tests excluded. No full mob
  pass was performed.
- Remaining: other component/enchantment/smelting/map/name functions and full loot
  contexts, predicates and reloads, plus the other D01–D06/block gates. The existing
  block-entity copy-components fallback is still broader than a complete loot-function
  implementation and remains part of D02. Full mob passes remain paused.

## Block-entity component sources and copy-components loot

- All **71** built-in `copy_components` declarations now execute in the ordered
  loot pipeline. Include absent versus empty, exclusion precedence, missing sources,
  present values versus removals, and the tool's effective prototype/patch view are
  preserved. **135** other function declarations remain unsupported.
- Ordinary breaking and `/loot mine` collect the same block-entity component source
  before borrowing the random stream; the post-loot same-block copying fallback is
  removed. Decorated-pot dynamic sherds also use that shared context helper.
- Shared retained-component storage now supports banners, skulls, chests, trapped
  chests, barrels, shulker boxes, hives, decorated pots and enchanting tables. It
  persists added patch entries not consumed by the entity; prototype defaults,
  removals, block-state and block-entity-data components are not retained. Collection
  merges stored values with current entity fields before applying the loot whitelist.
- Banners now restore/drop patterns and styled custom names; skulls restore/drop
  profile, note-block sound and styled names. Supported container sources expose
  contents, including an explicit empty container, independently of which values
  their built-in loot tables request. Pot pending loot remains separate from its
  retained container-loot component, matching its Java implicit-component methods.
- `LootCopyComponentsOracle` runs the actual Java 26.2 codec/evaluator on nine tables
  and exports **288 cases**, including both random sources and following random
  values. Rust uses production-generated versions of those tables. The block source
  comparison passes through chest placement, NBT save/reload and component collection.
  Additional tests cover nine entity storage round trips, styled names, banner/skull
  values, base exclusions, pot pending loot and real chest/banner loot whitelists.
- Final background run 6 passed **530 engine, 241 world and 66 utility tests**,
  with the same two previously separately passing localhost tests excluded. No live
  client or full mob pass.
- Remaining D02/D05: component storage/implicit fields for the other block-entity
  families, full container locks and text/component codecs, other entity component
  sources, 135 function declarations, full predicates, reloads and live integration.
  Other D01–D06/block gates remain open; this is a bounded checkpoint, not a claim
  that only mobs remain.

## Smelting, stew and ominous-bottle loot transforms

- Added shared `furnace_smelt`, `set_stew_effect` and
  `set_ominous_bottle_amplifier` execution for **25 declarations** (19/3/3).
  **110 function declarations remain unsupported**.
- Smelting assembles the matching furnace recipe, replaces input components,
  multiplies the result count when requested and caps it at the result's maximum
  stack size. Empty inputs and missing recipes preserve their original result.
  This uses the current built-in recipe registry; reloadable/custom recipe results
  and their component payloads remain part of the recipe/reload gap.
- Stew selection preserves draw order, appends effects, converts non-instantaneous
  durations to ticks and preserves Java integer overflow. Empty/non-stew inputs and
  empty choices skip sampling. Bottle amplifiers sample and clamp to 0–4, including
  mutations made while a stack is temporarily empty.
- Built-in smelting predicates now read the target's actual fire state and the
  direct attacker's main-hand enchantments. Context snapshots use fire immunity and
  player/living equipment; an indirect attacker's sword is not substituted for a
  projectile's equipment. Java 26.2 `attacker`/`direct_attacker` selectors are parsed
  explicitly. All supplied flag constraints are preserved; unsupported flag/slot/
  level constraints still fail closed. Explicit empty entity predicates require an
  entity; empty enchantment tests still require the equipment component.
- `LootTransformOracle` executes actual Java functions with the actual recipe
  manager loaded from all **73** built-in smelting recipes, including their three
  ingredient tags. Its level shim supplies only recipe access. **3,265 cases** cover
  all **1,537 items**, all **40 effects**, 54 production-generated tables, empty and
  oversized counts, metadata replacement/preservation, ordering and following RNG
  values. Compared item-component defaults are explicitly bound from the canonical
  item export. Separate Rust checks exercise the real built-in smelting condition
  tree and missing entity/equipment/component boundaries. This is shared engine
  coverage, not a full pass over any mob.
- Final background run 3 passed **533 engine, 241 world and 66 utility tests**,
  with the same two previously separately passing localhost tests excluded. Other
  D01–D06/block gates, full loot contexts/predicates, recipe reloads and live
  integration remain open.
