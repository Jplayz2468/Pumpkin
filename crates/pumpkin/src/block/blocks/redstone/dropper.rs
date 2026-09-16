use std::sync::Arc;
use std::sync::Mutex;

use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, NormalUseArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, OnStateReplacedArgs,
};
use crate::entity::EntityBase;

use crate::block::entities::dropper::DropperBlockEntity;
use crate::block::entities::hopper::HopperBlockEntity;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::Facing;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{FacingExt, translation};
use pumpkin_inventory::Inventory;
use pumpkin_inventory::generic_container_screen_handler::create_generic_3x3;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

struct DropperScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for DropperScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler = create_generic_3x3(sync_id, player_inventory, self.0.clone(), player);
        let screen_handler_arc = Arc::new(Mutex::new(handler));

        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_DROPPER,
            translation::bedrock::CONTAINER_DROPPER
        )
    }
}

#[pumpkin_block("minecraft:dropper")]
pub struct DropperBlock;

type DispenserLikeProperties = pumpkin_data::block_properties::DispenserLikeProperties;

const fn to_data3d(facing: Facing) -> i32 {
    match facing {
        Facing::North => 2,
        Facing::East => 5,
        Facing::South => 3,
        Facing::West => 4,
        Facing::Up => 1,
        Facing::Down => 0,
    }
}

impl BlockBehaviour for DropperBlock {
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
                pumpkin_data::statistic::CustomStatistic::InspectDropper as i32,
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
        block_entity.as_any().downcast_ref::<DropperBlockEntity>()?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(DropperScreenFactory(inventory)))
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = DispenserLikeProperties::default(args.block);
        props.facing = args.player.get_entity().get_facing().opposite();
        props.to_state_id(args.block)
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.world.get_block(args.position) != args.block {
            return;
        }

        let powered = block_receives_redstone_power(args.world, args.position)
            || block_receives_redstone_power(args.world, &args.position.up());

        let mut props =
            DispenserLikeProperties::from_state_id(args.world.get_block_state(args.position).id);

        if powered && !props.triggered {
            args.world
                .schedule_block_tick(args.block, *args.position, 4, TickPriority::Normal);
            props.triggered = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        } else if !powered && props.triggered {
            props.triggered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (_block, state) = args.world.get_block_and_state(args.position);
        if let Some(block_entity) = args.world.get_block_entity(args.position) {
            let Some(dropper) = block_entity.as_any().downcast_ref::<DropperBlockEntity>() else {
                return;
            };

            if let Some((slot_index, mut item)) = dropper.get_random_slot(args.world) {
                let props = DispenserLikeProperties::from_state_id(state.id);

                let target_pos = args
                    .position
                    .offset(props.facing.to_block_direction().to_offset());

                if let Some(container) = HopperBlockEntity::container_at(args.world, &target_pos) {
                    let backup = item.clone();
                    let one_item = item.split(1);

                    if HopperBlockEntity::add_one_item_from(
                        dropper,
                        container.as_ref(),
                        &one_item,
                        Some(props.facing.to_block_direction().opposite()),
                    ) {
                        dropper.set_stack(slot_index, item);
                        return;
                    }

                    dropper.set_stack(slot_index, backup);
                    return;
                }

                // No container found, dispense item into the world
                let drop_item = item.split(1);
                dropper.set_stack(slot_index, item);
                super::dispenser::spawn_default_item(
                    args.world,
                    args.position,
                    props.facing.to_block_direction(),
                    drop_item,
                );

                args.world
                    .sync_world_event(WorldEvent::SoundDispenserDispense, *args.position, 0);

                args.world.sync_world_event(
                    WorldEvent::ParticlesShootSmoke,
                    *args.position,
                    to_data3d(props.facing),
                );
            } else {
                args.world
                    .sync_world_event(WorldEvent::SoundDispenserFail, *args.position, 0);
            }
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}
