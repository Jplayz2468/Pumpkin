use std::sync::Arc;

use pumpkin_data::dimension::Dimension;
use pumpkin_data::tag::Taggable;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockDirection, tag};
use pumpkin_util::math::position::BlockPos;
use soul_fire::SoulFireBlock;

use crate::block::blocks::fire::fire::FireBlock;
use crate::block::{BlockBehaviour, CanPlaceAtArgs, OnEntityCollisionArgs};
use crate::entity::EntityBase;
use crate::world::World;
use crate::world::portal::nether::NetherPortal;
use pumpkin_data::damage::DamageType;

#[expect(clippy::module_inception)]
pub mod fire;
pub mod soul_fire;

pub struct FireBlockBase;

impl FireBlockBase {
    pub fn get_fire_type(world: &World, pos: &BlockPos) -> Block {
        let block = world.get_block(&pos.down());
        if SoulFireBlock::is_soul_base(block) {
            return Block::SOUL_FIRE;
        }
        Block::FIRE
    }

    #[must_use]
    pub fn can_place_on(block: &Block) -> bool {
        // Make sure the block below is not a fire block or fluid block
        block != &Block::SOUL_FIRE
            && block != &Block::FIRE
            && block != &Block::WATER
            && block != &Block::LAVA
    }

    pub fn is_soul_fire(world: &Arc<World>, block_pos: &BlockPos) -> bool {
        let block = world.get_block(&block_pos.down());
        block.has_tag(&tag::Block::MINECRAFT_SOUL_FIRE_BASE_BLOCKS)
    }

    pub fn can_place_at(world: &Arc<World>, block_pos: &BlockPos) -> bool {
        let block_state = world.get_block_state(block_pos);
        if !block_state.is_air() {
            return false;
        }
        let survives = if Self::is_soul_fire(world, block_pos) {
            SoulFireBlock.can_place_at(CanPlaceAtArgs {
                server: None,
                world: Some(world),
                block_accessor: world.as_ref(),
                block: &Block::SOUL_FIRE,
                state: Block::SOUL_FIRE.default_state,
                position: block_pos,
                direction: None,
                player: None,
                use_item_on: None,
            })
        } else {
            FireBlock.can_place_at(CanPlaceAtArgs {
                server: None,
                world: Some(world),
                block_accessor: world.as_ref(),
                block: &Block::FIRE,
                state: Block::FIRE.default_state,
                position: block_pos,
                direction: None,
                player: None,
                use_item_on: None,
            })
        };
        survives || Self::should_light_portal_at(world, block_pos, BlockDirection::Up)
    }

    pub fn should_light_portal_at(
        world: &Arc<World>,
        block_pos: &BlockPos,
        direction: BlockDirection,
    ) -> bool {
        let dimension = &world.dimension;
        if dimension != &Dimension::OVERWORLD && dimension != &Dimension::THE_NETHER {
            return false;
        }
        let mut found = false;

        for dir in BlockDirection::all() {
            if world.get_block(&block_pos.offset(dir.to_offset())) == &Block::OBSIDIAN {
                found = true;
                break;
            }
        }

        if !found {
            return false;
        }

        let dir = if direction.is_horizontal() {
            direction
                .rotate_counter_clockwise()
                .to_horizontal_axis()
                .unwrap_or(pumpkin_data::block_properties::HorizontalAxis::X)
        } else {
            if world.rand_bounded_i32(2) == 0 {
                pumpkin_data::block_properties::HorizontalAxis::X
            } else {
                pumpkin_data::block_properties::HorizontalAxis::Z
            }
        };
        NetherPortal::get_new_portal(world, block_pos, dir).is_some()
    }

    /// Shared fire collision behavior used by `fire` and `soul_fire`.
    pub fn apply_fire_collision(args: &OnEntityCollisionArgs<'_>, extra_damage_for_living: bool) {
        use crate::entity::inside_effects::Effect;
        args.effects.apply(Effect::ClearFreeze);
        args.effects.apply(Effect::FireIgnite);
        args.effects.after(Effect::FireIgnite, move |entity| {
            entity.damage(
                entity,
                if extra_damage_for_living { 2.0 } else { 1.0 },
                DamageType::IN_FIRE,
            );
        });
    }

    pub fn placed(args: &crate::block::PlacedArgs<'_>) {
        if matches!(args.world.dimension.id, id if id == Dimension::OVERWORLD.id || id == Dimension::THE_NETHER.id)
            && let Some(portal) = NetherPortal::get_new_portal(
                args.world,
                args.position,
                pumpkin_data::block_properties::HorizontalAxis::X,
            )
        {
            portal.create(args.world);
            return;
        }
        if let Some(behaviour) = args.world.block_registry.get_pumpkin_block(args.block.id)
            && !behaviour.can_place_at(CanPlaceAtArgs {
                server: None,
                world: Some(args.world),
                block_accessor: args.world.as_ref(),
                block: args.block,
                state: args.state_id.to_state(),
                position: args.position,
                direction: None,
                player: None,
                use_item_on: None,
            })
        {
            args.world.set_block_state(
                args.position,
                Block::AIR.default_state.id,
                pumpkin_world::world::BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn broken(world: &World, block_pos: BlockPos) {
        world.sync_world_event(WorldEvent::SoundExtinguishFire, block_pos, 0);
    }
}
