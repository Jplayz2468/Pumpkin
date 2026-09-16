use crate::block::entities::beehive::{BeeReleaseStatus, BeehiveBlockEntity};
use crate::block::{BlockBehaviour, BlockMetadata, GetComparatorOutputArgs};
use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::passive::bee::BeeEntity;
use crate::world::World;
use pumpkin_data::block_properties::{BeeNestLikeProperties, CampfireLikeProperties};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, BlockState};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::random::RandomImpl;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

pub struct BeehiveBlock;

impl BlockMetadata for BeehiveBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BEEHIVE, BlockId::BEE_NEST].into()
    }
}

impl BlockBehaviour for BeehiveBlock {
    fn on_place(&self, args: crate::block::OnPlaceArgs<'_>) -> pumpkin_data::BlockStateId {
        let mut props = BeeNestLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }
    fn placed(&self, args: crate::block::PlacedArgs<'_>) {
        if args.world.get_block_entity(args.position).is_none() {
            args.world
                .add_block_entity(Arc::new(BeehiveBlockEntity::new(*args.position)));
        }
    }
    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> pumpkin_data::BlockStateId {
        if args
            .world
            .get_block(&args.position.offset(args.direction.to_offset()))
            .id
            == BlockId::FIRE
        {
            if let Some(world) = args.world.as_arc() {
                release_bees(&world, args.position, args.state_id.to_state(), None);
            }
        }
        args.state_id
    }
    fn use_with_item(
        &self,
        args: crate::block::UseWithItemArgs<'_>,
    ) -> crate::block::registry::BlockActionResult {
        use crate::block::registry::BlockActionResult;
        use pumpkin_data::{item::Item, item_stack::ItemStack};
        if args.item_stack.item != &Item::GLASS_BOTTLE {
            return BlockActionResult::Pass;
        }
        let state = args.world.get_block_state(args.position);
        if BeeNestLikeProperties::from_state_id(state.id).honey_level < 5 {
            return BlockActionResult::Pass;
        }
        // BeehiveBlock.useItemOn consumes a bottle even with infinite materials.
        args.item_stack.decrement(1);
        let mut honey = ItemStack::new(1, &Item::HONEY_BOTTLE);
        if args.item_stack.is_empty() {
            *args.item_stack = honey;
        } else if !args.player.inventory().insert_stack_anywhere(&mut honey) && !honey.is_empty() {
            args.world
                .drop_stack(&args.player.living_entity.entity.block_pos.load(), honey);
        }
        args.world.play_sound(
            pumpkin_data::sound::Sound::ItemBottleFill,
            pumpkin_data::sound::SoundCategory::Blocks,
            &args.player.living_entity.entity.pos.load(),
        );
        args.world.emit_game_event_with_source(
            "fluid_pickup",
            args.position.to_centered_f64(),
            Some(args.player.living_entity.entity.entity_id),
        );
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            i32::from(Item::GLASS_BOTTLE.id),
            1,
        );
        finish_harvest(args.world, args.position, state, args.player);
        BlockActionResult::Success
    }

    fn prepare_explosion_drops(&self, args: crate::block::ExplodeArgs<'_>) {
        use pumpkin_data::entity::EntityType;
        // BeehiveBlock.getDrops releases occupants for these direct sources only.
        if args.source.is_some_and(|source| {
            let kind = source.get_entity().entity_type;
            [
                EntityType::TNT,
                EntityType::CREEPER,
                EntityType::WITHER_SKULL,
                EntityType::WITHER,
                EntityType::TNT_MINECART,
            ]
            .contains(kind)
        }) {
            release_bees(args.world, args.position, args.state, None);
        }
    }

    fn explode(&self, args: crate::block::ExplodeArgs<'_>) {
        // This also runs for TRIGGER_BLOCK, matching onExplosionHit's super call.
        anger_nearby_bees(args.world, args.position);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        {
            let state_id = args.world.get_block_state_id(args.position);
            let props = BeeNestLikeProperties::from_state_id(state_id);
            Some(props.honey_level)
        }
    }
}

/// CampfireBlock.isSmokeyPos uses the four-pixel-wide virtual smoke column.
pub fn is_smokey_pos(world: &World, pos: &BlockPos) -> bool {
    for i in 1..=5 {
        let check_pos = pos.down_height(i);
        let (block, state) = world.get_block_and_state(&check_pos);
        if is_lit_campfire(block, state) {
            return true;
        }
        let smoke_column = BoundingBox::new(
            Vector3::new(6.0 / 16.0, 0.0, 6.0 / 16.0),
            Vector3::new(10.0 / 16.0, 1.0, 10.0 / 16.0),
        );
        if state
            .get_block_collision_shapes_at(pos)
            .any(|shape| shape.intersects(&smoke_column))
        {
            let (below_block, below_state) = world.get_block_and_state(&check_pos.down());
            return is_lit_campfire(below_block, below_state);
        }
    }
    false
}

fn is_lit_campfire(block: &'static Block, state: &'static BlockState) -> bool {
    block.has_tag(&tag::Block::MINECRAFT_CAMPFIRES)
        && CampfireLikeProperties::from_state_id(state.id).lit
}

