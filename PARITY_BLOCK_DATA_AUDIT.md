# Block data audit — local vanilla 26.2

This is data inventory evidence, not full behavior or generated-shape parity.

- Assets: 1196 unique block names, 32366 unique state IDs, 683 shape entries.
- Defaults and shape references: 0 structural issues.
- Blocks.java public block fields: 852 fields checked. Two field aliases (POTTED_AZALEA and POTTED_FLOWERING_AZALEA) explicitly register the existing potted_azalea_bush and potted_flowering_azalea_bush IDs; all 852 are present after resolving those aliases. Constructor and runtime handler behavior are not validated by this check.
- tags/block: 265 vanilla JSON files, 0 missing, 0 differing (parsed JSON comparison).
- loot_table/blocks: 1113 vanilla JSON files, 0 missing, 0 differing (parsed JSON comparison).

## Still open

- Exhaustive constructor/inheritance and runtime handler routing for every registry ID.
- Property values/defaults and contextual collision/occlusion shapes against an independent vanilla dump.
- Loot execution, predicates and random sequences; matching declarations do not establish matching execution.
- The 326 Java source checklist and D01–D06 remain the explicit coverage backlog.
