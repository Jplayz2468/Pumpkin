use std::sync::Arc;

use crate::block::entities::BlockEntity;
use crate::block::entities::chest::ChestBlockEntity;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{ChestLikeProperties, ChestType, HorizontalFacing};
use pumpkin_data::{Block, BlockDirection, translation};
use pumpkin_inventory::Inventory;
use pumpkin_inventory::double::DoubleInventory;
use pumpkin_inventory::generic_container_screen_handler::{create_generic_9x3, create_generic_9x6};
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::{pumpkin_block, pumpkin_block_from_tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use std::sync::Mutex;

use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetComparatorOutputArgs, GetRedstonePowerArgs,
    GetScreenHandlerFactoryArgs, NormalUseArgs, OnPlaceArgs, OnStateReplacedArgs,
    OnSyncedBlockEventArgs, PathComputationType, RandomTickArgs, registry::BlockActionResult,
};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::world::World;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockState, HorizontalFacingExt};

struct ChestScreenFactory(Arc<dyn Inventory>, Vec<Arc<dyn BlockEntity>>);

impl ScreenHandlerFactory for ChestScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        if player.is_spectator() && self.1.iter().any(|chest| chest.has_loot_table()) {
            return None;
        }
        let opener = player.as_any().downcast_ref::<Player>();
        for chest in &self.1 {
            unpack_chest(chest, opener);
        }
        let concrete_handler = if self.0.size() > 27 {
            create_generic_9x6(sync_id, player_inventory, self.0.clone(), player)
        } else {
            create_generic_9x3(sync_id, player_inventory, self.0.clone(), player)
        };

        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        for chest in &self.1 {
            if let Some(name) = chest_custom_name(chest) {
                return name;
            }
        }
        if self.0.size() > 27 {
            pumpkin_macros::translate_cross!(
                translation::java::CONTAINER_CHESTDOUBLE,
                translation::bedrock::CONTAINER_CHESTDOUBLE
            )
        } else {
            pumpkin_macros::translate_cross!(
                translation::java::CONTAINER_CHEST,
                translation::bedrock::CONTAINER_CHEST
            )
        }
    }
}

// Shared chest behavior implementations
const LID_ANIMATION_EVENT_TYPE: u8 = 1;

fn on_place_chest_impl(args: &OnPlaceArgs<'_>) -> BlockStateId {
    let mut chest_props = ChestLikeProperties::default(args.block);
    chest_props.waterlogged = args
        .world
        .get_fluid_and_fluid_state(args.position)
        .0
        .matches_type(&pumpkin_data::Fluid::WATER);

    let (r#type, facing) = compute_chest_props(
        args.world,
        args.player,
        args.block,
        args.position,
        args.direction,
    );
    chest_props.facing = facing;
    chest_props.r#type = r#type;

    chest_props.to_state_id(args.block)
}

fn unpack_chest(entity: &Arc<dyn BlockEntity>, player: Option<&Player>) {
    if let Some(chest) = entity.as_any().downcast_ref::<ChestBlockEntity>() {
        chest.unpack_loot(player);
    } else if let Some(chest) = entity
        .as_any()
        .downcast_ref::<crate::block::entities::trapped_chest::TrappedChestBlockEntity>(
    ) {
        chest.unpack_loot(player);
    }
}

fn chest_custom_name(entity: &Arc<dyn BlockEntity>) -> Option<TextComponent> {
    if let Some(chest) = entity.as_any().downcast_ref::<ChestBlockEntity>() {
        chest.custom_name()
    } else {
        entity
            .as_any()
            .downcast_ref::<crate::block::entities::trapped_chest::TrappedChestBlockEntity>()?
            .custom_name()
    }
}

