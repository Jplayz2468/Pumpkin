# Block-first parity plan

## Working agreement

- Current user priority (2026-09-15): close block/shared-engine gaps before adding more mobs. Do not perform a full mob pass yet. Retain the item queue and reuse existing source work.
- Batch related engine fixes from the dependency queue; background tests run while implementation/review continues.
- A reviewed batch means its block handlers were compared and local fixes made. It does **not** mean verified 1:1 parity.
- Background tests are now authorized (2026-09-15). Keep useful work moving while they run; inspect finished results without waiting loops. Historical no-test statements in the log describe earlier work.
- Checkpoint each batch. Report the batch, concrete fixes, open dependencies and actual test status.

## Fixed block batches

| Batch | Scope | Source files | State |
| --- | --- | ---: | --- |
| B01 | Workstations | 8 | Reviewed; menu dependencies queued |
| B02 | Inventory automation | 4 | Reviewed; container/recipe dependencies queued |
| B03 | Special gameplay blocks | 8 | Reviewed; substantial BE/entity dependencies queued |
| B04 | Signs, banners, heads and light | 4 | Reviewed; item/component/protocol dependencies queued |
| B05 | Portals and gateways | 4 | Reviewed; transition/entity dependencies queued |
| B06 | Administrative and invisible blocks | 6 | Reviewed; execution/structure/protocol dependencies queued |
| B07 | Redstone and pistons | 33 | Reviewed; movement/ordering dependencies queued |
| B08 | Building shapes and placement | 15 | Reviewed; shared collision/fluid dependencies queued |
| B09 | Plants and crops | 54 | Carried source passes; documented dependencies retained |
| B10 | Sculk and bees | 8 | Carried source passes; documented dependencies retained |
| B11 | Terrain, growth and physical effects | 32 | Carried source passes; documented dependencies retained |
| B12 | Containers and furnishings | 23 | Carried source passes; documented dependencies retained |
| B13 | Remaining blocks and shared module routing | 23 | Reviewed/carried; entity-effect dependencies retained |
| B14 | Vanilla registry, inheritance and data coverage | registry gate | Data checked; exhaustive behavior/shape gate remains open |

## Source checklist

The inventory below covers all 222 Rust block source files except the dedicated `tests.rs` file. Module/assembly files are retained so shared handlers are not omitted. File counts are **not block-ID counts or a parity percentage**.

Statuses: **Reviewed** = this batch compared the source; **Carried** = documented prior source work in [the implementation log](BLOCK_ITEM_PARITY.md), with its remaining limitations retained; **Pending** = still needs a source pass or routing review.

