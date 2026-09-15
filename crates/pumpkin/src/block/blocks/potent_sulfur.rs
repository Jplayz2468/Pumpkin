use pumpkin_data::block_properties::{PotentSulfurLikeProperties, PotentSulfurState};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;

use crate::block::entities::potent_sulfur::PotentSulfurBlockEntity;
use crate::block::{BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, PlacedArgs};
use crate::world::World;

#[pumpkin_block("minecraft:potent_sulfur")]
pub struct PotentSulfurBlock;

impl BlockBehaviour for PotentSulfurBlock {
    /// Vanilla `PotentSulfurBlock.getStateForPlacement` (`PotentSulfurBlock.java:71-73`).
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        valid_block_state(args.world, args.position, args.block.default_state.id)
    }

    /// Vanilla `PotentSulfurBlock.updateShape` (`PotentSulfurBlock.java:56-68`): every
    /// neighbor change re-derives the geyser state the same way placement does, regardless
    /// of which side changed.
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        valid_block_state(args.world, args.position, args.state_id)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        // Vanilla `PotentSulfurBlock.onPlace` (`PotentSulfurBlock.java:102-117`): a geyser
        // that starts out already active announces its eruption immediately.
        let props = PotentSulfurLikeProperties::from_state_id(args.state_id);
        if matches!(
            props.potent_sulfur_state,
            PotentSulfurState::Erupting | PotentSulfurState::Continuous
        ) {
            args.world.add_synced_block_event(*args.position, 0, 0);
            let sound = if props.potent_sulfur_state == PotentSulfurState::Continuous {
                Sound::BlockPotentSulfurGeyserContinuousEruption
            } else {
                Sound::BlockPotentSulfurGeyserEruption
            };
            args.world.play_sound(
                sound,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
            args.world.emit_game_event_from_entity(
                "block_activate",
                args.position.to_centered_f64(),
                None,
                Some(args.state_id),
            );
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
}

/// Vanilla `PotentSulfurBlock.validBlockState` (`PotentSulfurBlock.java:75-95`): a geyser is
/// `Dry` unless a water source sits directly above it. If it does, the block below decides
/// whether it is merely `Wet`, erupts `Continuous`ly (lava below), or cycles between
/// `Dormant`/`Erupting` (a periodic-eruption block, e.g. magma, below).
fn valid_block_state(world: &World, pos: &BlockPos, state_id: BlockStateId) -> BlockStateId {
    let mut props = PotentSulfurLikeProperties::from_state_id(state_id);

    let (above_fluid, above_fluid_state) =
        World::fluid_state_from_block_state(world.get_block_state_id(&pos.up()));
    if !(above_fluid.matches_type(&Fluid::WATER) && above_fluid_state.is_source) {
        props.potent_sulfur_state = PotentSulfurState::Dry;
        return props.to_state_id(&Block::POTENT_SULFUR);
    }

    let below_pos = pos.down();
    let below_block = world.get_block(&below_pos);
    let (_, below_fluid_state) =
        World::fluid_state_from_block_state(world.get_block_state_id(&below_pos));
    // Vanilla `isSourceIfFluid`: no fluid at all, or a fluid source, both count.
    let is_source_if_fluid = below_fluid_state.is_empty || below_fluid_state.is_source;

    if below_block.has_tag(&tag::Block::MINECRAFT_CAUSES_CONTINUOUS_GEYSER_ERUPTIONS)
        && is_source_if_fluid
    {
        props.potent_sulfur_state = PotentSulfurState::Continuous;
    } else if below_block.has_tag(&tag::Block::MINECRAFT_CAUSES_PERIODIC_GEYSER_ERUPTIONS)
        && is_source_if_fluid
    {
        let is_geyser = matches!(
            props.potent_sulfur_state,
            PotentSulfurState::Erupting | PotentSulfurState::Dormant
        );
        if !is_geyser
            && let Some(block_entity) = world.get_block_entity(pos)
            && let Some(entity) = block_entity
                .as_any()
                .downcast_ref::<PotentSulfurBlockEntity>()
        {
            entity.reset_countdown();
        }

        if props.potent_sulfur_state != PotentSulfurState::Erupting {
            props.potent_sulfur_state = PotentSulfurState::Dormant;
        }
    } else {
        props.potent_sulfur_state = PotentSulfurState::Wet;
    }

    props.to_state_id(&Block::POTENT_SULFUR)
}
