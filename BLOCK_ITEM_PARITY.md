# Block and item parity checkpoint — 2026-09-15

## Status and constraints

**The requested 1:1 parity for all blocks and then all items is not complete.**
This checkpoint records source changes, not a passing parity result.

- Work branch: `codex/vanilla-spawning`, based on `1e01074e`.
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

Conversely, a registered catalyst still lacks its spreading behavior. Do not turn
scanner results into “100% parity.” The earlier `progress.py` also hardcodes the
item count at 40/51 and uses disposable agent branch names as review evidence.
Those branch names were removed during the authorized merged-worktree cleanup,
so they must not be used to reconstruct audit completion.

## Remaining work, in the user's block-then-item order

1. **Finish the block audit and implementations.** The major confirmed gap is
   sculk spreading: catalyst death-event delivery by distance and XP consumption,
   `SculkSpreader` charge cursors/persistence, sculk substrate conversion and growth,
   and the specialized vein spreader are absent. The existing catalyst is only
   placement plus a minimal block entity; do not substitute a periodic random
   spread rule for the vanilla system.
2. Finish sculk's upstream event coverage. Many vanilla emitters are still absent
   (for example container and projectile actions). Movement coverage here is for
   living entities; nonliving movement, flapping, and full movement sound/effect
   ordering still need review. Shrieker attribution also needs the actual
   controlling-passenger and dropped-item-owner semantics. Legacy particle
   protocol variants and Bedrock vibration particles have not been audited.
3. Audit the remaining block families against their actual vanilla implementations,
   including support/shape behavior and data-driven drops. Examples of visible
   existing gaps are stored-bee release and the full wall-shape rules. Copper chest
   scraping should also reproduce the connected half's secondary particle/event
   effects; state/inventory synchronization alone is not the whole item behavior.
4. **Then finish every item class and shared component path.** The earlier 40/51
   claim has no reliable per-class completion ledger. Specific known gaps:
   - Fishing still hand-builds its loot. The shared loot engine cannot yet express
     nested loot-table entries, quality weights, fishing/open-water and biome
     predicates, and all required item functions. Replacing the fishing helper
     with the current engine would silently remove valid catches.
   - Brush archaeology still advances from click handling rather than vanilla's
     continuous-use tick cadence; offhand use and loot context need a full port.
   - Spawn-egg offspring still use generic entity construction/baby metadata,
     rather than the proper ageable offspring factory and eligibility checks.
   - Sweep damage scaling, enchantment effects, knockback, and movement gating
     require further review beyond the target-box correction.
   - Specialized mob reactions to blocked attacks (notably ravager stun), custom
     component edge cases, protocol-version differences, and the broader item
     component/interaction pipeline still need comparison.
5. The earlier requested regression/gameplay comparison remains unperformed.
   Compilation and testing remain prohibited unless the user changes that instruction.