| Batch | Source file | Status |
| --- | --- | --- |
| B01 | [cartography_table.rs](crates/pumpkin/src/block/blocks/cartography_table.rs) | Reviewed |
| B01 | [crafting_table.rs](crates/pumpkin/src/block/blocks/crafting_table.rs) | Reviewed |
| B01 | [enchanting_table.rs](crates/pumpkin/src/block/blocks/enchanting_table.rs) | Reviewed |
| B01 | [fletching_table.rs](crates/pumpkin/src/block/blocks/fletching_table.rs) | Reviewed |
| B01 | [grindstone.rs](crates/pumpkin/src/block/blocks/grindstone.rs) | Reviewed |
| B01 | [loom.rs](crates/pumpkin/src/block/blocks/loom.rs) | Reviewed |
| B01 | [smithing_table.rs](crates/pumpkin/src/block/blocks/smithing_table.rs) | Reviewed |
| B01 | [stonecutter.rs](crates/pumpkin/src/block/blocks/stonecutter.rs) | Reviewed |
| B02 | [hopper.rs](crates/pumpkin/src/block/blocks/hopper.rs) | Reviewed |
| B02 | [redstone/crafter.rs](crates/pumpkin/src/block/blocks/redstone/crafter.rs) | Reviewed |
| B02 | [redstone/dispenser.rs](crates/pumpkin/src/block/blocks/redstone/dispenser.rs) | Reviewed |
| B02 | [redstone/dropper.rs](crates/pumpkin/src/block/blocks/redstone/dropper.rs) | Reviewed |
| B03 | [beacon.rs](crates/pumpkin/src/block/blocks/beacon.rs) | Reviewed |
| B03 | [carved_pumpkin.rs](crates/pumpkin/src/block/blocks/carved_pumpkin.rs) | Reviewed |
| B03 | [creaking_heart.rs](crates/pumpkin/src/block/blocks/creaking_heart.rs) | Reviewed |
| B03 | [spawner.rs](crates/pumpkin/src/block/blocks/spawner.rs) | Reviewed |
| B03 | [tnt.rs](crates/pumpkin/src/block/blocks/tnt.rs) | Reviewed |
| B03 | [trial_spawner.rs](crates/pumpkin/src/block/blocks/trial_spawner.rs) | Reviewed |
| B03 | [vault.rs](crates/pumpkin/src/block/blocks/vault.rs) | Reviewed |
| B03 | [wither_skull.rs](crates/pumpkin/src/block/blocks/wither_skull.rs) | Reviewed |
| B04 | [banners.rs](crates/pumpkin/src/block/blocks/banners.rs) | Reviewed |
| B04 | [light.rs](crates/pumpkin/src/block/blocks/light.rs) | Reviewed |
| B04 | [signs.rs](crates/pumpkin/src/block/blocks/signs.rs) | Reviewed |
| B04 | [skull_block.rs](crates/pumpkin/src/block/blocks/skull_block.rs) | Reviewed |
| B05 | [end_gateway.rs](crates/pumpkin/src/block/blocks/end_gateway.rs) | Reviewed |
| B05 | [end_portal.rs](crates/pumpkin/src/block/blocks/end_portal.rs) | Reviewed |
| B05 | [end_portal_frame.rs](crates/pumpkin/src/block/blocks/end_portal_frame.rs) | Reviewed |
| B05 | [nether_portal.rs](crates/pumpkin/src/block/blocks/nether_portal.rs) | Reviewed |
| B06 | [barrier.rs](crates/pumpkin/src/block/blocks/barrier.rs) | Reviewed |
| B06 | [command.rs](crates/pumpkin/src/block/blocks/command.rs) | Reviewed |
| B06 | [jigsaw.rs](crates/pumpkin/src/block/blocks/jigsaw.rs) | Reviewed |
| B06 | [structure_block.rs](crates/pumpkin/src/block/blocks/structure_block.rs) | Reviewed |
| B06 | [structure_void.rs](crates/pumpkin/src/block/blocks/structure_void.rs) | Reviewed |
| B06 | [test_block.rs](crates/pumpkin/src/block/blocks/test_block.rs) | Reviewed |
| B07 | [piston/mod.rs](crates/pumpkin/src/block/blocks/piston/mod.rs) | Reviewed |
| B07 | [piston/piston.rs](crates/pumpkin/src/block/blocks/piston/piston.rs) | Reviewed |
| B07 | [piston/piston_extension.rs](crates/pumpkin/src/block/blocks/piston/piston_extension.rs) | Reviewed |
| B07 | [piston/piston_head.rs](crates/pumpkin/src/block/blocks/piston/piston_head.rs) | Reviewed |
| B07 | [redstone/abstract_redstone_gate.rs](crates/pumpkin/src/block/blocks/redstone/abstract_redstone_gate.rs) | Reviewed |
| B07 | [redstone/bell.rs](crates/pumpkin/src/block/blocks/redstone/bell.rs) | Carried |
| B07 | [redstone/buttons.rs](crates/pumpkin/src/block/blocks/redstone/buttons.rs) | Carried |
| B07 | [redstone/comparator.rs](crates/pumpkin/src/block/blocks/redstone/comparator.rs) | Carried |
| B07 | [redstone/copper_bulb.rs](crates/pumpkin/src/block/blocks/redstone/copper_bulb.rs) | Reviewed |
| B07 | [redstone/daylight_detector.rs](crates/pumpkin/src/block/blocks/redstone/daylight_detector.rs) | Reviewed |
| B07 | [redstone/lever.rs](crates/pumpkin/src/block/blocks/redstone/lever.rs) | Carried |
| B07 | [redstone/lightning_rod.rs](crates/pumpkin/src/block/blocks/redstone/lightning_rod.rs) | Reviewed |
| B07 | [redstone/mod.rs](crates/pumpkin/src/block/blocks/redstone/mod.rs) | Reviewed |
| B07 | [redstone/observer.rs](crates/pumpkin/src/block/blocks/redstone/observer.rs) | Reviewed |
| B07 | [redstone/pressure_plate/mod.rs](crates/pumpkin/src/block/blocks/redstone/pressure_plate/mod.rs) | Reviewed |
| B07 | [redstone/pressure_plate/plate.rs](crates/pumpkin/src/block/blocks/redstone/pressure_plate/plate.rs) | Reviewed |
| B07 | [redstone/pressure_plate/weighted.rs](crates/pumpkin/src/block/blocks/redstone/pressure_plate/weighted.rs) | Reviewed |
| B07 | [redstone/rails/activator_rail.rs](crates/pumpkin/src/block/blocks/redstone/rails/activator_rail.rs) | Carried |
| B07 | [redstone/rails/common.rs](crates/pumpkin/src/block/blocks/redstone/rails/common.rs) | Carried |
| B07 | [redstone/rails/detector_rail.rs](crates/pumpkin/src/block/blocks/redstone/rails/detector_rail.rs) | Carried |
| B07 | [redstone/rails/mod.rs](crates/pumpkin/src/block/blocks/redstone/rails/mod.rs) | Carried |
| B07 | [redstone/rails/powered_rail.rs](crates/pumpkin/src/block/blocks/redstone/rails/powered_rail.rs) | Carried |
| B07 | [redstone/rails/rail.rs](crates/pumpkin/src/block/blocks/redstone/rails/rail.rs) | Carried |
| B07 | [redstone/redstone_block.rs](crates/pumpkin/src/block/blocks/redstone/redstone_block.rs) | Reviewed |
| B07 | [redstone/redstone_lamp.rs](crates/pumpkin/src/block/blocks/redstone/redstone_lamp.rs) | Reviewed |
| B07 | [redstone/redstone_ore.rs](crates/pumpkin/src/block/blocks/redstone/redstone_ore.rs) | Reviewed |
| B07 | [redstone/redstone_torch.rs](crates/pumpkin/src/block/blocks/redstone/redstone_torch.rs) | Carried |
| B07 | [redstone/redstone_wire.rs](crates/pumpkin/src/block/blocks/redstone/redstone_wire.rs) | Reviewed |
| B07 | [redstone/repeater.rs](crates/pumpkin/src/block/blocks/redstone/repeater.rs) | Carried |
| B07 | [redstone/sculk_sensor.rs](crates/pumpkin/src/block/blocks/redstone/sculk_sensor.rs) | Carried |
| B07 | [redstone/target_block.rs](crates/pumpkin/src/block/blocks/redstone/target_block.rs) | Carried |
| B07 | [redstone/tripwire.rs](crates/pumpkin/src/block/blocks/redstone/tripwire.rs) | Reviewed |
| B07 | [redstone/tripwire_hook.rs](crates/pumpkin/src/block/blocks/redstone/tripwire_hook.rs) | Reviewed |
| B08 | [chain.rs](crates/pumpkin/src/block/blocks/chain.rs) | Reviewed |
| B08 | [end_rod.rs](crates/pumpkin/src/block/blocks/end_rod.rs) | Reviewed |
| B08 | [fences.rs](crates/pumpkin/src/block/blocks/fences.rs) | Reviewed |
| B08 | [glass_panes.rs](crates/pumpkin/src/block/blocks/glass_panes.rs) | Reviewed |
| B08 | [glazed_terracotta.rs](crates/pumpkin/src/block/blocks/glazed_terracotta.rs) | Reviewed |
| B08 | [heavy_core.rs](crates/pumpkin/src/block/blocks/heavy_core.rs) | Reviewed |
| B08 | [iron_bars.rs](crates/pumpkin/src/block/blocks/iron_bars.rs) | Reviewed |
| B08 | [ladder.rs](crates/pumpkin/src/block/blocks/ladder.rs) | Reviewed |
| B08 | [lanterns.rs](crates/pumpkin/src/block/blocks/lanterns.rs) | Reviewed |
| B08 | [scaffolding.rs](crates/pumpkin/src/block/blocks/scaffolding.rs) | Reviewed |
| B08 | [slabs.rs](crates/pumpkin/src/block/blocks/slabs.rs) | Reviewed |
| B08 | [stairs.rs](crates/pumpkin/src/block/blocks/stairs.rs) | Reviewed |
| B08 | [torches.rs](crates/pumpkin/src/block/blocks/torches.rs) | Reviewed |
| B08 | [walls.rs](crates/pumpkin/src/block/blocks/walls.rs) | Carried |
| B08 | [weathering_copper.rs](crates/pumpkin/src/block/blocks/weathering_copper.rs) | Reviewed |
| B09 | [plant/azalea.rs](crates/pumpkin/src/block/blocks/plant/azalea.rs) | Carried |
| B09 | [plant/bamboo.rs](crates/pumpkin/src/block/blocks/plant/bamboo.rs) | Carried |
| B09 | [plant/bamboo_sapling.rs](crates/pumpkin/src/block/blocks/plant/bamboo_sapling.rs) | Carried |
| B09 | [plant/big_dripleaf.rs](crates/pumpkin/src/block/blocks/plant/big_dripleaf.rs) | Carried |
| B09 | [plant/big_dripleaf_stem.rs](crates/pumpkin/src/block/blocks/plant/big_dripleaf_stem.rs) | Carried |
| B09 | [plant/bush.rs](crates/pumpkin/src/block/blocks/plant/bush.rs) | Carried |
| B09 | [plant/cactus.rs](crates/pumpkin/src/block/blocks/plant/cactus.rs) | Carried |
| B09 | [plant/cactus_flower.rs](crates/pumpkin/src/block/blocks/plant/cactus_flower.rs) | Reviewed |
| B09 | [plant/cave_vines.rs](crates/pumpkin/src/block/blocks/plant/cave_vines.rs) | Carried |
| B09 | [plant/chorus_flower.rs](crates/pumpkin/src/block/blocks/plant/chorus_flower.rs) | Carried |
| B09 | [plant/chorus_plant.rs](crates/pumpkin/src/block/blocks/plant/chorus_plant.rs) | Carried |
| B09 | [plant/cocoa.rs](crates/pumpkin/src/block/blocks/plant/cocoa.rs) | Carried |
| B09 | [plant/crop/beetroot.rs](crates/pumpkin/src/block/blocks/plant/crop/beetroot.rs) | Reviewed |
| B09 | [plant/crop/carrot.rs](crates/pumpkin/src/block/blocks/plant/crop/carrot.rs) | Carried |
| B09 | [plant/crop/gourds/attached_stem.rs](crates/pumpkin/src/block/blocks/plant/crop/gourds/attached_stem.rs) | Carried |
| B09 | [plant/crop/gourds/mod.rs](crates/pumpkin/src/block/blocks/plant/crop/gourds/mod.rs) | Carried |
| B09 | [plant/crop/gourds/stem.rs](crates/pumpkin/src/block/blocks/plant/crop/gourds/stem.rs) | Carried |
| B09 | [plant/crop/mod.rs](crates/pumpkin/src/block/blocks/plant/crop/mod.rs) | Carried |
| B09 | [plant/crop/nether_wart.rs](crates/pumpkin/src/block/blocks/plant/crop/nether_wart.rs) | Reviewed |
| B09 | [plant/crop/pitcher_crop.rs](crates/pumpkin/src/block/blocks/plant/crop/pitcher_crop.rs) | Carried |
| B09 | [plant/crop/potatoes.rs](crates/pumpkin/src/block/blocks/plant/crop/potatoes.rs) | Carried |
| B09 | [plant/crop/sweet_berry_bush.rs](crates/pumpkin/src/block/blocks/plant/crop/sweet_berry_bush.rs) | Carried |
| B09 | [plant/crop/torch_flower.rs](crates/pumpkin/src/block/blocks/plant/crop/torch_flower.rs) | Carried |
| B09 | [plant/crop/wheat.rs](crates/pumpkin/src/block/blocks/plant/crop/wheat.rs) | Carried |
| B09 | [plant/dry_vegetation.rs](crates/pumpkin/src/block/blocks/plant/dry_vegetation.rs) | Carried |
| B09 | [plant/eyeblossom.rs](crates/pumpkin/src/block/blocks/plant/eyeblossom.rs) | Carried |
| B09 | [plant/flower.rs](crates/pumpkin/src/block/blocks/plant/flower.rs) | Reviewed |
| B09 | [plant/flowerbed.rs](crates/pumpkin/src/block/blocks/plant/flowerbed.rs) | Carried |
| B09 | [plant/fungus.rs](crates/pumpkin/src/block/blocks/plant/fungus.rs) | Carried |
| B09 | [plant/growing.rs](crates/pumpkin/src/block/blocks/plant/growing.rs) | Carried |
| B09 | [plant/hanging_moss.rs](crates/pumpkin/src/block/blocks/plant/hanging_moss.rs) | Carried |
| B09 | [plant/hanging_roots.rs](crates/pumpkin/src/block/blocks/plant/hanging_roots.rs) | Carried |
| B09 | [plant/kelp.rs](crates/pumpkin/src/block/blocks/plant/kelp.rs) | Carried |
| B09 | [plant/leaf_litter.rs](crates/pumpkin/src/block/blocks/plant/leaf_litter.rs) | Carried |
| B09 | [plant/lily_pad.rs](crates/pumpkin/src/block/blocks/plant/lily_pad.rs) | Carried |
| B09 | [plant/mangrove_propagule.rs](crates/pumpkin/src/block/blocks/plant/mangrove_propagule.rs) | Carried |
| B09 | [plant/mod.rs](crates/pumpkin/src/block/blocks/plant/mod.rs) | Carried |
| B09 | [plant/mushroom_plant.rs](crates/pumpkin/src/block/blocks/plant/mushroom_plant.rs) | Carried |
| B09 | [plant/nether_sprouts.rs](crates/pumpkin/src/block/blocks/plant/nether_sprouts.rs) | Reviewed |
| B09 | [plant/roots.rs](crates/pumpkin/src/block/blocks/plant/roots.rs) | Reviewed |
| B09 | [plant/sapling.rs](crates/pumpkin/src/block/blocks/plant/sapling.rs) | Carried |
| B09 | [plant/sea_pickles.rs](crates/pumpkin/src/block/blocks/plant/sea_pickles.rs) | Carried |
| B09 | [plant/seagrass.rs](crates/pumpkin/src/block/blocks/plant/seagrass.rs) | Carried |
| B09 | [plant/segmented.rs](crates/pumpkin/src/block/blocks/plant/segmented.rs) | Carried |
| B09 | [plant/short_plant.rs](crates/pumpkin/src/block/blocks/plant/short_plant.rs) | Carried |
| B09 | [plant/small_dripleaf.rs](crates/pumpkin/src/block/blocks/plant/small_dripleaf.rs) | Carried |
| B09 | [plant/spore_blossom.rs](crates/pumpkin/src/block/blocks/plant/spore_blossom.rs) | Carried |
| B09 | [plant/sugar_cane.rs](crates/pumpkin/src/block/blocks/plant/sugar_cane.rs) | Carried |
| B09 | [plant/tall_plant.rs](crates/pumpkin/src/block/blocks/plant/tall_plant.rs) | Carried |
| B09 | [plant/tall_seagrass.rs](crates/pumpkin/src/block/blocks/plant/tall_seagrass.rs) | Carried |
| B09 | [plant/tree_grower.rs](crates/pumpkin/src/block/blocks/plant/tree_grower.rs) | Carried |
| B09 | [plant/twisting_vines.rs](crates/pumpkin/src/block/blocks/plant/twisting_vines.rs) | Carried |
| B09 | [plant/weeping_vines.rs](crates/pumpkin/src/block/blocks/plant/weeping_vines.rs) | Carried |
| B09 | [plant/wither_rose.rs](crates/pumpkin/src/block/blocks/plant/wither_rose.rs) | Carried |
| B10 | [beehive.rs](crates/pumpkin/src/block/blocks/beehive.rs) | Carried |
| B10 | [sculk/mod.rs](crates/pumpkin/src/block/blocks/sculk/mod.rs) | Reviewed |
| B10 | [sculk/sculk_catalyst.rs](crates/pumpkin/src/block/blocks/sculk/sculk_catalyst.rs) | Carried |
| B10 | [sculk/sculk_shrieker.rs](crates/pumpkin/src/block/blocks/sculk/sculk_shrieker.rs) | Carried |
| B10 | [sculk/sculk_vein.rs](crates/pumpkin/src/block/blocks/sculk/sculk_vein.rs) | Carried |
| B10 | [sculk/shrieker_rules.rs](crates/pumpkin/src/block/blocks/sculk/shrieker_rules.rs) | Carried |
| B10 | [sculk/spreader.rs](crates/pumpkin/src/block/blocks/sculk/spreader.rs) | Carried |
| B10 | [sculk/vibration.rs](crates/pumpkin/src/block/blocks/sculk/vibration.rs) | Carried |
| B11 | [anvil.rs](crates/pumpkin/src/block/blocks/anvil.rs) | Carried |
| B11 | [brushable_block.rs](crates/pumpkin/src/block/blocks/brushable_block.rs) | Carried |
| B11 | [bubble_column.rs](crates/pumpkin/src/block/blocks/bubble_column.rs) | Carried |
| B11 | [coral/coral_block.rs](crates/pumpkin/src/block/blocks/coral/coral_block.rs) | Carried |
| B11 | [coral/coral_fan.rs](crates/pumpkin/src/block/blocks/coral/coral_fan.rs) | Carried |
| B11 | [coral/coral_plant.rs](crates/pumpkin/src/block/blocks/coral/coral_plant.rs) | Carried |
| B11 | [coral/mod.rs](crates/pumpkin/src/block/blocks/coral/mod.rs) | Reviewed |
| B11 | [dirt_path.rs](crates/pumpkin/src/block/blocks/dirt_path.rs) | Carried |
| B11 | [dragon_egg.rs](crates/pumpkin/src/block/blocks/dragon_egg.rs) | Carried |
| B11 | [dried_ghast.rs](crates/pumpkin/src/block/blocks/dried_ghast.rs) | Carried |
| B11 | [dripstone.rs](crates/pumpkin/src/block/blocks/dripstone.rs) | Carried |
| B11 | [falling.rs](crates/pumpkin/src/block/blocks/falling.rs) | Carried |
| B11 | [farmland.rs](crates/pumpkin/src/block/blocks/farmland.rs) | Carried |
| B11 | [fire/fire.rs](crates/pumpkin/src/block/blocks/fire/fire.rs) | Carried |
| B11 | [fire/mod.rs](crates/pumpkin/src/block/blocks/fire/mod.rs) | Reviewed |
| B11 | [fire/soul_fire.rs](crates/pumpkin/src/block/blocks/fire/soul_fire.rs) | Carried |
| B11 | [frogspawn.rs](crates/pumpkin/src/block/blocks/frogspawn.rs) | Carried |
| B11 | [grass_block.rs](crates/pumpkin/src/block/blocks/grass_block.rs) | Carried |
| B11 | [ice.rs](crates/pumpkin/src/block/blocks/ice.rs) | Carried |
| B11 | [moss_block.rs](crates/pumpkin/src/block/blocks/moss_block.rs) | Carried |
| B11 | [mud.rs](crates/pumpkin/src/block/blocks/mud.rs) | Carried |
| B11 | [netherrack.rs](crates/pumpkin/src/block/blocks/netherrack.rs) | Carried |
| B11 | [nylium.rs](crates/pumpkin/src/block/blocks/nylium.rs) | Carried |
| B11 | [potent_sulfur.rs](crates/pumpkin/src/block/blocks/potent_sulfur.rs) | Carried |
| B11 | [powder_snow.rs](crates/pumpkin/src/block/blocks/powder_snow.rs) | Carried |
| B11 | [rooted_dirt.rs](crates/pumpkin/src/block/blocks/rooted_dirt.rs) | Carried |
| B11 | [sniffer_egg.rs](crates/pumpkin/src/block/blocks/sniffer_egg.rs) | Carried |
| B11 | [snow.rs](crates/pumpkin/src/block/blocks/snow.rs) | Carried |
| B11 | [sponge.rs](crates/pumpkin/src/block/blocks/sponge.rs) | Carried |
| B11 | [spreading_snowy_block.rs](crates/pumpkin/src/block/blocks/spreading_snowy_block.rs) | Carried |
| B11 | [sulfur_spike.rs](crates/pumpkin/src/block/blocks/sulfur_spike.rs) | Carried |
| B11 | [turtle_egg.rs](crates/pumpkin/src/block/blocks/turtle_egg.rs) | Carried |
| B12 | [barrel.rs](crates/pumpkin/src/block/blocks/barrel.rs) | Carried |
| B12 | [bed.rs](crates/pumpkin/src/block/blocks/bed.rs) | Carried |
| B12 | [blast_furnace.rs](crates/pumpkin/src/block/blocks/blast_furnace.rs) | Carried |
| B12 | [brewing_stand.rs](crates/pumpkin/src/block/blocks/brewing_stand.rs) | Carried |
| B12 | [cake.rs](crates/pumpkin/src/block/blocks/cake.rs) | Carried |
| B12 | [campfire.rs](crates/pumpkin/src/block/blocks/campfire.rs) | Carried |
| B12 | [candle_cakes.rs](crates/pumpkin/src/block/blocks/candle_cakes.rs) | Carried |
| B12 | [candles.rs](crates/pumpkin/src/block/blocks/candles.rs) | Carried |
| B12 | [cauldron.rs](crates/pumpkin/src/block/blocks/cauldron.rs) | Carried |
| B12 | [chests.rs](crates/pumpkin/src/block/blocks/chests.rs) | Carried |
| B12 | [chiseled_bookshelf.rs](crates/pumpkin/src/block/blocks/chiseled_bookshelf.rs) | Carried |
| B12 | [composter.rs](crates/pumpkin/src/block/blocks/composter.rs) | Carried |
| B12 | [conduit.rs](crates/pumpkin/src/block/blocks/conduit.rs) | Carried |
| B12 | [decorated_pot.rs](crates/pumpkin/src/block/blocks/decorated_pot.rs) | Carried |
| B12 | [ender_chest.rs](crates/pumpkin/src/block/blocks/ender_chest.rs) | Carried |
| B12 | [flower_pots.rs](crates/pumpkin/src/block/blocks/flower_pots.rs) | Carried |
| B12 | [furnace.rs](crates/pumpkin/src/block/blocks/furnace.rs) | Carried |
| B12 | [jukebox.rs](crates/pumpkin/src/block/blocks/jukebox.rs) | Carried |
| B12 | [lectern.rs](crates/pumpkin/src/block/blocks/lectern.rs) | Carried |
| B12 | [respawn_anchor.rs](crates/pumpkin/src/block/blocks/respawn_anchor.rs) | Carried |
| B12 | [shelf.rs](crates/pumpkin/src/block/blocks/shelf.rs) | Carried |
| B12 | [shulker_box.rs](crates/pumpkin/src/block/blocks/shulker_box.rs) | Carried |
| B12 | [smoker.rs](crates/pumpkin/src/block/blocks/smoker.rs) | Carried |
| B13 | [abstract_wall_mounting.rs](crates/pumpkin/src/block/blocks/abstract_wall_mounting.rs) | Reviewed |
| B13 | [amethyst.rs](crates/pumpkin/src/block/blocks/amethyst.rs) | Carried |
| B13 | [carpet.rs](crates/pumpkin/src/block/blocks/carpet.rs) | Carried |
| B13 | [cobweb.rs](crates/pumpkin/src/block/blocks/cobweb.rs) | Reviewed |
| B13 | [doors.rs](crates/pumpkin/src/block/blocks/doors.rs) | Carried |
| B13 | [fence_gates.rs](crates/pumpkin/src/block/blocks/fence_gates.rs) | Carried |
| B13 | [hay.rs](crates/pumpkin/src/block/blocks/hay.rs) | Reviewed |
| B13 | [honey.rs](crates/pumpkin/src/block/blocks/honey.rs) | Reviewed |
| B13 | [huge_mushroom.rs](crates/pumpkin/src/block/blocks/huge_mushroom.rs) | Carried |
| B13 | [infested.rs](crates/pumpkin/src/block/blocks/infested.rs) | Carried |
| B13 | [infested_rotated_pillar.rs](crates/pumpkin/src/block/blocks/infested_rotated_pillar.rs) | Carried |
| B13 | [leaves.rs](crates/pumpkin/src/block/blocks/leaves.rs) | Carried |
| B13 | [logs.rs](crates/pumpkin/src/block/blocks/logs.rs) | Reviewed |
| B13 | [magma.rs](crates/pumpkin/src/block/blocks/magma.rs) | Reviewed |
| B13 | [mangrove_roots.rs](crates/pumpkin/src/block/blocks/mangrove_roots.rs) | Reviewed |
| B13 | [mod.rs](crates/pumpkin/src/block/blocks/mod.rs) | Reviewed |
| B13 | [note.rs](crates/pumpkin/src/block/blocks/note.rs) | Carried |
| B13 | [pumpkin.rs](crates/pumpkin/src/block/blocks/pumpkin.rs) | Reviewed |
| B13 | [slime.rs](crates/pumpkin/src/block/blocks/slime.rs) | Reviewed |
| B13 | [soul_sand.rs](crates/pumpkin/src/block/blocks/soul_sand.rs) | Reviewed |
| B13 | [tinted_glass.rs](crates/pumpkin/src/block/blocks/tinted_glass.rs) | Reviewed |
| B13 | [trapdoor.rs](crates/pumpkin/src/block/blocks/trapdoor.rs) | Carried |
| B13 | [vine.rs](crates/pumpkin/src/block/blocks/vine.rs) | Reviewed |

