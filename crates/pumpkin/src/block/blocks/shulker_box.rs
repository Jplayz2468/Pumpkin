use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{
    BrokenArgs, GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, OnPlaceArgs,
    OnStateReplacedArgs, OnSyncedBlockEventArgs,
};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::BlockEntity;
use crate::block::entities::shulker_box::ShulkerBoxBlockEntity;
use pumpkin_data::BlockStateId;
use pumpkin_data::translation;
use pumpkin_inventory::Inventory;
use pumpkin_inventory::generic_container_screen_handler::create_shulker_box_9x3;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::text::TextComponent;

struct ShulkerBoxScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for ShulkerBoxScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let shulker = self.0.as_any().downcast_ref::<ShulkerBoxBlockEntity>()?;
        if player.is_spectator() && shulker.has_loot_table() {
            return None;
        }
        shulker.unpack_loot(
            player
                .as_any()
                .downcast_ref::<crate::entity::player::Player>(),
        );
        let handler = create_shulker_box_9x3(sync_id, player_inventory, self.0.clone(), player);
        let screen_handler_arc = Arc::new(Mutex::new(handler));

        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        self.0
            .as_any()
            .downcast_ref::<ShulkerBoxBlockEntity>()
            .map_or_else(
                || {
                    pumpkin_macros::translate_cross!(
                        translation::java::CONTAINER_SHULKERBOX,
                        translation::bedrock::CONTAINER_SHULKERBOX
                    )
                },
                ShulkerBoxBlockEntity::display_name,
            )
    }
}

#[pumpkin_block_from_tag("minecraft:shulker_boxes")]
pub struct ShulkerBoxBlock;

type EndRodLikeProperties = pumpkin_data::block_properties::EndRodLikeProperties;

impl BlockBehaviour for ShulkerBoxBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = EndRodLikeProperties::default(args.block);
        props.facing = args.direction.to_facing().opposite();
        props.to_state_id(args.block)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        if args.r#type != Self::OPEN_ANIMATION_EVENT_TYPE {
            return false;
        }
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return false;
        };
        let Some(shulker) = entity.as_any().downcast_ref::<ShulkerBoxBlockEntity>() else {
            return false;
        };
        shulker.trigger_event(args.data);
        true
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return;
        };
        let Some(shulker) = entity.as_any().downcast_ref::<ShulkerBoxBlockEntity>() else {
            return;
        };
        if args.player.gamemode.load() == pumpkin_util::GameMode::Creative && !shulker.is_empty() {
            if let Some(item) = pumpkin_data::item::Item::from_registry_key(args.block.name) {
                let mut stack = pumpkin_data::item_stack::ItemStack::new(1, item);
                shulker.write_dropped_stack_components(&mut stack);
                args.world
                    .drop_stack_at(args.position.to_centered_f64(), stack);
            }
        } else {
            shulker.unpack_loot(Some(args.player));
        }
    }

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
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::OpenShulkerBox as i32,
                1,
            );
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let block_entity = args.world.get_block_entity(args.position)?;
        let shulker = block_entity
            .as_any()
            .downcast_ref::<ShulkerBoxBlockEntity>()?;
        if shulker.is_closed() {
            let state = args.world.get_block_state_id(args.position);
            let bounds = ShulkerBoxBlockEntity::progress_box(state, 0.0, 0.5)
                .at_pos(*args.position)
                .contract_all(1.0e-6);
            if !args.world.is_space_empty(bounds)
                || args
                    .world
                    .get_all_at_box(&bounds)
                    .iter()
                    .any(|entity| entity.is_collidable(None))
            {
                return None;
            }
        }
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(ShulkerBoxScreenFactory(inventory)))
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}

impl ShulkerBoxBlock {
    pub const OPEN_ANIMATION_EVENT_TYPE: u8 = 1;
}
