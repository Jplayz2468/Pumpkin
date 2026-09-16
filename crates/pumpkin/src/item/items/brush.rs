use crate::block::entities::brushable_block::BrushableBlockBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::entity::{EntityBase, player::Player};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::{
    Block, BlockDirection, BlockId,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::{
    Hand,
    math::{position::BlockPos, vector3::Vector3},
};
use std::any::Any;

pub struct BrushItem;
impl ItemMetadata for BrushItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::BRUSH.id])
    }
}

impl BrushItem {
    pub const USE_DURATION: i32 = 200;

    fn calculate_hit(player: &Player) -> Option<(BlockPos, BlockDirection, Vector3<f64>)> {
        let world = player.world();
        let start = player.eye_position();
        let end = start + player.get_looking_vector() * player.block_interaction_range();
        let (pos, result) = world.ray_trace_block_with_context(
            start,
            end,
            crate::world::RayFluidHandling::None,
            false,
            Some(player),
        )?;
        let hit = (pos, result.direction, result.position);
        let block_distance = (hit.2 - start).length();
        if world
            .ray_trace_entities(start, end)
            .iter()
            .any(|(entity, _, distance)| {
                entity.get_entity().entity_uuid != player.get_entity().entity_uuid
                    && !entity.is_spectator()
                    && entity.can_hit()
                    && *distance < block_distance
            })
        {
            return None;
        }
        Some(hit)
    }

    fn start(player: &Player, stack: &ItemStack, hand: Hand) {
        if Self::calculate_hit(player).is_some() {
            player
                .living_entity
                .set_active_hand(hand, stack.clone(), Self::USE_DURATION);
        }
    }
}

impl ItemBehaviour for BrushItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        face: BlockDirection,
        cursor_pos: Vector3<f32>,
        block: &Block,
        server: &Server,
    ) -> BlockActionResult {
        self.use_on_block_in_hand(
            item,
            player,
            Hand::Right,
            location,
            face,
            cursor_pos,
            block,
            server,
        )
    }

    fn use_on_block_in_hand(
        &self,
        item: &mut ItemStack,
        player: &Player,
        hand: Hand,
        _location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        _block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        Self::start(player, item, hand);
        BlockActionResult::Consume
    }

    fn on_use_tick(&self, stack: &ItemStack, player: &Player, remaining_use_ticks: i32) {
        let hand = *player
            .living_entity
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(hand) = hand else {
            return;
        };
        let held = player.inventory().get_stack_in_hand(hand);
        if remaining_use_ticks < 0 || !held.are_items_and_components_equal(stack) || held.is_empty()
        {
            player.living_entity.clear_active_hand();
            return;
        }
        let Some((pos, direction, _hit_location)) = Self::calculate_hit(player) else {
            player.living_entity.clear_active_hand();
            return;
        };
        if (Self::USE_DURATION - remaining_use_ticks + 1) % 10 != 5 {
            return;
        }
        let world = player.world();
        let block = world.get_block(&pos);
        // Client animation emits the dust particles. The server still advances
        // its random stream for those calls, as BrushItem.onUseTick does in Java.
        if !matches!(
            block.id,
            BlockId::AIR
                | BlockId::CAVE_AIR
                | BlockId::VOID_AIR
                | BlockId::BARRIER
                | BlockId::LIGHT
                | BlockId::STRUCTURE_VOID
                | BlockId::END_GATEWAY
                | BlockId::END_PORTAL
                | BlockId::BUBBLE_COLUMN
                | BlockId::WATER
                | BlockId::LAVA
                | BlockId::MOVING_PISTON
        ) {
            let particles = world.rand_bounded_i32(5) + 7;
            for _ in 0..particles {
                world.rand_f64();
                world.rand_f64();
            }
        }
        let sound = match block.id {
            BlockId::SUSPICIOUS_SAND => Sound::ItemBrushBrushingSand,
            BlockId::SUSPICIOUS_GRAVEL => Sound::ItemBrushBrushingGravel,
            _ => Sound::ItemBrushBrushingGeneric,
        };
        world.play_sound(sound, SoundCategory::Blocks, &pos.to_centered_f64());
        let Some(entity) = world.get_block_entity(&pos) else {
            return;
        };
        let Some(brushable) = entity.as_any().downcast_ref::<BrushableBlockBlockEntity>() else {
            return;
        };
        if let Some(server) = world.server.upgrade()
            && let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
        {
            let mut event = crate::plugin::api::events::block::block_brush::BlockBrushEvent::new(
                pos,
                world.clone(),
                player_arc,
                held.clone(),
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        if brushable.brush(&world, player, direction, &held) {
            let slot = if hand == Hand::Left {
                pumpkin_data::data_component_impl::EquipmentSlot::OFF_HAND
            } else {
                pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND
            };
            player.damage_item_in_slot(&slot, 1);
            let updated = player.inventory().get_stack_in_hand(hand);
            if updated.is_empty() {
                player.living_entity.clear_active_hand();
            } else {
                *player
                    .living_entity
                    .item_in_use
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(updated);
            }
        }
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