## B01 evidence and limits

- Reference: local 26.2 CraftingTableBlock, CartographyTableBlock, LoomBlock, SmithingTableBlock, StonecutterBlock, GrindstoneBlock, EnchantingTableBlock and EnchantingTableBlockEntity. Fletching table is a plain Block registration in Blocks.java; its pass-through handler needs no local change.
- Fixed menu-before-stat order in six handlers; loom placement uses horizontal facing even when looking vertically.
- Enchanting now requires its existing BE, retains names through placement/save/drop/menu title, and counts the 32 source offsets using provider/transmitter tags at the matching height.
- Grindstone support-free placement remains consistent with source canSurvive=true. Existing common placement and shape-data mechanisms remain dependencies.
- Enchanting bookshelf recalculation while the menu stays open, menu outputs/costs/consumption, and shared screen access/lifecycle move to I02. Generated shape/data correspondence moves to B14.

## B14 coverage gate

- Account for every entry in [the vanilla block source inventory](PARITY_VANILLA_BLOCK_SOURCES.md), including classes with no dedicated Rust file.
- Reconcile runtime registry IDs and vanilla Blocks.java constructors with handlers or inherited/default behavior. A registered ID alone does not close the gate.
- Compare generated properties/defaults/shapes, tags and block loot declarations. Record unhandled behavior here and retain unresolved coverage work when moving to mobs.

