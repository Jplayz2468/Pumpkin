use std::sync::Arc;
use std::sync::Mutex;

use crate::block::entities::{PropertyDelegate, furnace_like_block_entity::ExperienceContainer};
use pumpkin_data::{
    BlockStateId, block_properties::FurnaceLikeProperties, screen::WindowType, translation,
};
use pumpkin_inventory::Inventory;
use pumpkin_inventory::{
    furnace_like::furnace_like_screen_handler::FurnaceLikeScreenHandler,
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler},
};
use pumpkin_macros::pumpkin_block;

use crate::block::{
    BlockBehaviour, GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, NormalUseArgs,
    OnPlaceArgs, OnStateReplacedArgs, registry::BlockActionResult,
};

struct FurnaceScreenFactory {
    inventory: Arc<dyn Inventory>,
    property_delegate: Arc<dyn PropertyDelegate>,
    experience_container: Arc<dyn ExperienceContainer>,
}

impl FurnaceScreenFactory {
    fn new(
        inventory: Arc<dyn Inventory>,
        property_delegate: Arc<dyn PropertyDelegate>,
        experience_container: Arc<dyn ExperienceContainer>,
    ) -> Self {
        Self {
            inventory,
            property_delegate,
            experience_container,
        }
    }
}

impl ScreenHandlerFactory for FurnaceScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        if !crate::block::entities::container_lock::can_open_inventory(
            self.inventory.as_ref(),
            player,
            self.get_display_name(),
        ) {
            return None;
        }
        let concrete_handler = FurnaceLikeScreenHandler::new(
            sync_id,
            player_inventory,
            self.inventory.clone(),
            &self.property_delegate,
            self.experience_container.clone(),
            WindowType::Furnace,
        );

        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> pumpkin_util::text::TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_FURNACE,
            translation::bedrock::CONTAINER_FURNACE
        )
    }
}

#[pumpkin_block("minecraft:furnace")]
pub struct FurnaceBlock;

impl BlockBehaviour for FurnaceBlock {
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
                pumpkin_data::statistic::CustomStatistic::InteractWithFurnace as i32,
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
        let inventory = block_entity.clone().get_inventory()?;
        let property_delegate = block_entity.clone().to_property_delegate()?;
        let experience_container = block_entity.to_experience_container()?;
        Some(Box::new(FurnaceScreenFactory::new(
            inventory,
            property_delegate,
            experience_container,
        )))
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = FurnaceLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();

        props.to_state_id(args.block)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    #[test]
    fn menu_honors_main_hand_lock_and_spectator_bypass() {
        let entity = Arc::new(crate::block::entities::furnace::FurnaceBlockEntity::new(
            pumpkin_util::math::position::BlockPos::new(0, 0, 0),
        ));
        let factory = FurnaceScreenFactory::new(entity.clone(), entity.clone(), entity.clone());
        crate::block::entities::container_lock::menu_tests::check(&factory, &[entity.as_ref()]);
    }
}
