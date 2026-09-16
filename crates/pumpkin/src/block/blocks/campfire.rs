use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId, Enchantment,
    block_properties::CampfireLikeProperties,
    damage::DamageType,
    data_component_impl::EquipmentSlot,
    effect::StatusEffect,
    fluid::Fluid,
    recipes::{CookingRecipeKind, get_cooking_recipe_with_ingredient},
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_world::tick::TickPriority;

use crate::block::entities::campfire::CampfireBlockEntity;
use crate::{
    block::{
        BlockBehaviour, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs, OnPlaceArgs,
        OnProjectileHitArgs, PathComputationType, UseWithItemArgs, registry::BlockActionResult,
    },
    entity::EntityBase,
};

#[pumpkin_block_from_tag("minecraft:campfires")]
pub struct CampfireBlock;

impl BlockBehaviour for CampfireBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let Some(recipe) = get_cooking_recipe_with_ingredient(
            args.item_stack.item,
            CookingRecipeKind::CampfireCooking,
        ) else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::PassToDefaultBlockAction;
        };
        let Some(campfire) = block_entity.as_any().downcast_ref::<CampfireBlockEntity>() else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        for slot in 0..CampfireBlockEntity::SLOT_COUNT {
            let stored = campfire.items[slot]
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !stored.is_empty() {
                continue;
            }

            drop(stored);
            let mut cooking_time = recipe.cookingtime;
            if let Some(server) = args.world.server.upgrade() {
                let mut event =
                    crate::plugin::api::events::block::campfire_start::CampfireStartEvent::new(
                        *args.position,
                        args.world.clone(),
                        args.item_stack.clone(),
                        slot as u8,
                        cooking_time,
                    );
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return BlockActionResult::Consume;
                }
                cooking_time = event.cooking_time;
            }
            // Plugin callbacks run without holding the slot lock.
            let mut stored = campfire.items[slot].lock().unwrap();
            if !stored.is_empty() {
                continue;
            }
            *campfire.cooking_total_times[slot].lock().unwrap() = cooking_time;
            *campfire.cooking_times[slot].lock().unwrap() = 0;
            *stored = args
                .item_stack
                .split_unless_creative(args.player.gamemode.load(), 1);
            drop(stored);
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                Some(args.player.as_ref()),
                Some(state.id),
            );
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::InteractWithCampfire as i32,
                1,
            );
            args.world.update_block_entity(&block_entity);
            return BlockActionResult::Success;
        }

        BlockActionResult::Consume
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if CampfireLikeProperties::from_state_id(args.state.id).lit
            && let Some(living_entity) = args.entity.get_living_entity()
        {
            let has_frost_walker_enchantment = {
                let equipment = living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equipment
                    .equipment
                    .get(&EquipmentSlot::FEET)
                    .is_some_and(|boots| {
                        boots.get_enchantment_level(&Enchantment::FROST_WALKER) != 0
                    })
            };
            let has_fire_res = living_entity
                .get_effect(&StatusEffect::FIRE_RESISTANCE)
                .is_some();
            if has_frost_walker_enchantment || has_fire_res {
                // Campfire damage is prevented by Frost Walker boots or fire resistance.
                return;
            }
            let damage_amount = if args.block == &Block::SOUL_CAMPFIRE {
                2.0
            } else {
                1.0
            };
            args.entity
                .damage(args.entity, damage_amount, DamageType::CAMPFIRE);
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let (fluid, fluid_state) = crate::world::World::fluid_state_from_block_state(
            args.world.get_block_state_id(args.position),
        );
        let is_replacing_water = fluid.matches_type(&Fluid::WATER) && fluid_state.is_source;
        let mut props = CampfireLikeProperties::from_state_id(args.block.default_state.id);
        props.waterlogged = is_replacing_water;
        props.signal_fire = is_signal_fire_base_block(args.world.get_block(&args.position.down()));
        props.lit = !is_replacing_water;
        props.facing = args.player.get_entity().get_horizontal_facing();
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = CampfireLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }

        if args.direction == BlockDirection::Down {
            props.signal_fire =
                is_signal_fire_base_block(args.world.get_block(args.neighbor_position));
        }

        props.to_state_id(args.block)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        let mut props = CampfireLikeProperties::from_state_id(args.state.id);
        if args.projectile.get_entity().is_on_fire()
            && crate::entity::projectile::may_interact(args.projectile, args.world, args.position)
            && !props.lit
            && !props.waterlogged
        {
            props.lit = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                pumpkin_world::world::BlockFlags::NOTIFY_ALL,
            );
        }
    }
}

fn is_signal_fire_base_block(block: &Block) -> bool {
    block == &Block::HAY_BLOCK
}