## Shared dependency queue

| ID | Work | Impact / routing |
| --- | --- | --- |
| D01 | Swept collision, inside-effect ordering/deduplication, dynamic shapes and movement | Swept traversal and step collector are oracle-checked; fluid integration, support selection and item replay are wired. Step-up and entity/border gathering are implemented; dynamic shape grids, specialized movement and gameplay verification remain. |
| D02 | Loot functions, predicates, random sequences, contexts and reward rules | Blocks, containers, fishing and other item results; no placeholder loot claims. |
| D03 | Neighbor/scheduled-tick ordering, chunk edges and block-entity lifecycle | Includes opener recheck timing and experimental redstone orientation. |
| D04 | Entity random streams, custom environment attributes and nonplayer behavior | Includes breathing, piglin container anger and other documented entity dependencies. |
| D05 | Container lock predicates and notifications | Shared by chests, barrels, shulkers, processors and other named containers. |
| D06 | Protocol/version and Bedrock behavior | Keep distinct from Java source coverage. |
| D07 | Integration and background test failures | Fix compiler/API mismatches in one batch; separate these from gameplay source review. |

## Item batches (after the requested mob pass)

- I01: placement, buckets, bottles, tools and block interaction items.
- I02: crafting/processing menus, enchanting, repairs, inventory transfers and shared item components.
- I03: food, potions, continuous use and equipment.
- I04: weapons, projectiles, transport, spawning, maps/fishing and remaining items.
- I05: reconcile every item registry ID and shared component path; then close shared dependencies and background verification gaps.