/// DoubleBlockCombiner checks block identity, opposite halves, facing and entity type.
fn chest_entities(
    world: &World,
    position: &BlockPos,
    ignore_blocked: bool,
) -> Option<Vec<Arc<dyn BlockEntity>>> {
    let block = world.get_block(position);
    let first = world.get_block_entity(position)?;
    let valid = |entity: &Arc<dyn BlockEntity>| {
        if block == &Block::TRAPPED_CHEST {
            entity
                .as_any()
                .is::<crate::block::entities::trapped_chest::TrappedChestBlockEntity>()
        } else {
            entity.as_any().is::<ChestBlockEntity>()
        }
    };
    if !valid(&first) || (!ignore_blocked && is_chest_blocked(world, position)) {
        return None;
    }
    let props = ChestLikeProperties::from_state_id(world.get_block_state_id(position));
    if props.r#type != ChestType::Single {
        let neighbor_pos = position.offset(connected_direction(props).to_offset());
        let (neighbor, state) = world.get_block_and_state_id(&neighbor_pos);
        if neighbor == block {
            let other = ChestLikeProperties::from_state_id(state);
            if other.r#type == props.r#type.opposite() && other.facing == props.facing {
                if !ignore_blocked && is_chest_blocked(world, &neighbor_pos) {
                    return None;
                }
                if let Some(second) = world.get_block_entity(&neighbor_pos).filter(valid) {
                    return Some(if props.r#type == ChestType::Right {
                        vec![first, second]
                    } else {
                        vec![second, first]
                    });
                }
            }
        }
    }
    Some(vec![first])
}

fn combine_chest_inventories(entities: &[Arc<dyn BlockEntity>]) -> Option<Arc<dyn Inventory>> {
    let first = entities.first()?.clone().get_inventory()?;
    if let Some(second) = entities.get(1) {
        Some(DoubleInventory::new(first, second.clone().get_inventory()?))
    } else {
        Some(first)
    }
}

fn get_chest_comparator_output(args: &GetComparatorOutputArgs<'_>) -> Option<u8> {
    Some(
        chest_inventory(args.world, args.position, false).map_or(0, |inventory| {
            crate::block::calculate_comparator_output(inventory.as_ref())
        }),
    )
}

fn get_chest_screen_handler_factory(
    args: GetScreenHandlerFactoryArgs<'_>,
) -> Option<Box<dyn ScreenHandlerFactory>> {
    let entities = chest_entities(args.world, args.position, false)?;
    let inventory = combine_chest_inventories(&entities)?;
    Some(Box::new(ChestScreenFactory(inventory, entities)))
}

/// Automation ignores obstructed lids; loot resolves on actual inventory access.
pub(crate) fn chest_inventory(
    world: &World,
    position: &BlockPos,
    ignore_blocked: bool,
) -> Option<Arc<dyn Inventory>> {
    combine_chest_inventories(&chest_entities(world, position, ignore_blocked)?)
}

fn normal_use_chest_impl(args: &NormalUseArgs<'_>) -> BlockActionResult {
    let stat = if args.block.id == Block::TRAPPED_CHEST.id {
        pumpkin_data::statistic::CustomStatistic::TriggerTrappedChest
    } else {
        pumpkin_data::statistic::CustomStatistic::OpenChest
    };

    if let Some(factory) = get_chest_screen_handler_factory(GetScreenHandlerFactoryArgs {
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
            stat as i32,
            1,
        );
    }

    BlockActionResult::Success
}

#[pumpkin_block_from_tag("c:chests/wooden")]
pub struct ChestBlock;

