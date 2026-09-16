# Local Java parity probes

`InsideTraversalOracle.java` calls `BlockGetter.forEachBlockIntersectedBetween`
directly in the unmodified Java 26.2 server. It records visited cells and iteration
numbers for 120 cases, including all combinations of axis signs/zero components,
axis ties, negative coordinates, wide boxes and tiny displacements.

From the `pumpkin` repository, using the already downloaded Java 26.2 classpath
and JDK 26:

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/InsideTraversalOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' InsideTraversalOracle > crates/pumpkin/src/entity/inside_blocks_cases.json
```

The `from`, `to`, `min` and `max` arrays store unsigned IEEE-754 binary64 bits,
not decimal coordinates. This avoids JSON float parsing changing an axis tie by
one bit. The Rust test reconstructs doubles with `f64::from_bits` and compares
both the complete visit order and step numbering, without sorting.

This oracle verifies the traversal algorithm, not full entity effects, movement
packet validation, fluid lifecycle or gameplay parity.

## Ordered inside effects

`InsideEffectsOracle.java` invokes the real Java 26.2 `StepBasedCollector`
with 100 seeded sequences of 80 operations, recording primary effects and before/after
callbacks. The minimal Entity probe bypasses its constructor and records effects;
it does not test health, damage, fluid contacts or a running world.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/InsideEffectsOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' InsideEffectsOracle > crates/pumpkin/src/entity/inside_effects_cases.json
```

The Rust fixture compares exact output order, including repeated effects, callbacks
without primaries, step changes and repeated apply/clear calls.

## Collision grids, step-up and border extents

`CollisionStepCoordinates.java` exports the full X/Y/Z coordinate grids for all
32,366 Java 26.2 block states at `BlockPos.ZERO` with the empty collision context.
`generate_step_coordinates.py` deduplicates those grids into a Rust data table.
Flattening a shape into AABBs loses internal coordinates used by both overlapping
collision clipping and step candidate selection.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/CollisionStepCoordinates.java
java -cp '/tmp:../comparison/downloads/classpath/*' CollisionStepCoordinates > /tmp/step-coords.json
python3 tools/vanilla/generate_step_coordinates.py /tmp/step-coords.json
```

`StepCollisionOracle.java` invokes Java's real private shape clipper and step-height
collector for 400 fixed-seed box scenes. Its driver follows Entity.collide's branch
selection; it does not instantiate a world or test collision gathering.
`StaticCollisionOracle.java` invokes the real clipper for every static block state
with 81 fixed motions/starting boxes. Each state's expected result is an unsigned
FNV-1a-style fingerprint over the exact output double bits (zero signs normalized).
The Rust test rebuilds the same inputs and fingerprints, without sorting results.
`WorldBorderOracle.java` exercises the actual border's size, bounds and remaining
duration over 180 set/lerp/retarget/center/limit/tick operations.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/StepCollisionOracle.java tools/vanilla/StaticCollisionOracle.java tools/vanilla/WorldBorderOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' StepCollisionOracle > crates/pumpkin/src/entity/ai/control/step_collision_cases.json
java -cp '/tmp:../comparison/downloads/classpath/*' StaticCollisionOracle > crates/pumpkin/src/entity/ai/control/static_collision_hashes.json
java -cp '/tmp:../comparison/downloads/classpath/*' WorldBorderOracle > crates/pumpkin/src/world/border_cases.json
```