Item source inventory:

- [ ] [armor_stand.rs](crates/pumpkin/src/item/items/armor_stand.rs)
- [ ] [arrow.rs](crates/pumpkin/src/item/items/arrow.rs)
- [ ] [axe.rs](crates/pumpkin/src/item/items/axe.rs)
- [ ] [boat.rs](crates/pumpkin/src/item/items/boat.rs)
- [ ] [bone_meal.rs](crates/pumpkin/src/item/items/bone_meal.rs)
- [ ] [bow.rs](crates/pumpkin/src/item/items/bow.rs)
- [ ] [brush.rs](crates/pumpkin/src/item/items/brush.rs)
- [ ] [bucket.rs](crates/pumpkin/src/item/items/bucket.rs)
- [ ] [bundle.rs](crates/pumpkin/src/item/items/bundle.rs)
- [ ] [clock.rs](crates/pumpkin/src/item/items/clock.rs)
- [ ] [compass.rs](crates/pumpkin/src/item/items/compass.rs)
- [ ] [crossbow.rs](crates/pumpkin/src/item/items/crossbow.rs)
- [ ] [debug_stick.rs](crates/pumpkin/src/item/items/debug_stick.rs)
- [ ] [dye.rs](crates/pumpkin/src/item/items/dye.rs)
- [ ] [egg.rs](crates/pumpkin/src/item/items/egg.rs)
- [ ] [end_crystal.rs](crates/pumpkin/src/item/items/end_crystal.rs)
- [ ] [ender_eye.rs](crates/pumpkin/src/item/items/ender_eye.rs)
- [ ] [ender_pearl.rs](crates/pumpkin/src/item/items/ender_pearl.rs)
- [ ] [experience_bottle.rs](crates/pumpkin/src/item/items/experience_bottle.rs)
- [ ] [firework_rocket.rs](crates/pumpkin/src/item/items/firework_rocket.rs)
- [ ] [fishing_rod.rs](crates/pumpkin/src/item/items/fishing_rod.rs)
- [ ] [glass_bottle.rs](crates/pumpkin/src/item/items/glass_bottle.rs)
- [ ] [glowing_ink_sac.rs](crates/pumpkin/src/item/items/glowing_ink_sac.rs)
- [ ] [goat_horn.rs](crates/pumpkin/src/item/items/goat_horn.rs)
- [ ] [hanging_entity.rs](crates/pumpkin/src/item/items/hanging_entity.rs)
- [ ] [hoe.rs](crates/pumpkin/src/item/items/hoe.rs)
- [ ] [honeycomb.rs](crates/pumpkin/src/item/items/honeycomb.rs)
- [ ] [ignite/fire_charge.rs](crates/pumpkin/src/item/items/ignite/fire_charge.rs)
- [ ] [ignite/flint_and_steel.rs](crates/pumpkin/src/item/items/ignite/flint_and_steel.rs)
- [ ] [ignite/ignition.rs](crates/pumpkin/src/item/items/ignite/ignition.rs)
- [ ] [ignite/mod.rs](crates/pumpkin/src/item/items/ignite/mod.rs)
- [ ] [ink_sac.rs](crates/pumpkin/src/item/items/ink_sac.rs)
- [ ] [knowledge_book.rs](crates/pumpkin/src/item/items/knowledge_book.rs)
- [ ] [lead.rs](crates/pumpkin/src/item/items/lead.rs)
- [ ] [mace.rs](crates/pumpkin/src/item/items/mace.rs)
- [ ] [map.rs](crates/pumpkin/src/item/items/map.rs)
- [ ] [minecart.rs](crates/pumpkin/src/item/items/minecart.rs)
- [ ] [mod.rs](crates/pumpkin/src/item/items/mod.rs)
- [ ] [name_tag.rs](crates/pumpkin/src/item/items/name_tag.rs)
- [ ] [on_a_stick.rs](crates/pumpkin/src/item/items/on_a_stick.rs)
- [ ] [place_on_water.rs](crates/pumpkin/src/item/items/place_on_water.rs)
- [ ] [potions.rs](crates/pumpkin/src/item/items/potions.rs)
- [ ] [projectile_weapon.rs](crates/pumpkin/src/item/items/projectile_weapon.rs)
- [ ] [saddle.rs](crates/pumpkin/src/item/items/saddle.rs)
- [ ] [shears.rs](crates/pumpkin/src/item/items/shears.rs)
- [ ] [shield.rs](crates/pumpkin/src/item/items/shield.rs)
- [ ] [shovel.rs](crates/pumpkin/src/item/items/shovel.rs)
- [ ] [snowball.rs](crates/pumpkin/src/item/items/snowball.rs)
- [ ] [spawn_egg.rs](crates/pumpkin/src/item/items/spawn_egg.rs)
- [ ] [spear.rs](crates/pumpkin/src/item/items/spear.rs)
- [ ] [spyglass.rs](crates/pumpkin/src/item/items/spyglass.rs)
- [ ] [swords.rs](crates/pumpkin/src/item/items/swords.rs)
- [ ] [trident.rs](crates/pumpkin/src/item/items/trident.rs)
- [ ] [wind_charge.rs](crates/pumpkin/src/item/items/wind_charge.rs)
- [ ] [writable_book.rs](crates/pumpkin/src/item/items/writable_book.rs)

