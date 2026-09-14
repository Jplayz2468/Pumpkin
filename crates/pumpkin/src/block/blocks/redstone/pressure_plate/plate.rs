use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockState, BlockStateId,
    tag::{self},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, CanPlaceAtArgs, EmitsRedstonePowerArgs,
        GetRedstonePowerArgs, OnEntityCollisionArgs, OnNeighborUpdateArgs, OnScheduledTickArgs,
        OnStateReplacedArgs,
    },
    world::World,
};

use super::{PressurePlate, detection_box_at};

/// This is for Normal Pressure plates, so not Gold or Iron
pub struct PressurePlateBlock;

type PressurePlateProps = pumpkin_data::block_properties::StonePressurePlateLikeProperties;

impl BlockMetadata for PressurePlateBlock {
    fn ids() -> Box<[BlockId]> {
        let mut combined = Vec::new();
        combined.extend_from_slice(tag::Block::MINECRAFT_WOODEN_PRESSURE_PLATES.1);
        combined.extend_from_slice(tag::Block::MINECRAFT_STONE_PRESSURE_PLATES.1);
        combined.iter().map(|v| BlockId::new_or_air(*v)).collect()
    }
}

impl BlockBehaviour for PressurePlateBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        self.on_entity_collision_pp(args);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let output = self.get_redstone_output(args.block, state.id);
        if output > 0 {
            let (block, state) = args.world.get_block_and_state(args.position);
            Self.update_plate_state(args.world, args.position, block, state, output);
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

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if !Self::can_pressure_plate_place_at(args.world, args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.world
            .is_some_and(|world| Self::can_pressure_plate_place_at(world, args.position))
    }
}

impl PressurePlate for PressurePlateBlock {
    fn get_redstone_output(&self, _block: &Block, state: BlockStateId) -> u8 {
        let props = PressurePlateProps::from_state_id(state);
        if props.powered { 15 } else { 0 }
    }

    fn calculate_redstone_output(&self, world: &World, block: &Block, pos: &BlockPos) -> u8 {
        let aabb = detection_box_at(pos);
        let is_mobs_only = Self::is_mobs_only(block);

        let has_entity = world.get_entities_at_box(&aabb).iter().any(|e| {
            if is_mobs_only {
                e.get_living_entity().is_some()
            } else {
                true
            }
        });

        let has_player = world
            .get_players_at_box(&aabb)
            .iter()
            .any(|p| p.gamemode.load() != pumpkin_util::GameMode::Spectator);

        if has_entity || has_player { 15 } else { 0 }
    }

    fn set_redstone_output(&self, block: &Block, state: &BlockState, output: u8) -> BlockStateId {
        let mut props = PressurePlateProps::from_state_id(state.id);
        props.powered = output > 0;
        props.to_state_id(block)
    }
}

impl PressurePlateBlock {
    #[must_use]
    pub fn is_mobs_only(block: &Block) -> bool {
        block == &Block::STONE_PRESSURE_PLATE || block == &Block::POLISHED_BLACKSTONE_PRESSURE_PLATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn plate_sensitivity() {
        // Vanilla: Stone & Polished Blackstone plates only trigger for LivingEntity (mobs & players)
        assert!(PressurePlateBlock::is_mobs_only(&Block::STONE_PRESSURE_PLATE));
        assert!(PressurePlateBlock::is_mobs_only(
            &Block::POLISHED_BLACKSTONE_PRESSURE_PLATE
        ));

        // Vanilla: Wooden plates trigger for EVERYTHING (including items, arrows, projectiles)
        assert!(!PressurePlateBlock::is_mobs_only(&Block::OAK_PRESSURE_PLATE));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::SPRUCE_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::BIRCH_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::JUNGLE_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::ACACIA_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::DARK_OAK_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::MANGROVE_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::CHERRY_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::BAMBOO_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::CRIMSON_PRESSURE_PLATE
        ));
        assert!(!PressurePlateBlock::is_mobs_only(
            &Block::WARPED_PRESSURE_PLATE
        ));
    }

    #[test]
    fn plate_power_output_mapping() {
        let block = &Block::OAK_PRESSURE_PLATE;
        let mut props = PressurePlateProps::default(block);
        props.powered = false;
        assert_eq!(PressurePlateBlock.get_redstone_output(block, props.to_state_id(block)), 0);

        props.powered = true;
        assert_eq!(PressurePlateBlock.get_redstone_output(block, props.to_state_id(block)), 15);
    }
}

