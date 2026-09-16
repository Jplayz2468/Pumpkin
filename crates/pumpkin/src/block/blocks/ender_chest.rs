use std::sync::Arc;
use std::sync::Mutex;

use crate::block::entities::ender_chest::EnderChestBlockEntity;
use crate::block::{
    BlockBehaviour, GetScreenHandlerFactoryArgs, GetStateForNeighborUpdateArgs, NormalUseArgs,
    OnPlaceArgs, OnSyncedBlockEventArgs, PathComputationType, registry::BlockActionResult,
};
use crate::world::World;
use pumpkin_data::block_properties::LadderLikeProperties;
use pumpkin_data::{BlockState, BlockStateId, translation};
use pumpkin_inventory::ViewerCountTracker;
use pumpkin_inventory::{
    generic_container_screen_handler::create_generic_9x3,
    player::ender_chest_inventory::EnderChestInventory,
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;

pub struct EnderChestScreenFactory {
    pub inventory: Arc<EnderChestInventory>,
    pub tracker: Option<Arc<ViewerCountTracker>>,
}

impl ScreenHandlerFactory for EnderChestScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        if let Some(tracker) = &self.tracker {
            self.inventory.set_tracker(tracker.clone());
        }
        let handler = create_generic_9x3(sync_id, player_inventory, self.inventory.clone(), player);
        let concrete_arc = Arc::new(Mutex::new(handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_ENDERCHEST,
            translation::bedrock::CONTAINER_ENDERCHEST
        )
    }
}

#[pumpkin_block("minecraft:ender_chest")]
pub struct EnderChestBlock;

impl BlockBehaviour for EnderChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LadderLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.waterlogged = args
            .world
            .get_fluid_and_fluid_state(args.position)
            .0
            .matches_type(&pumpkin_data::fluid::Fluid::WATER);
        props.to_state_id(args.block)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        // On the server, we don't need to do more because the client is responsible for that.
        args.r#type == Self::LID_ANIMATION_EVENT_TYPE
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
                pumpkin_data::statistic::CustomStatistic::OpenEnderchest as i32,
                1,
            );

            // TODO: PiglinBrain.onGuardedBlockInteracted(serverWorld, player, true);
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        if is_chest_blocked(args.world, args.position) {
            return None;
        }

        let block_entity = args.world.get_block_entity(args.position)?;

        let block_entity = block_entity
            .as_any()
            .downcast_ref::<EnderChestBlockEntity>()?;

        let tracker = block_entity.get_tracker();
        let inventory = args.player.ender_chest_inventory();
        Some(Box::new(EnderChestScreenFactory {
            inventory: inventory.clone(),
            tracker: Some(tracker),
        }))
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if LadderLikeProperties::from_state_id(args.state_id).waterlogged {
            args.world.schedule_fluid_tick(
                &pumpkin_data::fluid::Fluid::WATER,
                *args.position,
                5,
                pumpkin_world::tick::TickPriority::Normal,
            );
        }
        args.state_id
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn is_chest_blocked(world: &World, block_pos: &BlockPos) -> bool {
    // Unlike ordinary chests, ender chests do not check for sitting cats.
    has_block_on_top(world, block_pos)
}
fn has_block_on_top(world: &World, block_pos: &BlockPos) -> bool {
    let above_pos = block_pos.up();
    let above_state = world.get_block_state(&above_pos);
    above_state.is_solid_block()
}
impl EnderChestBlock {
    pub const LID_ANIMATION_EVENT_TYPE: u8 = 1;
}