## Background verification

- Focused command: `cargo test --offline -p pumpkin --lib block::entities::` using the workspace 1.96 toolchain.
- First attempt stopped on a PotDecorations NBT string type mismatch; fixed `String` → `Box<str>` conversion.
- Second attempt exposed cached generated asset paths from a removed worktree and a removed tall-flower tag. Refreshed the build script output and selected the actual DoublePlantBlock families.
- Third attempt reached the main crate and exposed prior API/import/type integration failures; log: `/tmp/pumpkin-block-pass-tests-3.log`. D07 is active. No passing result is claimed.

### B02 open dependencies

- D02/D05: hopper, dropper, dispenser and crafter stored loot, custom names,
  item components, locks and menu-access checks still need the shared container
  pass; their simple inventory BEs do not yet implement RandomizableContainer.
- D03: exact hopper transfer atomicity/order and shape-fullness obstruction,
  crafter bulk insertion versus one-at-a-time insertion, BE removal/tick order.
- I01/I04: per-item dispenser behaviors (including missing sulfur-cube handling).
- I02: crafter recipe assembly/components, recipe-specific remainders and crafted
  callbacks/advancements; the existing matcher only returns item ID/count.
- D06: item-stack synchronization after partial hopper pickup and client BE data.

### B03 open dependencies

