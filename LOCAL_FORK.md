# Local survival fork

Repository: https://github.com/Jplayz2468/Pumpkin

Maintained branch: `codex/modern-pumpkin`.
Current upstream base: `4e11bb80` (latest fetched September 13, 2026),
with our safeguards replayed as a separate commit on top.

Original upstream base: `0.1.0-dev+26.2-26.45`, commit
`8d0d0d311778cb0aecb5fc957d571a38f286fda0`.

This branch keeps temporary server policy separate from future vanilla parity
fixes. No automatic source updates or deployments are configured.

## Temporary gameplay policy

Enable in `pumpkin.toml` and restart:

```toml
[local_safety]
disable_shulker_boxes = true
warn_redstone = true
redstone_warning_cooldown_seconds = 30
```

Both boolean options default to false so configurations without this section
retain upstream behavior. Each option can be disabled separately.

- Shulker **boxes**, including every dyed variant: reject player placement,
  opening, and breaking before mutation. Resynchronize predicted placements and
  held inventory without consuming the item. Dispensers retain boxes and play
  their failure sound. Ordinary calls to `World::break_block` also refuse to
  destroy boxes without a player cause.
- The policy does not remove items, recipes, mobs, or existing block entities.
  Administrator commands and code that directly replace world states can bypass
  it. It is not a full protection system for imported worlds: hopper transfers
  and direct world mutation are outside its scope. Store valuable items in
  chests/barrels until shulker persistence is repaired and validated.
- On placing, using, or breaking core redstone components, send a private system
  message explaining that machines may differ from vanilla. Include dust,
  torches, repeaters, comparators, pistons, hoppers, droppers, dispensers, crafters,
  sensors, buttons, pressure plates, rails, copper bulbs and iron doors/trapdoors.
  Ordinary building blocks and redstone ore do not trigger it. Passive circuit
  updates do not broadcast warnings. The cooldown is per connection and separate
  from the five-second shulker refusal notice.
- Shared block-registry paths cover Java and Bedrock placement/use. Bedrock is
  disabled in the local host configuration; its network behavior is not yet
  independently verified.

## Taking upstream changes deliberately

The `origin` remote is our fork; `upstream` is Pumpkin-MC/Pumpkin. The hosting
repository pins this repository as a submodule, so fetching does not change
the running server or the selected source revision.

```sh
git fetch upstream --tags
git log --oneline HEAD..upstream/master
git diff HEAD...upstream/master
```

For a chosen release, create a trial branch from our maintained branch, then
merge the exact selected tag:

```sh
git switch codex/modern-pumpkin
git switch -c codex/trial-release
git merge <selected-release-tag>
git submodule update --init --recursive
```

For an individual upstream fix, create a trial branch in the same way and use
`git cherry-pick -x <fix-commit>`. Inspect dependencies first: one bug fix may
require preceding commits or a new Minecraft data/protocol version. Do not merge
or cherry-pick merely because a new tag exists.

Build, lint, run tests and the manual checks below. Only then fast-forward the
maintained branch to the accepted trial branch, push it to our fork, and update
the hosting repository's submodule pointer. Deployment is a separate step, with
a stopped-world backup of the world, player data, config and previous binary.
Rollbacks after a format change require the matching world backup too.

## Path toward vanilla parity

Keep implementation fixes in separate commits from the temporary policy. For
each mechanic, pin the same Java version on vanilla and Pumpkin, start from
equivalent fixtures, perform identical actions/ticks, and compare server state
plus client-visible behavior. A checkbox or successful build alone is not parity.

1. Shulker persistence: empty/full and all colors; named and component-bearing
   items; survival break/place; creative behavior; dispenser placement; chunk
   unload/reload and server restart; explosions; hopper interactions. Compare
   inventory contents and NBT before/after, with no lost or duplicated items.
2. Pistons: push limits, sticky retraction, quasi-connectivity, short pulses,
   slime/honey, block entities, falling blocks, unloaded chunk boundaries,
   client resynchronization and save/reload.
3. Redstone: update order, directional effects, repeater locking, comparator
   modes, observers, hopper cooldowns, clocks and representative farms. Verify
   exact tick sequences rather than only whether a machine eventually works.

Once a mechanic passes its comparison suite, disable its temporary restriction
in a trial server. Remove policy code only after regression coverage exists.
