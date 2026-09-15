use std::sync::Arc;

use pumpkin_data::{
    Block, BlockId, BlockStateId,
    effect::StatusEffect,
    entity::EntityType,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use pumpkin_protocol::{VarInt, java::client::play::CParticle, ser::NetworkWriteExt};
use pumpkin_util::{
    Difficulty,
    math::{position::BlockPos, vector3::Vector3},
    random::{RandomImpl, legacy_rand::LegacyRand},
    version::JavaMinecraftVersion,
};
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
        OnEntityCollisionArgs, OnScheduledTickArgs, RandomTickArgs, blocks::plant::PlantBlockBase,
    },
    entity::passive::bee::BeeEntity,
    world::World,
};

const EYEBLOSSOM_XZ_RANGE: i32 = 3;
const EYEBLOSSOM_Y_RANGE: i32 = 2;

pub struct EyeblossomBlock;

impl BlockMetadata for EyeblossomBlock {
    fn ids() -> Box<[BlockId]> {
        Box::new([BlockId::OPEN_EYEBLOSSOM, BlockId::CLOSED_EYEBLOSSOM])
    }
}

impl BlockBehaviour for EyeblossomBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let was_open = args.block == &Block::OPEN_EYEBLOSSOM;
        let changed = try_changing_state(args.world, args.block, args.position, None);
        if changed {
            let sound = if was_open {
                Sound::BlockEyeblossomClose
            } else {
                Sound::BlockEyeblossomOpen
            };
            args.world.play_sound(
                sound,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let was_open = args.block == &Block::OPEN_EYEBLOSSOM;
        if try_changing_state(args.world, args.block, args.position, Some(args.random)) {
            let sound = if was_open {
                Sound::BlockEyeblossomCloseLong
            } else {
                Sound::BlockEyeblossomOpenLong
            };
            args.world.play_sound(
                sound,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        {
            if args.world.level_info.load().difficulty == Difficulty::Peaceful {
                return;
            }

            if args.entity.get_entity().entity_type == &EntityType::BEE
                && BeeEntity::attracts_bees(args.state)
                && let Some(living_entity) = args.entity.get_living_entity()
                && !living_entity.has_effect(&StatusEffect::POISON)
            {
                let effect = pumpkin_data::potion::Effect {
                    effect_type: &StatusEffect::POISON,
                    duration: 25,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: true,
                };
                living_entity.add_effect(effect);
            }
        }
    }
}

impl PlantBlockBase for EyeblossomBlock {}

fn try_changing_state(
    world: &Arc<World>,
    current_block: &Block,
    pos: &BlockPos,
    mut random: Option<&mut LegacyRand>,
) -> bool {
    let is_open = current_block == &Block::OPEN_EYEBLOSSOM;
    let should_be_open = world.eyeblossom_open(pos).unwrap_or(is_open);

    if should_be_open == is_open {
        return false;
    }

    let new_block = if is_open {
        &Block::CLOSED_EYEBLOSSOM
    } else {
        &Block::OPEN_EYEBLOSSOM
    };

    let old_state = world.get_block_state(pos).id;
    world.set_block_state(pos, new_block.default_state.id, BlockFlags::NOTIFY_ALL);
    world.emit_game_event_from_entity(
        "minecraft:block_change",
        pos.to_centered_f64(),
        None,
        Some(old_state),
    );
    spawn_transform_particle(world, pos, !is_open, &mut random);

    for nearby_pos in BlockPos::iterate(
        pos.offset(Vector3::new(
            -EYEBLOSSOM_XZ_RANGE,
            -EYEBLOSSOM_Y_RANGE,
            -EYEBLOSSOM_XZ_RANGE,
        )),
        pos.offset(Vector3::new(
            EYEBLOSSOM_XZ_RANGE,
            EYEBLOSSOM_Y_RANGE,
            EYEBLOSSOM_XZ_RANGE,
        )),
    ) {
        if world.get_block_state(&nearby_pos).id == old_state {
            let distance = f64::from(pos.squared_distance(&nearby_pos)).sqrt();
            let min_delay = (distance * 5.0) as i32;
            let max_delay = (distance * 10.0) as i32;
            let bound = max_delay - min_delay + 1;
            let delay = min_delay
                + if let Some(random) = random.as_deref_mut() {
                    random.next_bounded_i32(bound)
                } else {
                    world.rand_bounded_i32(bound)
                };
            world.schedule_block_tick(
                current_block,
                nearby_pos,
                delay as u32,
                TickPriority::Normal,
            );
        }
    }
    true
}

fn spawn_transform_particle(
    world: &World,
    pos: &BlockPos,
    open: bool,
    random: &mut Option<&mut LegacyRand>,
) {
    let mut next_double = || {
        if let Some(random) = random.as_deref_mut() {
            random.next_f64()
        } else {
            world.rand_f64()
        }
    };
    let start = pos.to_centered_f64();
    let lifetime = 0.5 + next_double();
    let velocity = Vector3::new(
        next_double() - 0.5,
        next_double() + 1.0,
        next_double() - 0.5,
    );
    let target = start + velocity * lifetime;
    let mut bytes = Vec::new();
    // TrailParticleOption.STREAM_CODEC: Vec3 doubles, RGB int, duration VarInt.
    if bytes.write_f64_be(target.x).is_err()
        || bytes.write_f64_be(target.y).is_err()
        || bytes.write_f64_be(target.z).is_err()
        || bytes
            .write_i32_be(if open { 16545810 } else { 6250335 })
            .is_err()
        || bytes
            .write_var_int(&VarInt((20.0 * lifetime) as i32))
            .is_err()
    {
        return;
    }
    let packet = CParticle::new(
        false,
        false,
        start,
        Vector3::new(0.0, 0.0, 0.0),
        0.0,
        1,
        VarInt(Particle::Trail.to_id() as i32),
        &bytes,
    );
    for player in &world.get_nearby_players(start, 32.0) {
        if let crate::net::ClientPlatform::Java(client) = player.client.as_ref()
            && player.client.java_version() >= JavaMinecraftVersion::V_1_21_4
            && let Ok(data) = client.serialize_packet(&packet)
        {
            client.try_enqueue_packet(data);
        }
    }
}