- Vault activation, timed ejection, cooldown, history/queue persistence and shared
  client data are now implemented (see ENGINE_GAPS.md). Remaining D02/D06 scope:
  complete loot functions/predicates, reloadable tables and named random streams,
  malformed config validation, and live protocol/gameplay comparison.
- D03/D04: creaking heart protector spawning/removal, resin production and linked
  entity lifecycle; player-caused explosion XP. The BE currently only tracks signal.
- D03/D04: fully rotated BlockPattern search, copper golem summoning/chest conversion,
  summoned-entity advancements and wither body-yaw data. Upright snow/iron/wither
  shape, clearing, spawn-position and neighbor behavior received local fixes.
- D04: TNT still stores a player-credit flag rather than the full persistent owner.
- D02/D04/D06: spawner/trial-spawner BE algorithms, beacon names/effects/beam
  networking and menu behavior need their detailed dependency passes.

### B04 open dependencies

- I01: hanging-sign chaining clicks and placement editor suppression after applying
  custom block-entity tags; block-item/pick-block component handling for light,
  banner patterns/names and skull profiles/note-block sounds.
- D01/B14: dynamic hanging-sign support/collision shapes and fluid behavior, skull
  context-sensitive replacement tests and complete rotation/mirror coverage.
- D06: filtered sign components, dialogs/custom click actions, command-source
  permission parity, sign editor network behavior, BE persistence/client sync.

