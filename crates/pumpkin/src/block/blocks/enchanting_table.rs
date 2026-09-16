use std::sync::Arc;
use std::sync::Mutex;

use crate::block::entities::enchanting_table::EnchantingTableBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, GetScreenHandlerFactoryArgs, NormalUseArgs, PathComputationType,
};
use pumpkin_data::BlockState;
use pumpkin_data::tag::Taggable;
use pumpkin_inventory::enchanting::enchanting_screen_handler::EnchantingTableScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_inventory::{Inventory, SimpleInventory};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;

#[pumpkin_block("minecraft:enchanting_table")]
pub struct EnchantingTableBlock;

impl BlockBehaviour for EnchantingTableBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
        }
        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let entity = args.world.get_block_entity(args.position)?;
        let table = entity
            .as_any()
            .downcast_ref::<EnchantingTableBlockEntity>()?;
        let title = table.display_name();
        let bookshelf_count = Self::bookshelf_count(args.world, args.position).min(15);

        let seed = args.player.enchantment_seed();
        Some(Box::new(EnchantingTableScreenFactory {
            bookshelf_count,
            seed,
            title,
        }))
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

impl EnchantingTableBlock {
    /// EnchantingTableBlock.BOOKSHELF_OFFSETS and isValidBookShelf (X-fastest order).
    pub(crate) fn bookshelf_count(world: &crate::world::World, pos: &BlockPos) -> i32 {
        let mut count = 0;
        for z in -2_i32..=2 {
            for y in 0..=1 {
                for x in -2_i32..=2 {
                    if (x.abs() == 2 || z.abs() == 2)
                        && world
                            .get_block(&pos.add(x, y, z))
                            .is_tagged_with("minecraft:enchantment_power_provider")
                            == Some(true)
                        && world
                            .get_block(&pos.add(x / 2, y, z / 2))
                            .is_tagged_with("minecraft:enchantment_power_transmitter")
                            == Some(true)
                    {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

struct EnchantingTableScreenFactory {
    bookshelf_count: i32,
    seed: i32,
    title: TextComponent,
}

impl ScreenHandlerFactory for EnchantingTableScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let inventory: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(2));
        let handler = EnchantingTableScreenHandler::new(
            sync_id,
            player_inventory,
            &inventory,
            self.seed,
            self.bookshelf_count,
        );
        let screen_handler_arc = Arc::new(Mutex::new(handler));
        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        self.title.clone()
    }
}
