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
