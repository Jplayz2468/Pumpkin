use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnNeighborUpdateArgs, PlacedArgs};
use crate::world::World;
use pumpkin_data::{
    Block, BlockDirection, BlockId, BlockStateId,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    world::WorldEvent,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

#[pumpkin_block("minecraft:sponge")]
pub struct SpongeBlock;
impl SpongeBlock {
    pub fn absorb_water(world: &Arc<World>, position: &BlockPos) -> bool {
        let mut queue = VecDeque::from([(*position, 0)]);
        let mut visited = HashSet::new();
        let mut accepted = 0;
        let mut approved = false;
        while let Some((pos, depth)) = queue.pop_front() {
            if !visited.insert(pos) {
                continue;
            }
            if pos != *position {
                let (block, state) = world.get_block_and_state(&pos);
                if !world.get_fluid(&pos).has_tag(&tag::Fluid::MINECRAFT_WATER) {
                    continue;
                }
                let drained = block.set_waterlogged(state.id, false);
                let is_liquid = block == &Block::WATER || block == &Block::BUBBLE_COLUMN;
                let aquatic_plant = matches!(
                    block.id,
                    BlockId::KELP
                        | BlockId::KELP_PLANT
                        | BlockId::SEAGRASS
                        | BlockId::TALL_SEAGRASS
                );
                if drained.is_none() && !is_liquid && !aquatic_plant {
                    continue;
                }
                if !approved {
                    let mut event =
                        crate::plugin::api::events::block::sponge_absorb::SpongeAbsorbEvent::new(
                            *position,
                        );
                    if let Some(server) = world.server.upgrade() {
                        server.plugin_manager.fire_blocking(&server, &mut event);
                    }
                    if event.cancelled {
                        return false;
                    }
                    approved = true;
                }
                if let Some(dry) = drained {
                    world.set_block_state(&pos, dry, BlockFlags::NOTIFY_ALL);
                    let survives =
                        world
                            .block_registry
                            .get_pumpkin_block(block.id)
                            .is_none_or(|behaviour| {
                                behaviour.can_place_at(CanPlaceAtArgs {
                                    server: None,
                                    world: Some(world),
                                    block_accessor: world.as_ref(),
                                    block,
                                    state,
                                    position: &pos,
                                    direction: None,
                                    player: None,
                                    use_item_on: None,
                                })
                            });
                    if !survives {
                        world.break_block(&pos, None, BlockFlags::NOTIFY_ALL);
                    }
                } else {
                    if aquatic_plant {
                        crate::block::drop_loot(
                            world,
                            block,
                            &pos,
                            false,
                            &crate::world::loot::LootContextParameters {
                                block_state: Some(state),
                                position: Some(pos.to_centered_f64()),
                                ..Default::default()
                            },
                        );
                    }
                    world.set_block_state(&pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
                }
            }
            accepted += 1;
            if accepted >= 65 {
                break;
            }
            if depth < 6 {
                for direction in BlockDirection::all() {
                    queue.push_back((pos.offset(direction.to_offset()), depth + 1));
                }
            }
        }
        if accepted <= 1 {
            return false;
        }
        world.set_block_state(
            position,
            Block::WET_SPONGE.default_state.id,
            BlockFlags::NOTIFY_LISTENERS,
        );
        world.play_sound(
            Sound::BlockSpongeAbsorb,
            SoundCategory::Blocks,
            &position.to_centered_f64(),
        );
        true
    }
}
impl BlockBehaviour for SpongeBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        Self::absorb_water(args.world, args.position);
    }
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        Self::absorb_water(args.world, args.position);
    }
}

#[pumpkin_block("minecraft:wet_sponge")]
pub struct WetSpongeBlock;
impl BlockBehaviour for WetSpongeBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        if args.world.environment_attributes().get_value_bool(
            pumpkin_data::environment_attribute::EnvironmentAttribute::GameplayWaterEvaporates,
            args.position,
        ) {
            args.world.set_block_state(
                args.position,
                Block::SPONGE.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            args.world
                .sync_world_event(WorldEvent::ParticlesWaterEvaporating, *args.position, 0);
            args.world.play_sound_fine(
                Sound::BlockWetSpongeDries,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
                1.0,
                (1.0 + args.world.rand_f32() * 0.2) * 0.7,
            );
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
}
