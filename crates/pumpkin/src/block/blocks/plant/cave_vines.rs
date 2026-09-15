use super::growing::CAVE;
use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnScheduledTickArgs, RandomTickArgs,
};
use pumpkin_data::{BlockId, BlockStateId};
pub struct CaveVinesBlock;
impl BlockMetadata for CaveVinesBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::CAVE_VINES, BlockId::CAVE_VINES_PLANT].into()
    }
}
impl BlockBehaviour for CaveVinesBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        CAVE.can_place_at(args)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        CAVE.on_place(args)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        CAVE.neighbor_state(args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        CAVE.scheduled_tick(args);
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        CAVE.random_tick(args);
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        CAVE.bonemeal_target(args)
    }
    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        CAVE.bonemeal(args);
    }
    fn normal_use(
        &self,
        args: crate::block::NormalUseArgs<'_>,
    ) -> crate::block::registry::BlockActionResult {
        use crate::block::registry::BlockActionResult;
        use pumpkin_data::{
            Block,
            block_properties::{CaveVinesLikeProperties, CaveVinesPlantLikeProperties},
            sound::{Sound, SoundCategory},
        };
        let state = args.world.get_block_state_id(args.position);
        let next = if args.block == &Block::CAVE_VINES {
            let mut props = CaveVinesLikeProperties::from_state_id(state);
            if !props.berries {
                return BlockActionResult::Pass;
            }
            props.berries = false;
            props.to_state_id(args.block)
        } else {
            let mut props = CaveVinesPlantLikeProperties::from_state_id(state);
            if !props.berries {
                return BlockActionResult::Pass;
            }
            props.berries = false;
            props.to_state_id(args.block)
        };
        if !super::harvest_loot(&args, &pumpkin_data::loot_table::HARVEST_CAVE_VINE) {
            return BlockActionResult::Pass;
        }
        let pitch = 0.8_f32 + args.world.rand_f32() * (1.2_f32 - 0.8_f32);
        args.world.play_sound_fine(
            Sound::BlockCaveVinesPickBerries,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            1.0,
            pitch,
        );
        args.world.set_block_state(
            args.position,
            next,
            pumpkin_world::world::BlockFlags::NOTIFY_LISTENERS,
        );
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            Some(args.player.as_ref()),
            Some(next),
        );
        BlockActionResult::SuccessServer
    }
}
