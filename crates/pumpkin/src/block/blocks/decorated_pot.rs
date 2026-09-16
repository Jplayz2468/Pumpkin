use pumpkin_data::{
    BlockState, BlockStateId,
    block_properties::DecoratedPotLikeProperties,
    data_component_impl::EnchantmentsImpl,
    fluid::Fluid,
    particle::Particle,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

use crate::{
    block::{
        BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, GetStateForNeighborUpdateArgs,
        NormalUseArgs, OnPlaceArgs, OnProjectileHitArgs, OnStateReplacedArgs,
        OnSyncedBlockEventArgs, PathComputationType, UseWithItemArgs,
        entities::decorated_pot::DecoratedPotBlockEntity, registry::BlockActionResult,
    },
    world::World,
};

#[pumpkin_block("minecraft:decorated_pot")]
pub struct DecoratedPotBlock;

impl BlockBehaviour for DecoratedPotBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = DecoratedPotLikeProperties::from_state_id(args.block.default_state.id);
        props.facing = args.player.living_entity.entity.get_horizontal_facing();
        let (fluid, state) =
            World::fluid_state_from_block_state(args.world.get_block_state_id(args.position));
        props.waterlogged = fluid.matches_type(&Fluid::WATER) && state.is_source;
        props.cracked = false;
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if DecoratedPotLikeProperties::from_state_id(args.state_id).waterlogged {
            args.world
                .schedule_fluid_tick(&Fluid::WATER, *args.position, 5, TickPriority::Normal);
        }
        args.state_id
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(pot) = entity.as_any().downcast_ref::<DecoratedPotBlockEntity>() else {
            return BlockActionResult::Pass;
        };
        let stored = pot.get_item();
        if args.item_stack.is_empty()
            || stored.as_ref().is_some_and(|item| {
                !item.is_empty()
                    && (!item.are_items_and_components_equal(args.item_stack)
                        || item.item_count >= item.get_max_stack_size())
            })
        {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        pot.wobble(true);
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            i32::from(args.item_stack.item.id),
            1,
        );
        let mut item = args.item_stack.clone();
        item.item_count = 1;
        if !pot.try_insert_item(&mut item, 1) {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if !args.player.has_infinite_materials() {
            args.item_stack.decrement(1);
        }
        let item = pot.get_item().expect("inserted pot item");
        let fill = f32::from(item.item_count) / f32::from(item.get_max_stack_size());
        args.world.play_sound_fine(
            Sound::BlockDecoratedPotInsert,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            1.0,
            0.7 + 0.5 * fill,
        );
        args.world.spawn_particles(
            Particle::DustPlume,
            args.position.to_f64() + Vector3::new(0.5, 1.2, 0.5),
            7,
            Vector3::new(0.0, 0.0, 0.0),
            0.0,
        );
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            None,
        );
        BlockActionResult::Success
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let Some(entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(pot) = entity.as_any().downcast_ref::<DecoratedPotBlockEntity>() else {
            return BlockActionResult::Pass;
        };
        args.world.play_sound(
            Sound::BlockDecoratedPotInsertFail,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
        );
        pot.wobble(false);
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            None,
        );
        BlockActionResult::Success
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == 1
            && args.data < 2
            && args
                .world
                .get_block_entity(args.position)
                .is_some_and(|entity| entity.as_any().is::<DecoratedPotBlockEntity>())
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        let tool = args.player.inventory().held_item();
        let prevents_shattering =
            tool.get_data_component::<EnchantmentsImpl>()
                .is_some_and(|data| {
                    data.enchantment.iter().any(|(enchantment, _)| {
                        enchantment
                            .has_tag(&tag::Enchantment::MINECRAFT_PREVENTS_DECORATED_POT_SHATTERING)
                    })
                });
        if tool
            .item
            .has_tag(&tag::Item::MINECRAFT_BREAKS_DECORATED_POTS)
            && !prevents_shattering
        {
            let mut props = DecoratedPotLikeProperties::from_state_id(args.state.id);
            props.cracked = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if crate::entity::projectile::may_interact(args.projectile, args.world, args.position)
            && crate::entity::projectile::may_break(args.projectile, args.world)
        {
            let mut props = DecoratedPotLikeProperties::from_state_id(args.state.id);
            props.cracked = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK,
            );
            args.world.break_block_from_entity(
                args.position,
                args.projectile,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        Some(
            args.world
                .get_block_entity(args.position)
                .and_then(|entity| {
                    entity
                        .as_any()
                        .downcast_ref::<DecoratedPotBlockEntity>()
                        .map(DecoratedPotBlockEntity::get_comparator_output)
                })
                .unwrap_or(0),
        )
    }
    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