impl BlockBehaviour for ChestBlock {
    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        chest_neighbor_state(args)
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        on_place_chest_impl(&args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

/// Copper chests have the same behavior as wooden chests but also oxidize over time.
#[pumpkin_block_from_tag("minecraft:copper_chests")]
pub struct CopperChestBlock;

impl
    crate::block::blocks::weathering_copper::ChangeOverTimeBlock<
        crate::block::blocks::weathering_copper::WeatherState,
    > for CopperChestBlock
{
    fn get_age(
        &self,
        block: &Block,
    ) -> Option<crate::block::blocks::weathering_copper::WeatherState> {
        crate::block::blocks::weathering_copper::get_weather_state(block)
    }

    fn get_chance_modifier(
        &self,
        age: crate::block::blocks::weathering_copper::WeatherState,
    ) -> f32 {
        crate::block::blocks::weathering_copper::get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_first(block)
    }
}

impl crate::block::blocks::weathering_copper::WeatheringCopper for CopperChestBlock {}

impl BlockBehaviour for CopperChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let state = on_place_chest_impl(&args);
        let props = ChestLikeProperties::from_state_id(state);
        if props.r#type == ChestType::Single {
            return state;
        }
        let neighbor_pos = args.position.offset(connected_direction(props).to_offset());
        let neighbor = args.world.get_block(&neighbor_pos);
        if !chests_can_connect(args.block, neighbor) {
            return state;
        }
        // CopperChestBlock.java:75-94: mismatched wax is removed, then the less
        // oxidized of the two stages determines both halves through updateShape.
        let this = unwaxed_copper_chest(args.block);
        let other = unwaxed_copper_chest(neighbor);
        let wax_mismatch =
            args.block.name.starts_with("waxed_") != neighbor.name.starts_with("waxed_");
        let age =
            |block| super::weathering_copper::get_weather_state(block).map_or(0, |a| a.ordinal());
        let target = if age(this) <= age(other) {
            if wax_mismatch { this } else { args.block }
        } else if wax_mismatch {
            other
        } else {
            neighbor
        };
        props.to_state_id(target)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        chest_neighbor_state(args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        let current_state_id = args.world.get_block_state_id(args.position);
        let chest_props = ChestLikeProperties::from_state_id(current_state_id);

        // Only oxidize LEFT or SINGLE chests (not RIGHT) to prevent double oxidation
        if chest_props.r#type == ChestType::Right {
            return;
        }

        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return;
        };
        let Some(chest_entity) = block_entity.as_any().downcast_ref::<ChestBlockEntity>() else {
            return;
        };
        if chest_entity.get_viewer_count() > 0 {
            return;
        }

        crate::block::blocks::weathering_copper::change_over_time(
            args.world,
            args.position,
            args.block,
            &mut args.random,
        );
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

/// Trapped chests have the same behavior as wooden chests but also emit redstone power based on viewer count.
#[pumpkin_block("minecraft:trapped_chest")]
pub struct TrappedChestBlock;

impl BlockBehaviour for TrappedChestBlock {
    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        chest_neighbor_state(args)
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        on_place_chest_impl(&args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        use crate::block::entities::trapped_chest::TrappedChestBlockEntity;

        // Get viewer count from this chest
        let viewer_count = if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(trapped_chest) = block_entity
                .as_any()
                .downcast_ref::<TrappedChestBlockEntity>()
        {
            trapped_chest.get_viewer_count()
        } else {
            0
        };

        viewer_count.min(15) as u8
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        // Strong power emitted to the block beneath the trapped chest
        // The block below queries with direction Up (from below looking up at the chest)
        if args.direction == BlockDirection::Up {
            self.get_weak_redstone_power(args)
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn compute_chest_props(
    world: &World,
    player: &Player,
    block: &Block,
    block_pos: &BlockPos,
    face: BlockDirection,
) -> (ChestType, HorizontalFacing) {
    let player_facing = player.get_entity().get_horizontal_facing();
    let chest_facing = player_facing.opposite();

    if player.get_entity().is_sneaking() {
        let Some(face) = face.to_horizontal_facing() else {
            return (ChestType::Single, chest_facing);
        };

        let (clicked_block, clicked_block_state) =
            world.get_block_and_state_id(&block_pos.offset(face.to_offset()));

        if chests_can_connect(block, clicked_block) {
            let clicked_props = ChestLikeProperties::from_state_id(clicked_block_state);

            if clicked_props.r#type != ChestType::Single {
                return (ChestType::Single, chest_facing);
            }

            if clicked_props.facing.rotate_clockwise() == face {
                return (ChestType::Left, clicked_props.facing);
            } else if clicked_props.facing.rotate_counter_clockwise() == face {
                return (ChestType::Right, clicked_props.facing);
            }
        }

        return (ChestType::Single, chest_facing);
    }

    if get_chest_properties_if_can_connect(
        world,
        block,
        block_pos,
        chest_facing,
        chest_facing.rotate_clockwise(),
        ChestType::Single,
    )
    .is_some()
    {
        (ChestType::Left, chest_facing)
    } else if get_chest_properties_if_can_connect(
        world,
        block,
        block_pos,
        chest_facing,
        chest_facing.rotate_counter_clockwise(),
        ChestType::Single,
    )
    .is_some()
    {
        (ChestType::Right, chest_facing)
    } else {
        (ChestType::Single, chest_facing)
    }
}

fn unwaxed_copper_chest(block: &Block) -> &Block {
    Block::from_name(block.name.strip_prefix("waxed_").unwrap_or(block.name)).unwrap_or(block)
}

fn chests_can_connect(block: &Block, neighbor: &Block) -> bool {
    block == neighbor
        || (block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS)
            && neighbor.has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS))
}