### B05 open dependencies

- D03/D04/D06: shared portal processor timing/transition state, End credits and
  respawn targets, passenger trees, gateway pearl handling/tickets and exit
  generation. Gateways still teleport directly rather than through PortalProcessor.
- D04: portal piglin spawn finalization/structure-spawn reason and passenger
  cooldowns; distance-manager fast-path spawning decisions.
- I01/B14: eye-of-ender frame formation, portal pick-block/fluid replacement
  behavior and generated render/shape data.

### B06 open dependencies

- Command success callbacks, once-per-tick tracking/persistence, sequence-limit
  gamerule and disabled-command scheduling are now implemented. Remaining D03/D06:
  wider raw comparator representation, component-form names/output and editor protocol.
- D03: structure block redstone SAVE/LOAD/CORNER operations, template operations
  and structure mode/state initialization; current BE is mainly persisted config.
- I01/B14: game-master item placement and block-state components, barrier bucket
  operations restricted to creative players, test block cloning and invisible shapes.
- D06: jigsaw/test-instance editors and test runner lifecycle.

### B07 open dependencies

- D01/D03: contextual conductor/support shapes, vanilla HashSet neighbor order,
  experimental redstone orientation/evaluator and exact scheduled-update ordering.
  MOVED no longer suppresses shape updates; propagated flags clear drop suppression.
- Piston moving shapes, swept-face queries, collision-clipped pushes (including
  normal player pushes), slime velocity, honey carry, completion shape/drop handling
  and waterlogging are implemented. Remaining D01/D03/D04: full movement-side-effect
  parity, swept inside-effect aggregation, source orientation and live contraptions.
- D02/I01: moving-piston loot and pick-block data; contextual ore BlockItem checks.
- D04/D06: minecart movement/command carts, precise entity collision dispatch and
  redstone BE/network integration retain the limitations from the carried ports.

### B08–B12 retained dependencies

- B08 / D01 / I01: scaffolding contextual collision is implemented. Climbing and
  placement extension, fluid-container acceptance and full shape/data comparison remain. Fence lead
  binding shares the existing item helper; exact leash knot lifecycle remains D04.
- B09 / D01–D03: feature-cache buffering versus interleaved world callbacks/RNG,
  configured tree/provider/height-view behavior, liquid containers, double-plant
  mining/drop ordering and non-world light access. Prior vegetation/crop source
  logs remain the evidence, not a new complete gameplay verification.
- B10 / D01–D04/D06: vibration traversal/dispatch, warden and bee entity lifecycle,
  spreader/worldgen stream interactions and client synchronization retain the
  limits documented in the earlier sculk and bee passes.
- B11 / D01–D04: falling collision/landing pipeline, effect aggregation, fluid
  scheduling and loot contexts remain shared work across the carried terrain ports.
- B12 / D02–D06/I02: locks, loot, names/components, menus/recipes, opener timing,
  collision and protocol dependencies from each container/furnishing log remain.

### B13 / B14 transition checkpoint

- Pumpkin carving now uses the bundled interaction loot table, directional seed
  ejection, carve sound, attributed shear event and used-item statistic. Its loot
  evaluator still has the D02 context/random-stream limitations.
- Vines use normal block-item placement/consumption for extra faces, with corrected
  context direction ordering. Slime applies the source step-speed reduction.
- Honey side sliding, landing/slide status effects and advancement checks are
  implemented. Remaining D01/D04: full shared landing/suppress-bounce behavior,
  magma enchantment-driven immunity and cobweb/inside-effect aggregation.
- B14 data evidence is in PARITY_BLOCK_DATA_AUDIT.md. Exhaustive Java constructor
  inheritance, runtime routing and contextual shape parity remain open. User's
  request to prioritize mobs does not turn this unfinished gate into a pass.
- Next active work: shared engine dependencies; mob passes are paused and item batches remain queued. All block source files now
  have a reviewed or carried source-pass disposition, not a 1:1 certification.

Mob work is paused at the user’s request. See [ENGINE_GAPS.md](ENGINE_GAPS.md) for current engine work and [MOB_PARITY.md](MOB_PARITY.md) for preserved mob scope.
