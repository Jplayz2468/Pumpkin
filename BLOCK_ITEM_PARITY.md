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
3. **Bee lifecycle remains incomplete.** Stored occupants can now leave, but the
   bee's autonomous hive entry, pollination/flight, and neutral anger AI still need
   their mob-side implementations. Explosion release and table-driven honeycomb
   harvest are now implemented. Full custom environment attributes, loot context,
   and random-sequence fidelity remain shared gaps.
4. Finish sculk's upstream game-event coverage. Fishing-hook impact geometry and
   event delivery still need a full port. Player container interaction-range
   rechecks are implemented; non-player users and the broader lifecycle remain open.
   Movement coverage is for living entities; nonliving movement, flapping, and
   movement sound/effect ordering need review. Shrieker controller/item attribution
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
   - Brush archaeology still advances from click handling rather than vanilla's
     continuous-use tick cadence; offhand use and loot context need a full port.
   - Spawn-egg offspring still use generic entity construction/baby metadata,
     rather than the ageable offspring factory and eligibility checks.
   - Sweep damage scaling, enchantment effects, knockback, and movement gating
     require review beyond the target-box correction.
   - Specialized mob reactions to blocked attacks (notably ravager stun), custom
     component edge cases, protocol-version differences, and the broader item
     component/interaction pipeline still need comparison.
7. The earlier regression/gameplay comparison remains unperformed. Compilation and
   testing remain prohibited unless the user changes that instruction.
