# Elytra landing and respawn fixes

This focused branch starts at `138b9264` on `codex/vanilla-spawning`
(code through `d3811eca`). The same flight code is present in the installed
modern build `85cced1e`; the complete patch also passes a cached-index
`git apply --check` against that revision. A backport still needs its own build
and runtime verification. No play-server binary, world, configuration, operator
list, or running comparison server is changed by these source edits.

## Reproduced problems

A copy of installed build `85cced1e` was run with a disposable world on
localhost:25568, independently of the play server and existing spawning labs.
A Java 26.2 protocol client equipped an elytra, started gliding, died, respawned,
and sent a grounded position-and-rotation packet. Shared entity flags stayed
at `128` (bit 7, gliding) while player ability flags were `0` (no flying and no
permission to fly). The client-facing state was stuck, even though respawn had
cleared Pumpkin's separate `fall_flying` boolean. Resetting creative flight alone
could not repair this mismatch. The old reset also bypassed pose/dimension
synchronization by writing `pose` directly.

In a second reproduction, descending gently from Y=199.9 to a floor at Y=191
cost five health points and retained the gliding pose. Only position-only
movement packets cleared gliding on landing. Position-and-rotation, rotation,
and status-only paths differed. Start requests checked only whether the player
was airborne, so missing/broken elytra and repeated start requests were mishandled.

## Official Java reference

The oracle executes original Mojang Java **26.2** classes from server distribution
SHA-1 `823e2250d24b3ddac457a60c92a6a941943fcd6a`. Game jars and disassembly are
not checked in. The driver and runtime probes live in the hoster's
`flight-comparison/` directory. The Rust fixture contains 245 results from:

- `Entity.checkFallDistanceAccumulation`: while gliding, downward velocity
  strictly greater than -0.5 caps fall distance at 1; exactly -0.5 does not.
- `LivingEntity.calculateFallDamage`: floor of
  `(distance + 1e-6 - safeFallDistance) * blockMultiplier * fallDamageMultiplier`.
- `LivingEntity.handleFallFlyingCollisions`: positive damage from
  `(horizontalSpeedBefore - horizontalSpeedAfter) * 10 - 3`, only on collision.
- `LivingEntity.canGlideUsing`: glider component, matching equipped slot,
  and remaining usable durability. At 431 damage a 432-durability elytra cannot glide.
- `Player.tryToStartFallFlying` and `stopFallFlying`: a repeated or invalid start
  request stops gliding; water prevents starting.

`Player.canGlide`, `LivingEntity.canGlide`, `ServerPlayer.restoreFrom`, and
`PlayerList.respawn` were additionally inspected in the original class files.
Vanilla creates a fresh player on respawn and applies its game mode. Creative
respawns with flight allowed but inactive; spectator respawns with flight active.

## Implementation

All four Java movement variants share fall/landing handling. The last accepted
client displacement is stored separately from server-tick movement and reset on
teleports/respawns, avoiding tick-phase-dependent impact speeds. Gentle gliding
caps accumulated fall distance; steep dives still hurt. Landing damage honors
the fall-damage-multiplier attribute. Rotation/status readers preserve the two
collision bits separately rather than treating any nonzero flags byte as ground.

Wall damage requires both a client collision report and an actual intersecting
block shape at the contacted horizontal face. Blocked velocity components are
zeroed for post-impact speed: the distance traveled up to the wall is not used
as the remaining speed. Braking or a fabricated collision report in open air
does not cause wall damage.

Death, respawn, NBT restoration, landing, equipment loss, levitation, and ability
flight keep the internal gliding state and shared metadata consistent. Respawn
uses the pose setter to restore dimensions and metadata and reapplies game-mode
ability flags. Invalid starts send a corrective metadata update even if the
server was already not gliding. Local safety policies and Bedrock disabling are
unchanged.

## Scope and limits

This is a focused adaptation to Pumpkin's client-authoritative player movement,
not a replacement with Java's complete server-side travel simulation. Landing
and wall arithmetic match the official oracle for the supplied inputs, but
incoming velocity is inferred from accepted client displacements. Rocket boosts,
packet batching/timing, diagonal/sliding impacts, ladders, fluid travel, and
full trajectory/elytra-wear parity are not claimed. Clients still control movement;
this is not an anti-cheat implementation. `fall_distance` remains Pumpkin's f32
storage; Java uses double precision. Runtime tests use a scripted client, not the
rendered Java game client.

Deployment and a restart of the play server remain separate work.

## Regression evidence

The installed-build copy passed 8/26 packet-level checks; the candidate passed
26/26. Gentle landing changed from 15 health/still gliding to 20 health/standing.
Steep landing retained 15 health while correctly returning to standing. The
straight wall impact changed from no damage to 7 damage. Rotation-only and
status-only wall impacts also hurt. Death and respawn now report flag 7 clear,
standing pose, and survival ability flags clear; creative respawn retains mayfly
while clearing flying. All 245 Java oracle results regenerate byte-for-byte.

The focused Rust tests cover the oracle, gentle-versus-steep landings, all
standard respawn game modes, creative-to-survival flight, and both packed flag
readers. The hoster report records the complete unit suite and binary hashes.
