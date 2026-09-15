use super::BlockEntity;
use crate::{
    entity::{EntityBase, ageable::AgeableMob, mob::Mob, passive::bee::BeeEntity, player::Player},
    world::World,
};
use pumpkin_data::{
    BlockState,
    block_properties::BeeNestLikeProperties,
    data_component_impl::{BeeOccupant, BeesImpl},
    entity::EntityType,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::{math::vector3::Vector3, random::RandomImpl};
use pumpkin_world::world::BlockFlags;
use std::sync::Mutex;
use std::sync::{Arc, atomic::Ordering::Relaxed};

pub struct BeehiveBlockEntity {
    pub position: BlockPos,
    pub bees: Mutex<Vec<BeeOccupant>>,
    pub flower_pos: Mutex<Option<BlockPos>>,
}

impl BlockEntity for BeehiveBlockEntity {
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
        let bees = nbt
            .get_list("bees")
            .or_else(|| nbt.get_list("Bees"))
            .and_then(|list| {
                list.iter()
                    .map(|tag| {
                        let mut value = tag.extract_compound()?.clone();
                        for (old, new) in [
                            ("EntityData", "entity_data"),
                            ("TicksInHive", "ticks_in_hive"),
                            ("MinOccupationTicks", "min_ticks_in_hive"),
                        ] {
                            if value.get(new).is_none()
                                && let Some(tag) = value.get(old).cloned()
                            {
                                value.put(new, tag);
                            }
                        }
                        BeeOccupant::read_data(&NbtTag::Compound(value))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        let flower_pos = nbt
            .get_int_array("flower_pos")
            .and_then(|arr| (arr.len() == 3).then(|| BlockPos::new(arr[0], arr[1], arr[2])))
            .or_else(|| {
                nbt.get_compound("flower_pos")
                    .or_else(|| nbt.get_compound("FlowerPos"))
                    .map(|c| {
                        BlockPos::new(
                            c.get_int("X").or_else(|| c.get_int("x")).unwrap_or(0),
                            c.get_int("Y").or_else(|| c.get_int("y")).unwrap_or(0),
                            c.get_int("Z").or_else(|| c.get_int("z")).unwrap_or(0),
                        )
                    })
            });
        Self {
            position,
            bees: Mutex::new(bees),
            flower_pos: Mutex::new(flower_pos),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_list(
            "bees",
            self.bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .map(BeeOccupant::write_data)
                .collect(),
        );
        if let Ok(fp) = self.flower_pos.lock()
            && let Some(fp) = fp.as_ref()
        {
            nbt.put(
                "flower_pos",
                pumpkin_nbt::tag::NbtTag::IntArray(vec![fp.0.x, fp.0.y, fp.0.z]),
            );
        }
    }

    fn tick(&self, world: &Arc<World>) {
        let state = world.get_block_state(&self.position);
        if !state.id.to_block().has_tag(&tag::Block::MINECRAFT_BEEHIVES) {
            return;
        }
        let ready = {
            let mut bees = self
                .bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if bees.is_empty() {
                return;
            }
            bees.iter_mut()
                .filter_map(|bee| {
                    // BeeData.tick uses post-increment and a strict greater-than check.
                    let ready = bee.ticks_in_hive > bee.min_ticks_in_hive;
                    bee.ticks_in_hive = bee.ticks_in_hive.wrapping_add(1);
                    ready.then(|| bee.clone())
                })
                .collect::<Vec<_>>()
        };
        for occupant in ready {
            let reason = if occupant.entity_data.get_bool("HasNectar").unwrap_or(false) {
                BeeReleaseStatus::HoneyDelivered
            } else {
                BeeReleaseStatus::Released
            };
            let Some(index) = self.take_occupant(&occupant) else {
                continue;
            };
            if self
                .release_occupant(world, state, &occupant, reason)
                .is_none()
            {
                self.restore_occupant(index, occupant);
            }
        }
        self.mark_dirty(world);
        let occupied = !self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty();
        if occupied
            && world
                .random
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next_f64()
                < 0.005
        {
            world.play_sound(
                Sound::BlockBeehiveWork,
                SoundCategory::Blocks,
                &(self.position.to_centered_f64() - Vector3::new(0.0, 0.5, 0.0)),
            );
        }
    }

    fn write_dropped_stack_components(&self, stack: &mut ItemStack) {
        stack.set_data_component(BeesImpl {
            bees: self
                .bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        });
    }

    fn apply_components_from_item_stack(&self, stack: &ItemStack) {
        *self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .get_data_component::<BeesImpl>()
            .map_or_else(Vec::new, |data| data.bees.clone());
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BeehiveBlockEntity {
    pub const ID: &'static str = "minecraft:beehive";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            bees: Mutex::new(Vec::new()),
            flower_pos: Mutex::new(None),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BeeReleaseStatus {
    HoneyDelivered,
    Released,
    Emergency,
}

// BeehiveBlockEntity.IGNORED_BEE_TAGS: only portable occupant state survives a stay.
const IGNORED_BEE_TAGS: &[&str] = &[
    "Air",
    "drop_chances",
    "equipment",
    "Brain",
    "CanPickUpLoot",
    "DeathTime",
    "fall_distance",
    "FallFlying",
    "Fire",
    "HurtTime",
    "LeftHanded",
    "Motion",
    "NoGravity",
    "OnGround",
    "PortalCooldown",
    "Pos",
    "Rotation",
    "sleeping_pos",
    "CannotEnterHiveTicks",
    "TicksSincePollination",
    "CropsGrownSincePollination",
    "hive_pos",
    "Passengers",
    "leash",
    "UUID",
];

impl BeehiveBlockEntity {
    fn mark_dirty(&self, world: &World) {
        world
            .level
            .read_chunk_sync(&self.position.chunk_position(), |chunk| {
                chunk.mark_dirty(true)
            });
    }

    // Reserve the occupant before world callbacks. In particular, honey updates
    // may notify nearby fire, which can re-enter emergency release on this hive.
    fn take_occupant(&self, occupant: &BeeOccupant) -> Option<usize> {
        let mut bees = self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = bees.iter().position(|bee| bee == occupant)?;
        bees.remove(index);
        Some(index)
    }
    fn restore_occupant(&self, index: usize, occupant: BeeOccupant) {
        let mut bees = self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = index.min(bees.len());
        bees.insert(index, occupant);
    }

    pub fn empty_all(
        &self,
        world: &Arc<World>,
        state: &BlockState,
        player: Option<&Player>,
        reason: BeeReleaseStatus,
    ) {
        let occupants = self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for occupant in occupants {
            let Some(index) = self.take_occupant(&occupant) else {
                continue;
            };
            if let Some(entity) = self.release_occupant(world, state, &occupant, reason) {
                if let Some(player) = player
                    && let Some(bee) = entity.cast_any().downcast_ref::<BeeEntity>()
                {
                    let delta =
                        player.living_entity.entity.pos.load() - entity.get_entity().pos.load();
                    if delta.x * delta.x + delta.y * delta.y + delta.z * delta.z <= 16.0 {
                        if crate::block::blocks::beehive::is_smokey_pos(world, &self.position) {
                            bee.cannot_enter_hive_ticks.store(400, Relaxed);
                        } else if let Some(player) =
                            world.get_entity_by_id(player.living_entity.entity.entity_id)
                        {
                            bee.set_mob_target(Some(player));
                        }
                    }
                }
                self.mark_dirty(world);
            } else {
                self.restore_occupant(index, occupant);
            }
        }
    }

    fn release_occupant(
        &self,
        world: &Arc<World>,
        state: &BlockState,
        occupant: &BeeOccupant,
        reason: BeeReleaseStatus,
    ) -> Option<Arc<dyn EntityBase>> {
        if reason != BeeReleaseStatus::Emergency && world.bees_stay_in_hive(&self.position) {
            return None;
        }
        let props = BeeNestLikeProperties::from_state_id(state.id);
        let facing = props.facing.to_offset();
        let front = self.position.offset(facing);
        let blocked = !world.get_block_state(&front).collision_shapes.is_empty();
        if blocked && reason != BeeReleaseStatus::Emergency {
            return None;
        }
        let mut data = occupant.entity_data.clone();
        let id = data.get_string("id")?;
        let kind = EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))?;
        if !kind.has_tag(&tag::EntityType::MINECRAFT_BEEHIVE_INHABITORS) {
            return None;
        }
        for key in IGNORED_BEE_TAGS {
            data.child_tags.remove(*key);
        }
        let entity = crate::entity::r#type::from_type(
            kind,
            self.position.to_centered_f64(),
            world,
            uuid::Uuid::new_v4(),
        );
        entity.read_nbt_non_mut(&data);
        entity.get_entity().set_has_no_gravity(true);
        if let Some(bee) = entity.cast_any().downcast_ref::<BeeEntity>() {
            bee.hive_pos.store(Some(self.position));
            if !bee.is_age_locked() {
                let age = bee.get_age();
                bee.set_age(if age < 0 {
                    age.saturating_add(occupant.ticks_in_hive).min(0)
                } else {
                    age.saturating_sub(occupant.ticks_in_hive).max(0)
                });
            }
            bee.mob_entity.love_ticks.store(
                bee.mob_entity
                    .love_ticks
                    .load(Relaxed)
                    .saturating_sub(occupant.ticks_in_hive)
                    .max(0),
                Relaxed,
            );
            let flower = *self
                .flower_pos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if flower.is_some()
                && bee.flower_pos.load().is_none()
                && world
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .next_f32()
                    < 0.9
            {
                bee.flower_pos.store(flower);
            }
            if reason == BeeReleaseStatus::HoneyDelivered {
                bee.set_has_nectar(false);
                bee.crops_grown_since_pollination.store(0, Relaxed);
                if props.honey_level < 5 {
                    let increase = if world
                        .random
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .next_bounded_i32(100)
                        == 0
                    {
                        2
                    } else {
                        1
                    };
                    let mut honey = props;
                    honey.honey_level = (props.honey_level + increase).min(5);
                    world.set_block_state(
                        &self.position,
                        honey.to_state_id(state.id.to_block()),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
            }
            let size = entity.get_entity().entity_dimension.load();
            let delta = if blocked {
                0.0
            } else {
                0.55 + f64::from(size.width / 2.0)
            };
            entity.get_entity().set_pos(Vector3::new(
                f64::from(self.position.0.x) + 0.5 + delta * f64::from(facing.x),
                f64::from(self.position.0.y) + 0.5 - f64::from(size.height / 2.0),
                f64::from(self.position.0.z) + 0.5 + delta * f64::from(facing.z),
            ));
        }
        world.play_sound(
            Sound::BlockBeehiveExit,
            SoundCategory::Blocks,
            &self.position.to_centered_f64(),
        );
        world.emit_game_event_from_entity(
            "block_change",
            self.position.to_centered_f64(),
            Some(entity.as_ref()),
            Some(world.get_block_state_id(&self.position)),
        );
        world.spawn_initialized_entity(entity.clone());
        world
            .get_entity_by_id(entity.get_entity().entity_id)
            .map(|_| entity)
    }
}
