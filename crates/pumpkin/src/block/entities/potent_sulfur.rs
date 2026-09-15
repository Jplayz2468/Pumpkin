use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use pumpkin_data::block_properties::{PotentSulfurLikeProperties, PotentSulfurState};
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::potion::Effect;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockState};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::random::xoroshiro128::Xoroshiro;
use pumpkin_util::random::{RandomDeriverImpl, RandomGenerator, RandomImpl};
use pumpkin_world::world::BlockFlags;

use super::BlockEntity;
use crate::world::World;

/// Vanilla `PotentSulfurBlockEntity.GEYSER_SALT` (`PotentSulfurBlockEntity.java:114`).
const GEYSER_SALT: u64 = (-904_011_478i64) as u64;

pub struct PotentSulfurBlockEntity {
    pub position: BlockPos,
    pub waiting_countdown: AtomicI32,
}

impl BlockEntity for PotentSulfurBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let countdown = nbt.get_int("countdown").unwrap_or(-1);
        Self {
            position,
            waiting_countdown: AtomicI32::new(countdown),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let countdown = self.waiting_countdown.load(Ordering::Relaxed);
        nbt.put_int("countdown", countdown);
    }

    /// Vanilla `PotentSulfurBlockEntity`'s `SERVER_*` tickers, dispatched the same way
    /// `PotentSulfurBlock.getTicker` (`PotentSulfurBlock.java:146-170`) combines them per
    /// state. The `CLIENT_*` tickers (`PotentSulfurBlockEntity.java:62-82`) are pure
    /// particle/sound cosmetics that only ever run on `level.isClientSide()`, which is
    /// always `false` for a dedicated server; real vanilla clients already reproduce them
    /// locally from the synced block state and block entity, so Pumpkin does not replicate
    /// them here.
    fn tick(&self, world: &Arc<World>) {
        let block = world.get_block(&self.position);
        if block != &Block::POTENT_SULFUR {
            return;
        }

        let state_id = world.get_block_state(&self.position).id;
        let current_state = PotentSulfurLikeProperties::from_state_id(state_id).potent_sulfur_state;

        match current_state {
            PotentSulfurState::Dry => {}
            PotentSulfurState::Wet => {
                self.nausea_effect_tick(world);
            }
            PotentSulfurState::Dormant => {
                self.waiting_countdown_tick(world, current_state);
                self.nausea_effect_tick(world);
            }
            PotentSulfurState::Erupting => {
                self.launch_entity_tick(world);
                self.waiting_countdown_tick(world, current_state);
            }
            PotentSulfurState::Continuous => {
                self.launch_entity_tick(world);
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PotentSulfurBlockEntity {
    pub const ID: &'static str = "minecraft:potent_sulfur";
    /// Vanilla `PotentSulfurBlockEntity.EFFECT_RANGE` (`PotentSulfurBlockEntity.java:42`).
    pub const EFFECT_RANGE: f64 = 3.0;

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            waiting_countdown: AtomicI32::new(-1),
        }
    }

    /// Vanilla `PotentSulfurBlockEntity.resetCountdown` (`PotentSulfurBlockEntity.java:161-163`).
    pub fn reset_countdown(&self) {
        self.waiting_countdown.store(-1, Ordering::Relaxed);
    }

    /// Vanilla `SERVER_NAUSEA_EFFECT_TICKER` (`PotentSulfurBlockEntity.java:50-61`).
    fn nausea_effect_tick(&self, world: &Arc<World>) {
        if world.get_world_age() % 10 != 0 {
            return;
        }
        let Some(source) = find_noxious_gas_source_block(world, &self.position) else {
            return;
        };

        // Vanilla `getNearbyLivingEntities`: `new AABB(pos).inflate(2.5, 0.0, 2.5)`.
        let search_box = BoundingBox::from_block(&source).expand(2.5, 0.0, 2.5);
        let mut entities = Vec::new();
        world.extend_entities_in_box_where(&mut entities, usize::MAX, search_box, |entity| {
            !entity.is_spectator() && entity.get_entity().is_alive()
        });

        for entity in entities {
            let Some(living) = entity.get_living_entity() else {
                continue;
            };
            let eye_pos = entity.get_eye_pos();
            if can_be_reached_by_noxious_gas(world, &source, eye_pos) {
                living.add_effect(Effect {
                    effect_type: &StatusEffect::NAUSEA,
                    duration: 80,
                    amplifier: 0,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }

    /// Vanilla `SERVER_WAITING_COUNTDOWN_TICKER` (`PotentSulfurBlockEntity.java:83-113`).
    fn waiting_countdown_tick(&self, world: &Arc<World>, current_state: PotentSulfurState) {
        if world.get_world_age() % 20 != 0 {
            return;
        }
        let Some(source) = find_noxious_gas_source_block(world, &self.position) else {
            return;
        };

        if self.waiting_countdown.load(Ordering::Relaxed) <= 0 {
            let water_blocks = source.0.y - self.position.0.y - 1;
            let mut rng = geyser_positional_random(world, &self.position);
            let countdown = if current_state == PotentSulfurState::Dormant {
                10 * (water_blocks - 1) + rng.next_inbetween_i32(15, 30)
            } else {
                // Vanilla still burns one `nextInt()` call here to stay in step with the
                // Dormant branch's extra draw, even though the result is unused.
                rng.next_i32();
                (water_blocks - 1) + rng.next_inbetween_i32(1, 2)
            };
            self.waiting_countdown.store(countdown, Ordering::Relaxed);
        }

        if self.waiting_countdown.load(Ordering::Relaxed) > 0 {
            self.waiting_countdown.fetch_sub(1, Ordering::Relaxed);
        }

        if self.waiting_countdown.load(Ordering::Relaxed) == 0 {
            let new_state = if current_state == PotentSulfurState::Dormant {
                PotentSulfurState::Erupting
            } else {
                PotentSulfurState::Dormant
            };

            let old_state = world.get_block_state_id(&self.position);
            let mut props = PotentSulfurLikeProperties::from_state_id(old_state);
            props.potent_sulfur_state = new_state;
            world.set_block_state(
                &self.position,
                props.to_state_id(&Block::POTENT_SULFUR),
                BlockFlags::NOTIFY_ALL,
            );

            if new_state == PotentSulfurState::Dormant {
                world.emit_game_event_from_entity(
                    "block_deactivate",
                    self.position.to_centered_f64(),
                    None,
                    Some(old_state),
                );
            }
        }
    }

    /// Vanilla `LAUNCH_ENTITY_TICKER` (`PotentSulfurBlockEntity.java:115-135`).
    fn launch_entity_tick(&self, world: &Arc<World>) {
        let Some(source) = find_noxious_gas_source_block(world, &self.position) else {
            return;
        };

        let water_blocks = source.0.y - self.position.0.y - 1;
        let above = self.position.up();
        let geyser_force_height = get_unobstructed_block_count(world, &above, water_blocks);
        let search_box = BoundingBox::from_block(&above).expand_towards(
            0.0,
            f64::from(geyser_force_height - 1),
            0.0,
        );

        let mut entities = Vec::new();
        world.extend_entities_in_box_where(&mut entities, usize::MAX, search_box, |entity| {
            !entity.is_spectator() && entity.get_entity().is_alive()
        });

        let max_speed_to_launch = f64::from(0.3f32) + f64::from(water_blocks) * 0.1;
        for entity in entities {
            let velocity = entity.get_entity().velocity.load();
            if velocity.y > -0.5 {
                if let Some(living) = entity.get_living_entity() {
                    living
                        .fall_distance
                        .store(living.fall_distance.load().min(1.0));
                }
                if let Some(falling) = entity
                    .cast_any()
                    .downcast_ref::<crate::entity::falling::FallingEntity>()
                {
                    falling.check_fall_distance_accumulation();
                }
            }
            // Player movement (and its geyser force) is simulated by the client.
            if entity.get_player().is_some() || entity.is_passenger() {
                continue;
            }
            if entity
                .get_entity()
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_NOT_AFFECTED_BY_GEYSERS)
            {
                continue;
            }
            let base_entity = entity.get_entity();
            let velocity = base_entity.velocity.load();
            if velocity.y < max_speed_to_launch {
                base_entity.velocity.store(Vector3::new(
                    velocity.x,
                    velocity.y + f64::from(0.2f32),
                    velocity.z,
                ));
                base_entity.velocity_dirty.store(true, Ordering::Relaxed);
            }
        }
    }
}

/// Vanilla `PotentSulfurBlockEntity.findNoxiousGasSourceBlock`
/// (`PotentSulfurBlockEntity.java:208-227`): climbs the water column above the geyser, up to
/// `PotentSulfurBlock.ALLOWED_WATER_BLOCKS_ABOVE` (4) blocks, and returns the first
/// non-water passable block above it (the water's "surface").
fn find_noxious_gas_source_block(world: &Arc<World>, origin: &BlockPos) -> Option<BlockPos> {
    let max_y = origin.0.y + 4 + 1;
    let mut pos = origin.up();

    loop {
        if pos.0.y > max_y {
            return None;
        }

        let state = world.get_block_state(&pos);
        let block = world.get_block(&pos);
        let (fluid, fluid_state) =
            World::fluid_state_from_block_state(world.get_block_state_id(&pos));
        let is_water_source = fluid.matches_type(&Fluid::WATER) && fluid_state.is_source;

        if !is_water_source
            || (block != &Block::WATER && !is_geyser_passable_block(state, block, &pos))
        {
            return if state.is_air() || is_geyser_passable_block(state, block, &pos) {
                Some(pos)
            } else {
                None
            };
        }

        pos = pos.up();
    }
}

/// Vanilla `PotentSulfurBlockEntity.isGeyserPassableBlock`
/// (`PotentSulfurBlockEntity.java:204-206`). Uses the block-local collision shapes rather
/// than a position-scoped `CollisionContext`, since Pumpkin's shape query has no equivalent
/// of vanilla's `CollisionContext.positionContext`.
fn is_geyser_passable_block(
    state: &'static BlockState,
    block: &'static Block,
    pos: &BlockPos,
) -> bool {
    if state.is_air() || block == &Block::WATER {
        true
    } else {
        state.get_block_collision_shapes_at(pos).next().is_none()
    }
}

/// Vanilla `PotentSulfurBlockEntity.getUnobstructedBlockCount`
/// (`PotentSulfurBlockEntity.java:189-202`).
fn get_unobstructed_block_count(world: &Arc<World>, start: &BlockPos, water_blocks: i32) -> i32 {
    let geyser_force_height = 6 * water_blocks;
    for i in 0..geyser_force_height {
        let current = start.up_height(i);
        let state = world.get_block_state(&current);
        let block = world.get_block(&current);
        if !is_geyser_passable_block(state, block, &current) {
            return i;
        }
    }
    geyser_force_height
}

/// Vanilla `PotentSulfurBlockEntity.canBeReachedByNoxiousGas`
/// (`PotentSulfurBlockEntity.java:229-243`).
fn can_be_reached_by_noxious_gas(
    world: &Arc<World>,
    source: &BlockPos,
    eye_pos: Vector3<f64>,
) -> bool {
    let at_pos = BlockPos::floored(eye_pos.x, eye_pos.y, eye_pos.z);
    let state = world.get_block_state(&at_pos);
    let block = world.get_block(&at_pos);
    if !is_geyser_passable_block(state, block, &at_pos) {
        return false;
    }

    let source_center = source.to_centered_f64();
    if eye_pos.squared_distance_to_vec(&source_center) > 9.0 {
        return false;
    }

    let below_source = source.down().to_centered_f64();
    let below_pos = Vector3::new(eye_pos.x, eye_pos.y - 1.0, eye_pos.z);
    let below_block_pos = BlockPos::floored(below_pos.x, below_pos.y, below_pos.z);
    let (below_fluid, below_fluid_state) =
        World::fluid_state_from_block_state(world.get_block_state_id(&below_block_pos));
    let is_water = below_fluid.matches_type(&Fluid::WATER) && below_fluid_state.is_source;

    is_water && world.has_line_of_sight(below_source, below_pos)
}

/// Vanilla `PotentSulfurBlockEntity.geyserPositional` (`PotentSulfurBlockEntity.java:174-176`):
/// a deterministic, position-seeded random source used so the countdown doesn't need to be
/// persisted while `> 0`.
fn geyser_positional_random(world: &Arc<World>, pos: &BlockPos) -> RandomGenerator {
    let seed = world.level.seed.0 ^ GEYSER_SALT;
    let mut base = Xoroshiro::from_seed(seed);
    let deriver = base.next_splitter();
    RandomGenerator::Xoroshiro(deriver.split_pos(pos.0.x, pos.0.y, pos.0.z))
}
