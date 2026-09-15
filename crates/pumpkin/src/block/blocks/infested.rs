use uuid::Uuid;

use pumpkin_data::Enchantment;
use pumpkin_data::entity::EntityType;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::GameMode;
use pumpkin_util::math::vector3::Vector3;

use crate::block::BlockBehaviour;
use crate::block::BrokenArgs;
use crate::entity::r#type::from_type;

// The `c:cobblestones/infested` tag only lists `infested_cobblestone`
// (assets/datapacks/26_2/data/c/tags/block/cobblestones/infested.json), so the stone/stone-brick
// variants must be registered explicitly here instead of via that tag.
#[pumpkin_block(
    "infested_stone",
    "infested_cobblestone",
    "infested_stone_bricks",
    "infested_mossy_stone_bricks",
    "infested_cracked_stone_bricks",
    "infested_chiseled_stone_bricks"
)]
pub struct InfestedBlock;

impl BlockBehaviour for InfestedBlock {
    /// Vanilla ties the Silverfish spawn to `spawnAfterBreak`, not to the break itself
    /// (`InfestedBlock.java:59-65`), which only runs when the block's drops are actually
    /// processed:
    /// - `ServerPlayerGameMode.destroyBlock` skips `Block.playerDestroy` (and therefore
    ///   `spawnAfterBreak`) entirely when `player.preventsBlockDrops()` -- i.e.
    ///   `abilities.instabuild`, set by Creative mode -- is true, so Creative breaks never
    ///   spawn a Silverfish.
    /// - `spawnAfterBreak` itself only spawns when the `BLOCK_DROPS` (`doTileDrops`) game rule
    ///   is on (`InfestedBlock.java:62`).
    /// - It also requires the breaking tool to lack any enchantment from the
    ///   `prevents_infested_spawns` tag (`InfestedBlock.java:62-64`), which today contains only
    ///   Silk Touch -- this is why Silk-Touch-mining infested blocks never releases a
    ///   Silverfish.
    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() == GameMode::Creative {
            return;
        }

        if !args.world.level_info.load().game_rules.block_drops {
            return;
        }

        let held_item = args.player.inventory().held_item();
        if held_item.get_enchantment_level(&Enchantment::SILK_TOUCH) > 0 {
            return;
        }

        // Vanilla spawns the Silverfish at the block's horizontal center but keeps the floor Y
        // (`InfestedBlock.java:53`: `pos.getX() + 0.5, pos.getY(), pos.getZ() + 0.5`).
        let pos = args.position.0.to_f64();
        let spawn_pos = Vector3::new(pos.x + 0.5, pos.y, pos.z + 0.5);

        // Use the mob factory (not a bare `Entity`) so the Silverfish gets its real AI --
        // wandering, targeting players, calling for help when hurt -- matching
        // `EntityTypes.SILVERFISH.create(...)` in vanilla.
        let silverfish = from_type(&EntityType::SILVERFISH, spawn_pos, args.world, Uuid::new_v4());
        args.world.spawn_entity(silverfish);
    }
}
