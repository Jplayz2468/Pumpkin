use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, CanPlaceAtArgs, EmitsRedstonePowerArgs,
        GetRedstonePowerArgs, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs,
        OnScheduledTickArgs, OnStateReplacedArgs,
    },
    world::World,
};

use super::{PressurePlate, detection_box_at};

/// This is for Gold and Iron Pressure Plate
pub struct WeightedPressurePlateBlock;

type PressurePlateProps = pumpkin_data::block_properties::LightWeightedPressurePlateLikeProperties;

impl BlockMetadata for WeightedPressurePlateBlock {
    fn ids() -> Box<[BlockId]> {
        // light = Gold
        // heavy = Iron
        [
            BlockId::LIGHT_WEIGHTED_PRESSURE_PLATE,
            BlockId::HEAVY_WEIGHTED_PRESSURE_PLATE,
        ]
        .into()
    }
}

impl BlockBehaviour for WeightedPressurePlateBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        self.on_entity_collision_pp(args);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let output = self.get_redstone_output(args.block, state.id);
        if output > 0 {
            let (block, state) = args.world.get_block_and_state(args.position);
            Self.update_plate_state(args.world, args.position, block, state, output, None);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        self.on_state_replaced_pp(args);
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        self.get_redstone_output(args.block, args.state.id)
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.direction == BlockDirection::Up {
            return self.get_redstone_output(args.block, args.state.id);
        }
        0
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction == BlockDirection::Down
            && !Self::can_pressure_plate_place_at(args.world, args.position)
        {
            Block::AIR.default_state.id
        } else {
            args.state_id
        }
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_pressure_plate_place_at(args.block_accessor, args.position)
    }
}

impl PressurePlate for WeightedPressurePlateBlock {
    fn get_redstone_output(&self, _block: &Block, state: BlockStateId) -> u8 {
        let props = PressurePlateProps::from_state_id(state);
        props.power
    }

    fn calculate_redstone_output(&self, world: &World, block: &Block, pos: &BlockPos) -> u8 {
        // light = Gold (15)
        // heavy = Iron (150)
        let weight = if block == &Block::LIGHT_WEIGHTED_PRESSURE_PLATE {
            15
        } else {
            150
        };
        let aabb = detection_box_at(pos);
        let count = world
            .get_all_at_box(&aabb)
            .iter()
            .filter(|entity| !entity.is_spectator() && !entity.is_ignoring_block_triggers())
            .count();
        calculate_weighted_signal(count, weight)
    }

    fn set_redstone_output(&self, block: &Block, state: &BlockState, output: u8) -> BlockStateId {
        let mut props = PressurePlateProps::from_state_id(state.id);
        props.power = output;
        props.to_state_id(block)
    }

    fn tick_rate(&self) -> u32 {
        10
    }
}

/// Computes redstone signal strength for weighted pressure plates.
/// Matches vanilla formula in WeightedPressurePlateBlock.java:44-45:
/// `float percent = (float)Math.min(this.maxWeight, count) / this.maxWeight; return Mth.ceil(percent * 15.0F);`
#[must_use]
pub fn calculate_weighted_signal(count: usize, weight: usize) -> u8 {
    let count = count.min(weight);
    if count > 0 && weight > 0 {
        let f = count as f32 / weight as f32;
        (f * 15.0).ceil() as u8
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_weighted_plate_math() {
        // Gold plate: max weight 15 (1 entity = 1 power level)
        let weight = 15;
        assert_eq!(calculate_weighted_signal(0, weight), 0);
        assert_eq!(calculate_weighted_signal(1, weight), 1);
        assert_eq!(calculate_weighted_signal(5, weight), 5);
        assert_eq!(calculate_weighted_signal(14, weight), 14);
        assert_eq!(calculate_weighted_signal(15, weight), 15);
        assert_eq!(calculate_weighted_signal(100, weight), 15); // capped at max weight
    }

    #[test]
    fn test_heavy_weighted_plate_math() {
        // Iron plate: max weight 150 (up to 10 entities per power level)
        let weight = 150;
        assert_eq!(calculate_weighted_signal(0, weight), 0);
        assert_eq!(calculate_weighted_signal(1, weight), 1);
        assert_eq!(calculate_weighted_signal(10, weight), 1);
        assert_eq!(calculate_weighted_signal(11, weight), 2);
        assert_eq!(calculate_weighted_signal(20, weight), 2);
        assert_eq!(calculate_weighted_signal(21, weight), 3);
        assert_eq!(calculate_weighted_signal(150, weight), 15);
        assert_eq!(calculate_weighted_signal(300, weight), 15); // capped at max weight
    }

    #[test]
    fn test_weighted_plate_tick_rate() {
        // Weighted pressure plates tick every 10 ticks (WeightedPressurePlateBlock.java:63)
        assert_eq!(WeightedPressurePlateBlock.tick_rate(), 10);
    }
}