These probes verify bounded algorithms and static contexts. Dynamic/contextual
shape construction, moving piston unions, server packet validation and gameplay
must still be verified separately. The border's change to tick units is documented
in [Mojang's 1.21.11 release notes](https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-11).


## Dynamic piston collision union

`PistonCollisionOracle.java` constructs real Java 26.2 moving-piston block entities,
sets progress and the direction-specific NOCLIP context, and fingerprints their
complete X/Y/Z grids and ordered optimized boxes. It covers 100,656 combinations
across every state of ten representative blocks, six directions, both extension
and source flags, nine progress values, and both suppression modes. Progress values
include adjacent floats around shape boundaries, not only normal half-tick steps.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/PistonCollisionOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' PistonCollisionOracle > crates/pumpkin/src/block/entities/piston_collision_cases.json
```

Each fixture row contains state ID, direction, extending, source, float progress,
NOCLIP enabled and an unsigned FNV-style fingerprint of binary64 values (zero signs
normalized). Counts precede each axis and the box sequence. Rust invokes its real
piston shape constructor and compares without sorting boxes. These cases prove
bounded shape construction; they do not certify entity displacement, packet
handling or a running piston contraption.


## Collision restitution

`CollisionRestitutionOracle.java` invokes Java 26.2 Entity's actual private
`restituteMovementAfterCollisions` method on a non-living probe with controlled
velocity, gravity, drag and bounciness. 800 seeded cases cover collision axes,
slime/bed/honey/ordinary blocks, suppression and gravity/drag compensation.
Bootstrap does not load datapack tags, so the probe explicitly binds honey to
`suppresses_bounce`, matching the vanilla JSON's sole entry. It overrides the bounce
event sink rather than loading a world. Output vectors use raw binary64 bits;
Rust compares them exactly except NaN payloads, for which it checks NaN semantics.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/CollisionRestitutionOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' CollisionRestitutionOracle > crates/pumpkin/src/entity/ai/control/collision_restitution_cases.json
```

This verifies restitution math, not movement authority, fall damage, vehicle
control or full world dispatch. Those integration gates remain in ENGINE_GAPS.md.


## Block rays

`BlockRayOracle.java` calls Java 26.2 `BlockGetter.traverseBlocks` and
`VoxelShape.clip` for 600 fixed-seed rays, including zero-length, axis-aligned,
very short and exact block-boundary rays. Shapes include full blocks and water-like
heights. Fixtures retain input double bits, the exact visited-cell sequence and
whether the shape was hit. This proves the shared ray algorithm, not live world
selection of resetting blocks, fluid state or portal gamerules.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/BlockRayOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' BlockRayOracle > crates/pumpkin/src/world/block_ray_cases.json
```


## Fall-distance accumulation

`FallDistanceOracle.java` invokes Java 26.2 Entity's actual `checkFallDamage` method
on a probe with controlled water state. It records 1,200 updates, including small
movements added to large counters. The comparison uses exact binary64 bits and
checks Java's float-cast movement feeding a double accumulator. Landing is disabled
in this probe so it needs no world; landing callbacks, particles and passenger
propagation require separate integration verification.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/FallDistanceOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' FallDistanceOracle > crates/pumpkin/src/entity/fall_distance_cases.json
```

## Projectile rays and border deflection

`ProjectileRayOracle.java` invokes Java 26.2 ProjectileUtil's nearest-entity query
and age margin on controlled candidate boxes (1,200 cases). It covers strict ties,
inside starts, exact endpoints, epsilon edges and tick-count overflow. The test
isolates geometry/selection; it does not validate world query ordering, pickability,
owner immunity or AbstractArrow's distinct many-hit path.

`BorderRayOracle.java` invokes CollisionGetter.clipIncludingBorder with a controlled
ordinary ray result (500 cases). `BorderDeflectionOracle.java` invokes
Projectile.hitTargetOrDeflectSelf on actual Arrow instances for 200 border hits,
recording velocity, yaw, sync requirement and RNG state. Unsafe initialization
replaces only unrelated world/entity construction; the invoked methods are vanilla.
Previous-rotation storage and full tick integration are separate engine gaps.

These probes use the existing BlockClipOracle, FluidInteractionOracle and
FallDistanceOracle helpers compiled into `/tmp`.

```sh
javac -cp '../comparison/downloads/classpath/*:/tmp' -d /tmp tools/vanilla/ProjectileRayOracle.java tools/vanilla/BorderRayOracle.java tools/vanilla/BorderDeflectionOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' ProjectileRayOracle > crates/pumpkin/src/entity/projectile/ray_cases.json
java -cp '/tmp:../comparison/downloads/classpath/*' BorderRayOracle > crates/pumpkin/src/world/border_ray_cases.json
java -cp '/tmp:../comparison/downloads/classpath/*' BorderDeflectionOracle > crates/pumpkin/src/entity/projectile/border_deflection_cases.json
```

## Projectile owner collision

`ProjectileOwnerOracle.java` invokes actual Projectile.checkLeftOwner and
canHitEntity for 1,200 stateful cases. The probe supplies an owner/root/passenger
tree, boxes and pickability; Java performs swept-range intersection, the exit latch,
once-per-tick suppression and vehicle-group immunity. Query counters verify that
already-checked/latched states do not repeat the owner-tree scan. This does not
validate live spatial membership, owner UUID persistence or every entity's own
pickability override.

```sh
javac -cp '../comparison/downloads/classpath/*:/tmp' -d /tmp tools/vanilla/ProjectileOwnerOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' ProjectileOwnerOracle > crates/pumpkin/src/entity/projectile/owner_cases.json
```

## Entity owner references

`EntityReferenceOracle.java` uses the real generic Java EntityReference with simple
UniquelyIdentifyable targets. Its 600 stateful cases verify UUID- and object-created
references, cache hits without lookups, removed cached targets, rejected removed or
wrong-UUID lookup results, replacements, and UUID codec integers. Rust uses the same
resolver with weak live-entity handles; the fixture tests resolution semantics, not
world registration lifetime or network delivery across dimensions.

```sh
javac -cp '../comparison/downloads/classpath/*:/tmp' -d /tmp tools/vanilla/EntityReferenceOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' EntityReferenceOracle > crates/pumpkin/src/entity/entity_reference_cases.json
```

## Chunk tickets and scheduled container ordering

`ChunkTickOracle.java` runs unmodified Java 26.2 `LevelTicks` for 128 seeded cases.
Each schedules 48 ticks across four chunks, then changes activation masks and
budgets over 25 ticks. Rust compares exact delivery order, including overdue work.
`ChunkTicketOracle.java` exercises `TicketStorage.CODEC` and activation, checking
182 numeric NBT/coercion and duplicate-refresh cases for canonical portal/forced
levels. Neither probe models a running chunk holder or connected client.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/ChunkTickOracle.java tools/vanilla/ChunkTicketOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' ChunkTickOracle > crates/pumpkin-world/src/tick/chunk_tick_cases.json
java -cp '/tmp:../comparison/downloads/classpath/*' ChunkTicketOracle > crates/pumpkin/src/world/portal/chunk_ticket_cases.json
```


## Named sequences and loot draw order

`RandomSequenceOracle.java` invokes unmodified Java 26.2 RandomSequences, its CODEC
and Xoroshiro source for 96 seed/salt/flag combinations. It writes the JSON vectors
and `random_sequence_java.dat`, a genuine Java gzip saved-data fixture.
`LootRandomOracle.java` runs actual LootPool/LootTable raw generation, available-slot
shuffle and splitting for 256 cases with legacy and Xoroshiro sources. Reflection
constructs a LootContext without a ServerLevel and accesses the private slot/split
methods. Its fixture items bind only their vanilla MAX_STACK_SIZE=64 component;
this probe deliberately does not claim server or full component integration.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/RandomSequenceOracle.java tools/vanilla/LootRandomOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' RandomSequenceOracle crates/pumpkin/src/world/random_sequence_cases.json
java -cp '/tmp:../comparison/downloads/classpath/*' LootRandomOracle crates/pumpkin/src/world/loot_random_cases.json
```


## Structured loot tables and ordered functions

`LootTreeOracle.java` writes 12 table JSON inputs plus 576 raw-output/random-state
cases using the actual Java 26.2 LootTable codec and evaluator. The production
`pumpkin-codegen` loot emitter compiles the same JSON into Rust test tables, so the
comparison exercises source parsing as well as evaluation. Fixtures bind the
stack-size component of their few item types and construct the minimal loot context;
this is not a running ServerLevel, full component or reload test. Named-reference
resolution, recursion guards and tag behavior have separate source/Rust checks.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/LootTreeOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' LootTreeOracle crates/pumpkin/src/world/loot_tree_tables.json crates/pumpkin/src/world/loot_tree_cases.json
cargo run --offline --manifest-path tools/pumpkin-codegen/Cargo.toml -- loot_table loot_tree_test_tables
```


## Loot components and Java block property identity

`BlockPropertyIdentityOracle.java` groups every registered block property using the
actual Java Property.equals implementations, ordered deterministically by block and
property name. Its asset drives copy-state compatibility in the generated evaluator.
`LootComponentOracle.java` writes 11 JSON inputs and 704 cases for damage, potion and
copy-state functions, including following RNG values. It binds explicit fixture item
prototypes/components and uses a minimal context, without starting a ServerLevel.
The same production code generator compiles the JSON inputs into Rust test tables.

```sh
javac -cp '../comparison/downloads/classpath/*' -d /tmp tools/vanilla/BlockPropertyIdentityOracle.java tools/vanilla/LootComponentOracle.java
java -cp '/tmp:../comparison/downloads/classpath/*' BlockPropertyIdentityOracle assets/block_property_ids.json
java -cp '/tmp:../comparison/downloads/classpath/*' LootComponentOracle crates/pumpkin/src/world/loot_component_tables.json crates/pumpkin/src/world/loot_component_cases.json
cargo run --offline --manifest-path tools/pumpkin-codegen/Cargo.toml -- loot_table loot_component_test_tables
```
