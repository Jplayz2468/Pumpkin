use crate::world::{DefaultExplosionDamageCalculator, Explosion, ExplosionDamageCalculator, World};
use pumpkin_data::block_properties::RespawnAnchorLikeProperties;
use pumpkin_data::item::Item;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{
    Block, BlockDirection, BlockState,
    data_component_impl::EquipmentSlot,
    fluid::{Fluid, FluidState},
    translation,
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, GetComparatorOutputArgs, NormalUseArgs, PathComputationType, UseWithItemArgs,
};

/// Vanilla `RespawnAnchorBlock.MAX_CHARGES`.
const MAX_CHARGES: u8 = 4;

#[pumpkin_block("minecraft:respawn_anchor")]
pub struct RespawnAnchorBlock;

impl BlockBehaviour for RespawnAnchorBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state_id(args.position);
        let mut props = RespawnAnchorLikeProperties::from_state_id(state);
        if args.item_stack.item == &Item::GLOWSTONE
            && !args.item_stack.is_empty()
            && props.charges < MAX_CHARGES
        {
            props.charges += 1;
            let state = props.to_state_id(args.block);
            args.world
                .set_block_state(args.position, state, BlockFlags::NOTIFY_ALL);
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                Some(args.player.as_ref()),
                Some(state),
            );
            args.world.play_sound(
                Sound::BlockRespawnAnchorCharge,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
            if !args.player.has_infinite_materials() {
                args.item_stack.decrement(1);
            }
            BlockActionResult::Success
        } else {
            let offhand = args.player.inventory().off_hand_item();
            if *args.equipment_slot == EquipmentSlot::MAIN_HAND
                && offhand.item == &Item::GLOWSTONE
                && !offhand.is_empty()
                && props.charges < MAX_CHARGES
            {
                BlockActionResult::Pass
            } else {
                BlockActionResult::PassToDefaultBlockAction
            }
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let props = RespawnAnchorLikeProperties::from_state_id(
            args.world.get_block_state_id(args.position),
        );
        if props.charges == 0 {
            return BlockActionResult::Pass;
        }
        if !args.world.respawn_anchor_works(args.position) {
            args.world.set_block_state(
                args.position,
                Block::AIR.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            let water = [
                BlockDirection::North,
                BlockDirection::South,
                BlockDirection::West,
                BlockDirection::East,
            ]
            .into_iter()
            .any(|direction| {
                water_would_flow(args.world, &args.position.offset(direction.to_offset()))
            }) || World::fluid_state_from_block_state(
                args.world.get_block_state_id(&args.position.up()),
            )
            .0
            .matches_type(&Fluid::WATER);
            args.world.explode_bad_respawn_point(
                args.position.to_centered_f64(),
                Some(Arc::new(AnchorExplosionCalculator {
                    position: *args.position,
                    water,
                })),
            );
            return BlockActionResult::SuccessServer;
        }
        if args.player.set_respawn_point(
            args.world.dimension.clone(),
            *args.position,
            0.0,
            0.0,
            false,
        ) {
            args.player
                .send_system_message(&pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_SET_SPAWN,
                    translation::bedrock::TILE_BED_RESPAWNSET
                ));
            args.world.play_sound(
                Sound::BlockRespawnAnchorSetSpawn,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
            BlockActionResult::SuccessServer
        } else {
            BlockActionResult::Consume
        }
    }

    /// Charges scale over the full signal range, so each charge is worth 15 / 4.
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = RespawnAnchorLikeProperties::from_state_id(args.state.id);
        Some(props.charges * 15 / MAX_CHARGES)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn water_would_flow(world: &World, pos: &BlockPos) -> bool {
    let (fluid, state) = World::fluid_state_from_block_state(world.get_block_state_id(pos));
    if !fluid.matches_type(&Fluid::WATER) {
        return false;
    }
    state.is_source
        || (state.level >= 2
            && !World::fluid_state_from_block_state(world.get_block_state_id(&pos.down()))
                .0
                .matches_type(&Fluid::WATER))
}

struct AnchorExplosionCalculator {
    position: BlockPos,
    water: bool,
}
impl ExplosionDamageCalculator for AnchorExplosionCalculator {
    fn get_block_explosion_resistance(
        &self,
        explosion: &Explosion<'_>,
        world: &World,
        pos: &BlockPos,
        block: &Block,
        fluid: &FluidState,
    ) -> Option<f32> {
        if *pos == self.position && self.water {
            Some(Block::WATER.blast_resistance)
        } else {
            DefaultExplosionDamageCalculator
                .get_block_explosion_resistance(explosion, world, pos, block, fluid)
        }
    }
}
