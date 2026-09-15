use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::block::entities::lectern::LecternBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetComparatorOutputArgs, GetRedstonePowerArgs,
    GetScreenHandlerFactoryArgs, NormalUseArgs, OnPlaceArgs, OnScheduledTickArgs,
    OnStateReplacedArgs, PathComputationType, UseWithItemArgs,
};
use crate::entity::EntityBase;
use crate::world::World;
use pumpkin_data::block_properties::LecternLikeProperties;
use pumpkin_data::data_component_impl::{BlockEntityDataImpl, EquipmentSlot};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, tag, translation};
use pumpkin_inventory::Inventory;
use pumpkin_inventory::lectern_screen_handler::{LecternController, LecternScreenHandler};
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use pumpkin_util::{Hand, PermissionLvl};
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use std::sync::Mutex;

/// Bridges the screen handler back into the world: page changes emit the
/// vanilla redstone pulse and taking the book clears `has_book`.
struct LecternPageController {
    inventory: Arc<dyn Inventory>,
}

impl LecternPageController {
    fn entity(&self) -> Option<&LecternBlockEntity> {
        self.inventory.as_any().downcast_ref::<LecternBlockEntity>()
    }
}

impl LecternController for LecternPageController {
    fn current_page(&self) -> i32 {
        self.entity()
            .map_or(0, |entity| entity.page.load(Ordering::Relaxed) as i32)
    }

    fn set_page(&self, page: i32) {
        if let Some(entity) = self.entity() {
            entity.set_page(page);
        }
    }

    fn on_book_taken(&self) {}
}

struct LecternScreenFactory {
    inventory: Arc<dyn Inventory>,
    controller: Arc<dyn LecternController>,
}

impl ScreenHandlerFactory for LecternScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        _player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler =
            LecternScreenHandler::new(sync_id, self.inventory.clone(), self.controller.clone());
        Some(Arc::new(Mutex::new(handler)) as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_LECTERN,
            translation::bedrock::TILE_LECTERN_NAME
        )
    }
}

#[pumpkin_block("minecraft:lectern")]
pub struct LecternBlock;

impl LecternBlock {
    /// Vanilla pulse length of a page-turn signal, in game ticks.
    const PAGE_TURN_PULSE_TICKS: u32 = 2;

    /// The lectern strongly powers the block below it, so its neighbors need
    /// updating whenever the power or book state changes.
    fn update_neighbors_below(world: &Arc<World>, position: &BlockPos) {
        world.update_neighbors_at(&position.down(), &Block::LECTERN, None);
    }

    /// Emits the vanilla page-turn redstone pulse: powered for two game ticks.
    pub(crate) fn pulse(world: &Arc<World>, position: &BlockPos) {
        let (block, state_id) = world.get_block_and_state_id(position);
        if block != &Block::LECTERN {
            return;
        }
        let mut props = LecternLikeProperties::from_state_id(state_id);
        props.powered = true;
        world.set_block_state(position, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        Self::update_neighbors_below(world, position);
        world.schedule_block_tick(
            block,
            *position,
            Self::PAGE_TURN_PULSE_TICKS,
            TickPriority::Normal,
        );
        world.sync_world_event(WorldEvent::SoundPageTurn, *position, 0);
    }

    /// Sets `has_book`, dropping any pending pulse like vanilla `setHasBook`.
    pub(crate) fn set_has_book(
        world: &Arc<World>,
        position: &BlockPos,
        has_book: bool,
        source: Option<&dyn EntityBase>,
    ) {
        let (block, state_id) = world.get_block_and_state_id(position);
        if block != &Block::LECTERN {
            return;
        }
        let mut props = LecternLikeProperties::from_state_id(state_id);
        props.powered = false;
        props.has_book = has_book;
        let state = props.to_state_id(block);
        world.set_block_state(position, state, BlockFlags::NOTIFY_ALL);
        world.emit_game_event_from_entity(
            "block_change",
            position.to_centered_f64(),
            source,
            Some(state),
        );
        Self::update_neighbors_below(world, position);
    }
}

impl BlockBehaviour for LecternBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LecternLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        if args.player.has_infinite_materials()
            && args.player.permission_lvl.load() >= PermissionLvl::Two
            && let Ok(hand) = Hand::from_packet_id(args.use_item_on.hand.0)
        {
            let stack = args.player.inventory().get_stack_in_hand(hand);
            props.has_book = stack
                .get_data_component::<BlockEntityDataImpl>()
                .is_some_and(|data| data.nbt.child_tags.contains_key("Book"));
        }
        props.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if !LecternLikeProperties::from_state_id(args.world.get_block_state_id(args.position))
            .has_book
        {
            return BlockActionResult::Consume;
        }
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
                pumpkin_data::statistic::CustomStatistic::InteractWithLectern as i32,
                1,
            );
        }
        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let props =
            LecternLikeProperties::from_state_id(args.world.get_block_state(args.position).id);
        if !props.has_book {
            return None;
        }

        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;

        let controller = Arc::new(LecternPageController {
            inventory: inventory.clone(),
        });

        Some(Box::new(LecternScreenFactory {
            inventory,
            controller,
        }))
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let props =
            LecternLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if props.has_book {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if args
            .item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_LECTERN_BOOKS)
        {
            if let Some(entity) = args.world.get_block_entity(args.position)
                && let Some(lectern) = entity.as_any().downcast_ref::<LecternBlockEntity>()
            {
                let book = args
                    .item_stack
                    .split_unless_creative(args.player.gamemode.load(), 1);
                lectern.set_book(book);
                Self::set_has_book(args.world, args.position, true, Some(args.player.as_ref()));
                args.world.play_block_sound(
                    Sound::ItemBookPut,
                    SoundCategory::Blocks,
                    *args.position,
                );
            }
            BlockActionResult::Success
        } else if args.item_stack.is_empty() && *args.equipment_slot == EquipmentSlot::MAIN_HAND {
            BlockActionResult::Pass
        } else {
            BlockActionResult::PassToDefaultBlockAction
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let mut props =
            LecternLikeProperties::from_state_id(args.world.get_block_state(args.position).id);
        props.powered = false;
        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
        Self::update_neighbors_below(args.world, args.position);
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LecternLikeProperties::from_state_id(args.state.id);
        if props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LecternLikeProperties::from_state_id(args.state.id);
        if props.powered && args.direction == BlockDirection::Up {
            15
        } else {
            0
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if LecternLikeProperties::from_state_id(args.old_state_id).powered {
            Self::update_neighbors_below(args.world, args.position);
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if !LecternLikeProperties::from_state_id(args.world.get_block_state_id(args.position))
            .has_book
        {
            return Some(0);
        }
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(lectern_entity) = block_entity.as_any().downcast_ref::<LecternBlockEntity>()
        {
            Some(lectern_entity.comparator_output())
        } else {
            Some(0)
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
