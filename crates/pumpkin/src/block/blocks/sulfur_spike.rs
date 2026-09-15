use super::dripstone::DripstoneBlock;
use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnLandedUponArgs, OnPlaceArgs,
    OnProjectileHitArgs, OnScheduledTickArgs, PathComputationType, RandomTickArgs,
};
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:sulfur_spike")]
pub struct SulfurSpikeBlock;

impl BlockBehaviour for SulfurSpikeBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        DripstoneBlock.can_place_at(args)
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        DripstoneBlock.on_place(args)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        DripstoneBlock.get_state_for_neighbor_update(args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        DripstoneBlock.on_scheduled_tick(args);
    }
    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        DripstoneBlock.on_projectile_hit(args);
    }
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        DripstoneBlock.on_landed_upon(args);
    }
    fn random_tick(&self, args: RandomTickArgs<'_>) {
        DripstoneBlock.random_tick(args);
    }
    fn is_pathfindable(&self, _: &BlockState, _: PathComputationType) -> bool {
        false
    }
}
