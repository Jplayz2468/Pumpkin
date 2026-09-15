use std::sync::Arc;

use pumpkin_data::block_properties::TurtleEggLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockStateId, tag};
use pumpkin_data::{environment_attribute::EnvironmentAttribute, world::WorldEvent};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::gamemode::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use uuid::Uuid;

use crate::block::{
    BlockBehaviour, BlockIsReplacing, BrokenArgs, CanUpdateAtArgs, OnEntityStepArgs,
    OnLandedUponArgs, OnPlaceArgs, PlacedArgs, RandomTickArgs,
};
use crate::entity::EntityBase;
use crate::entity::r#type::from_type;
use crate::world::World;

type TurtleEggProperties = TurtleEggLikeProperties;

#[pumpkin_block("minecraft:turtle_egg")]
pub struct TurtleEggBlock;

impl TurtleEggBlock {
    #[must_use]
    pub fn is_sand(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        world.get_block(pos).has_tag(&tag::Block::MINECRAFT_SAND)
    }

    #[must_use]
    pub fn on_sand(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        Self::is_sand(world, &pos.down())
    }

    #[must_use]
    pub fn can_destroy_egg(world: &World, entity: &dyn EntityBase) -> bool {
        let entity_type = entity.get_entity().entity_type;
        if entity_type.id == EntityType::TURTLE.id || entity_type.id == EntityType::BAT.id {
            return false;
        }
        if entity.get_living_entity().is_none() {
            return false;
        }
        if entity_type.id == EntityType::PLAYER.id {
            return true;
        }
        world.level_info.load().game_rules.mob_griefing
    }

    pub fn decrease_eggs(
        world: &Arc<World>,
        pos: &BlockPos,
        state_id: BlockStateId,
        block: &Block,
    ) {
        egg_sound(world, pos, Sound::EntityTurtleEggBreak);
        let mut props = TurtleEggProperties::from_state_id(state_id);
        if props.eggs <= 1 {
            world.break_block(pos, None, BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS);
        } else {
            props.eggs -= 1;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
            world.emit_game_event_from_entity(
                "block_destroy",
                pos.to_centered_f64(),
                None,
                Some(state_id),
            );
            world.sync_world_event(
                WorldEvent::ParticlesDestroyBlock,
                *pos,
                i32::from(state_id.as_u16()),
            );
        }
    }

    pub fn destroy_egg(
        world: &Arc<World>,
        pos: &BlockPos,
        state_id: BlockStateId,
        block: &Block,
        entity: &dyn EntityBase,
        randomness: u32,
    ) {
        if state_id.to_block() == &Block::TURTLE_EGG
            && Self::can_destroy_egg(world, entity)
            && world.rand_bounded_i32(randomness as i32) == 0
        {
            Self::decrease_eggs(world, pos, state_id, block);
        }
    }
}

impl BlockBehaviour for TurtleEggBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let mut properties = TurtleEggProperties::from_state_id(state_id);
            if properties.eggs < 4 {
                properties.eggs += 1;
            }
            return properties.to_state_id(args.block);
        }

        let properties = TurtleEggProperties::default(args.block);
        properties.to_state_id(args.block)
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        let b = BlockAccessor::get_block(args.world, args.position);
        !args.player.get_entity().is_sneaking()
            && TurtleEggProperties::from_state_id(args.state_id).eggs < 4
            && args.block.id == b.id
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if Self::on_sand(args.world.as_ref(), args.position) {
            args.world.sync_world_event(
                WorldEvent::ParticlesTurtleEggPlacement,
                *args.position,
                15,
            );
        }
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        if !args.entity.get_entity().is_sneaking() {
            Self::destroy_egg(
                args.world,
                args.position,
                args.state.id,
                args.block,
                args.entity,
                100,
            );
        }
    }

    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        // Zombie's subclasses all inherit the falling-on-eggs exemption.
        let kind = args.entity.get_entity().entity_type;
        if ![
            EntityType::ZOMBIE,
            EntityType::HUSK,
            EntityType::DROWNED,
            EntityType::ZOMBIE_VILLAGER,
            EntityType::ZOMBIFIED_PIGLIN,
        ]
        .iter()
        .any(|zombie| zombie.id == kind.id)
        {
            let (block, state) = args.world.get_block_and_state(args.position);
            Self::destroy_egg(args.world, args.position, state.id, block, args.entity, 3);
        }
        if let Some(living) = args.entity.get_living_entity() {
            living.handle_fall_damage(args.entity, args.fall_distance, 1.0);
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let chance = args.world.environment_attributes().get_value_f32(
            EnvironmentAttribute::GameplayTurtleEggHatchChance,
            args.position,
        );
        if chance <= 0.0
            || args.world.rand_f32() >= chance
            || !Self::on_sand(args.world.as_ref(), args.position)
        {
            return;
        }
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = TurtleEggProperties::from_state_id(state_id);
        if props.hatch < 2 {
            egg_sound(args.world, args.position, Sound::EntityTurtleEggCrack);
            props.hatch += 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                None,
                Some(state_id),
            );
        } else {
            egg_sound(args.world, args.position, Sound::EntityTurtleEggHatch);
            args.world
                .set_block_state(args.position, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            args.world.emit_game_event_from_entity(
                "block_destroy",
                args.position.to_centered_f64(),
                None,
                Some(state_id),
            );
            for i in 0..props.eggs {
                args.world.sync_world_event(
                    WorldEvent::ParticlesDestroyBlock,
                    *args.position,
                    i32::from(state_id.as_u16()),
                );
                let spawn_pos = Vector3::new(
                    args.position.0.x as f64 + 0.3 + f64::from(i) * 0.2,
                    args.position.0.y as f64,
                    args.position.0.z as f64 + 0.3,
                );
                let turtle = from_type(&EntityType::TURTLE, spawn_pos, args.world, Uuid::new_v4());
                if let Some(ageable) = turtle.get_mob().and_then(|mob| mob.as_ageable()) {
                    ageable.set_age(-24000);
                }
                if let Some(turtle_data) = turtle
                    .cast_any()
                    .downcast_ref::<crate::entity::passive::turtle::TurtleEntity>(
                ) {
                    turtle_data.home_pos.store(*args.position);
                }
                turtle.get_entity().set_rotation(0.0, 0.0);
                args.world.spawn_entity(turtle);
            }
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            Self::decrease_eggs(args.world, args.position, args.state.id, args.block);
        }
    }
}

fn egg_sound(world: &Arc<World>, pos: &BlockPos, sound: Sound) {
    world.play_sound_fine(
        sound,
        SoundCategory::Blocks,
        &pos.to_centered_f64(),
        0.7,
        0.9 + world.rand_f32() * 0.2,
    );
}
