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