fn connected_direction(props: ChestLikeProperties) -> HorizontalFacing {
    if props.r#type == ChestType::Left {
        props.facing.rotate_clockwise()
    } else {
        props.facing.rotate_counter_clockwise()
    }
}

/// ChestBlock.updateShape plus CopperChestBlock.updateShape. Copy the neighbor's
/// copper stage into this half without changing this half's facing/type/water.
fn chest_neighbor_state(args: crate::block::GetStateForNeighborUpdateArgs<'_>) -> BlockStateId {
    let mut props = ChestLikeProperties::from_state_id(args.state_id);
    super::schedule_waterlogged_tick(args.world, args.position, props.waterlogged);
    let neighbor = Block::from_state_id(args.neighbor_state_id);
    let mut result_block = args.block;
    if chests_can_connect(args.block, neighbor) && args.direction.to_horizontal_facing().is_some() {
        let other = ChestLikeProperties::from_state_id(args.neighbor_state_id);
        if props.r#type == ChestType::Single
            && other.r#type != ChestType::Single
            && props.facing == other.facing
            && connected_direction(other).to_block_direction() == args.direction.opposite()
        {
            props.r#type = other.r#type.opposite();
        }
        if args
            .block
            .has_tag(&pumpkin_data::tag::Block::MINECRAFT_COPPER_CHESTS)
            && props.r#type != ChestType::Single
            && connected_direction(props).to_block_direction() == args.direction
        {
            result_block = neighbor;
        }
    } else if connected_direction(props).to_block_direction() == args.direction {
        props.r#type = ChestType::Single;
    }
    props.to_state_id(result_block)
}

fn get_chest_properties_if_can_connect(
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
    facing: HorizontalFacing,
    direction: HorizontalFacing,
    wanted_type: ChestType,
) -> Option<ChestLikeProperties> {
    let (neighbor_block, neighbor_block_state) =
        world.get_block_and_state_id(&block_pos.offset(direction.to_offset()));

    if !chests_can_connect(block, neighbor_block) {
        return None;
    }

    let neighbor_props = ChestLikeProperties::from_state_id(neighbor_block_state);
    if neighbor_props.facing == facing && neighbor_props.r#type == wanted_type {
        return Some(neighbor_props);
    }

    None
}

fn is_chest_blocked(world: &World, block_pos: &BlockPos) -> bool {
    has_block_on_top(world, block_pos)
        || world
            .get_entities_at_box(
                &pumpkin_util::math::boundingbox::BoundingBox::new_array(
                    [0.0, 1.0, 0.0],
                    [1.0, 2.0, 1.0],
                )
                .at_pos(*block_pos),
            )
            .iter()
            .any(|entity| {
                !entity
                    .get_entity()
                    .removed
                    .load(std::sync::atomic::Ordering::Relaxed)
                    && entity.get_entity().entity_type.id
                        == pumpkin_data::entity::EntityType::CAT.id
                    && entity.get_mob().is_some_and(|mob| mob.is_sitting())
            })
}
fn has_block_on_top(world: &World, block_pos: &BlockPos) -> bool {
    let above_pos = block_pos.up();
    let above_state = world.get_block_state(&above_pos);
    above_state.is_solid_block()
}

trait ChestTypeExt {
    fn opposite(&self) -> ChestType;
}

impl ChestTypeExt for ChestType {
    fn opposite(&self) -> Self {
        match self {
            Self::Single => Self::Single,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}