/// Mirrors `BeehiveBlock.angerNearbyBees` (BeehiveBlock.java:118-132): every `Bee` in a
/// 17x13x17 box centered on the hive (`AABB(pos).inflate(8.0, 6.0, 8.0)`) that has no current
/// target is set to attack a random player also found in that box.
pub fn anger_nearby_bees(world: &World, pos: &BlockPos) {
    let center = pos.to_centered_f64();
    let radius = Vector3::new(8.5, 6.5, 8.5);
    let aabb = BoundingBox::new(center - radius, center + radius);

    let bees: Vec<Arc<dyn EntityBase>> = world
        .get_entities_at_box(&aabb)
        .into_iter()
        .filter(|e| e.cast_any().downcast_ref::<BeeEntity>().is_some())
        .collect();
    if bees.is_empty() {
        return;
    }

    let players = world.get_players_at_box(&aabb);
    if players.is_empty() {
        return;
    }

    for bee_entity in &bees {
        let Some(bee) = bee_entity.cast_any().downcast_ref::<BeeEntity>() else {
            continue;
        };
        if bee.mob_entity.get_target().is_some() {
            continue;
        }
        let index = world
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_bounded_i32(players.len() as i32) as usize;
        let target = players[index].clone();
        bee.set_mob_target(Some(target as Arc<dyn EntityBase>));
    }
}

pub fn hive_contains_bees(world: &World, pos: &BlockPos) -> bool {
    world
        .get_block_entity(pos)
        .and_then(|block_entity| {
            let hive = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>()?;
            let guard = hive.bees.lock().ok()?;
            Some(!guard.is_empty())
        })
        .unwrap_or(false)
}

/// Reset honey and deliver the emergency release only when smoke does not sedate
/// the hive, shared by shears and bottle harvesting.
pub fn finish_harvest(
    world: &Arc<World>,
    pos: &BlockPos,
    state: &BlockState,
    player: &crate::entity::player::Player,
) {
    let smoked = is_smokey_pos(world, pos);
    if !smoked && hive_contains_bees(world, pos) {
        anger_nearby_bees(world, pos);
    }
    let mut props = BeeNestLikeProperties::from_state_id(state.id);
    props.honey_level = 0;
    world.set_block_state(
        pos,
        props.to_state_id(state.id.to_block()),
        BlockFlags::NOTIFY_ALL,
    );
    if !smoked {
        release_bees(world, pos, state, Some(player));
    }
}

fn release_bees(
    world: &Arc<World>,
    pos: &BlockPos,
    state: &BlockState,
    player: Option<&crate::entity::player::Player>,
) {
    if let Some(entity) = world.get_block_entity(pos)
        && let Some(hive) = entity.as_any().downcast_ref::<BeehiveBlockEntity>()
    {
        hive.empty_all(world, state, player, BeeReleaseStatus::Emergency);
    }
}

/// BeehiveBlock.playerWillDestroy preserves an occupied or honey-filled hive in
/// creative, even though ordinary creative block drops are suppressed.
pub(crate) fn drop_creative_hive(
    world: &Arc<World>,
    pos: &BlockPos,
    state: &BlockState,
    player: &crate::entity::player::Player,
    hive: &BeehiveBlockEntity,
) {
    use crate::block::entities::BlockEntity;
    if player.gamemode.load() != pumpkin_util::GameMode::Creative
        || !world.level_info.load().game_rules.block_drops
    {
        return;
    }
    let honey = BeeNestLikeProperties::from_state_id(state.id).honey_level;
    if honey == 0
        && hive
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    {
        return;
    }
    let Some(item) = pumpkin_data::item::Item::from_id(state.id.to_block().item_id) else {
        return;
    };
    let mut stack = pumpkin_data::item_stack::ItemStack::new(1, item);
    hive.collect_components(&mut stack);
    stack.set_data_component(pumpkin_data::data_component_impl::BlockStateImpl {
        properties: std::borrow::Cow::Owned(vec![("honey_level".into(), honey.to_string().into())]),
    });
    let entity = crate::entity::Entity::new(
        world.clone(),
        pos.to_f64(),
        &pumpkin_data::entity::EntityType::ITEM,
    );
    world.spawn_entity(Arc::new(crate::entity::item::ItemEntity::new(
        entity, stack,
    )));
}

pub(crate) fn player_destroyed(
    world: &Arc<World>,
    pos: &BlockPos,
    state: &BlockState,
    player: &crate::entity::player::Player,
    hive: &BeehiveBlockEntity,
) {
    if player.gamemode.load() == pumpkin_util::GameMode::Creative {
        return;
    }
    let tool = player.inventory().held_item();
    let protected = tool
        .get_data_component::<pumpkin_data::data_component_impl::EnchantmentsImpl>()
        .is_some_and(|enchants| {
            enchants.enchantment.iter().any(|(enchantment, level)| {
                *level > 0
                    && enchantment
                        .has_tag(&tag::Enchantment::MINECRAFT_PREVENTS_BEE_SPAWNS_WHEN_MINING)
            })
        });
    if !protected {
        hive.empty_all(world, state, Some(player), BeeReleaseStatus::Emergency);
        anger_nearby_bees(world, pos);
    }
    if state.id.to_block().id == BlockId::BEE_NEST
        && hive
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            == 3
        && pumpkin_data::Enchantment::from_name("silk_touch")
            .is_some_and(|e| tool.get_enchantment_level(e) > 0)
    {
        player.trigger_advancement_criterion(
            pumpkin_data::advancement::Advancement::HUSBANDRY_SILK_TOUCH_NEST,
            "silk_touch_nest",
        );
    }
}
