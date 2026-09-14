//! Spawn finalization state. Metadata initialization and NBT loading are separate
//! entry points: loading a saved entity must never reroll its spawn group.

use crate::entity::{
    EntityBase,
    attributes::{Modifier, ModifierOperation},
};
use crate::world::World;
use pumpkin_data::attributes::Attributes;
use pumpkin_util::random::{RandomImpl, legacy_rand::LegacyRand};

pub const RANDOM_SPAWN_BONUS_ID: &str = "minecraft:random_spawn_bonus";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnReason {
    Natural,
    ChunkGeneration,
    Jockey,
    Conversion,
    Command,
    Triggered,
}

/// Object-safe primitive source so reference draw tapes and the live world use
/// the same decision code. No helper is allowed to create a hidden RNG.
pub trait SpawnRandom {
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;
}

impl SpawnRandom for LegacyRand {
    fn next_float(&mut self) -> f32 {
        self.next_f32()
    }
    fn next_double(&mut self) -> f64 {
        self.next_f64()
    }
}

/// Each draw locks briefly: never hold the RNG lock across entity callbacks.
pub struct WorldSpawnRandom<'a>(pub &'a World);
impl SpawnRandom for WorldSpawnRandom<'_> {
    fn next_float(&mut self) -> f32 {
        self.0
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f32()
    }
    fn next_double(&mut self) -> f64 {
        self.0
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_f64()
    }
}

pub struct SpawnContext<'a> {
    pub reason: SpawnReason,
    pub random: &'a mut dyn SpawnRandom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgeableGroupData {
    pub size: i32,
    pub should_spawn_baby: bool,
    pub baby_chance: f32,
}

impl AgeableGroupData {
    pub const fn new(should_spawn_baby: bool, baby_chance: f32) -> Self {
        Self {
            size: 0,
            should_spawn_baby,
            baby_chance,
        }
    }

    /// Call only once a spawn attempt has passed placement and mob checks.
    /// Java skips the first member's roll, and uses <= (not <) thereafter.
    pub fn finalize_member(&mut self, random: &mut dyn SpawnRandom) -> bool {
        let baby =
            self.should_spawn_baby && self.size > 0 && random.next_float() <= self.baby_chance;
        self.size = self.size.wrapping_add(1);
        baby
    }
}

impl Default for AgeableGroupData {
    fn default() -> Self {
        Self::new(true, 0.05)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpawnGroupData {
    Ageable(AgeableGroupData),
    Zombie(ZombieGroupData),
}

#[derive(Debug, PartialEq)]
struct BaseSpawnData {
    follow_bonus: Option<f64>,
    left_handed: bool,
}

fn base_spawn_data(random: &mut dyn SpawnRandom, existing_bonus: bool) -> BaseSpawnData {
    // Mob.finalizeSpawn uses triangle(0, 0.11485000000000001), then handedness.
    let follow_bonus = (!existing_bonus)
        .then(|| 0.0 + 0.114_850_000_000_000_01 * (random.next_double() - random.next_double()));
    BaseSpawnData {
        follow_bonus,
        left_handed: random.next_float() < 0.05,
    }
}

/// The verified default ageable family. Species with custom group subclasses or
/// additional group decisions are deliberately added by their own finalizers.
fn default_ageable_family(name: &str) -> bool {
    matches!(
        name,
        "armadillo"
            | "bee"
            | "mooshroom"
            | "happy_ghast"
            | "sniffer"
            | "cow"
            | "pig"
            | "chicken"
            | "sheep"
            | "cat"
            | "frog"
            | "goat"
            | "turtle"
            | "nautilus"
            | "camel"
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZombieGroupData {
    pub baby: bool,
    pub can_spawn_jockey: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ZombieSpawnEvent {
    Pickup(bool),
    Baby,
    FindChicken,
    CreateChicken,
    Doors(bool),
    Equipment,
}

/// The Zombie group phase runs AFTER Mob's base finalizer. Jockey callbacks
/// run immediately because creating a chicken consumes this same world stream.
fn zombie_group_phase(
    random: &mut dyn SpawnRandom,
    group: &mut Option<ZombieGroupData>,
    special: f32,
    conversion: bool,
    already_baby: bool,
    mut apply: impl FnMut(ZombieSpawnEvent, &mut dyn SpawnRandom),
) {
    if !conversion {
        let pickup = random.next_float() < 0.55 * special;
        apply(ZombieSpawnEvent::Pickup(pickup), random);
    }
    let data = group.get_or_insert_with(|| ZombieGroupData {
        baby: random.next_float() < 0.05,
        can_spawn_jockey: true,
    });
    if data.baby {
        apply(ZombieSpawnEvent::Baby, random);
    }
    if (already_baby || data.baby) && data.can_spawn_jockey {
        // Java compares a widened float with the DOUBLE literal 0.05 here.
        if f64::from(random.next_float()) < 0.05 {
            apply(ZombieSpawnEvent::FindChicken, random);
        } else if f64::from(random.next_float()) < 0.05 {
            apply(ZombieSpawnEvent::CreateChicken, random);
        }
    }
    let doors = random.next_float() < special * 0.1;
    apply(ZombieSpawnEvent::Doors(doors), random);
    if !conversion {
        apply(ZombieSpawnEvent::Equipment, random);
    }
}

pub(crate) fn apply_base_spawn(mob: &dyn crate::entity::mob::Mob, random: &mut dyn SpawnRandom) {
    let living = &mob.get_mob_entity().living_entity;
    let has_bonus = living
        .attributes
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&Attributes::FOLLOW_RANGE.id)
        .is_some_and(|a| a.modifiers.iter().any(|m| m.id == RANDOM_SPAWN_BONUS_ID));
    let base = base_spawn_data(random, has_bonus);
    if let Some(amount) = base.follow_bonus {
        living.update_attribute(&Attributes::FOLLOW_RANGE, |a| {
            a.add_permanent_modifier(Modifier {
                id: RANDOM_SPAWN_BONUS_ID.to_owned(),
                amount,
                operation: ModifierOperation::MultiplyBase,
            });
        });
    }
    mob.get_mob_entity().set_left_handed(base.left_handed);
}

/// Returns whether this finalizer already initialized the mob's equipment.
pub fn finalize_spawn_group(
    entity: &std::sync::Arc<dyn EntityBase>,
    context: &mut SpawnContext<'_>,
    group: &mut Option<SpawnGroupData>,
) -> bool {
    let Some(mob) = entity.get_mob() else {
        return false;
    };
    let name = entity.get_entity().entity_type.resource_name;
    mob.mob_finalize_spawn(context.reason);
    if name == "warden" {
        return false;
    }
    if crate::entity::mob::zombie::is_zombie_family(name) {
        apply_base_spawn(mob, context.random);
        let world = entity.get_entity().world.load_full();
        let difficulty = crate::entity::mob::equipment::RegionalDifficulty::at(
            &world,
            entity.get_entity().pos.load(),
        );
        let mut data = match group.take() {
            Some(SpawnGroupData::Zombie(data)) => Some(data),
            _ => None,
        };
        zombie_group_phase(
            context.random,
            &mut data,
            difficulty.special_multiplier,
            context.reason == SpawnReason::Conversion,
            entity
                .get_entity()
                .age
                .load(std::sync::atomic::Ordering::Relaxed)
                < 0,
            |event, random| match event {
                ZombieSpawnEvent::Pickup(value) => mob.get_mob_entity().set_can_pick_up_loot(value),
                ZombieSpawnEvent::Baby => crate::entity::mob::zombie::set_baby(mob, true),
                ZombieSpawnEvent::FindChicken => try_chicken_jockey(entity, random, false),
                ZombieSpawnEvent::CreateChicken => try_chicken_jockey(entity, random, true),
                ZombieSpawnEvent::Doors(value) => {
                    if let Some(zombie) = mob.as_zombie_base() {
                        zombie.set_can_break_doors(value, mob);
                    }
                }
                ZombieSpawnEvent::Equipment => {
                    crate::entity::mob::equipment::equip_mob_on_spawn(entity.as_ref(), &world)
                }
            },
        );
        *group = data.map(SpawnGroupData::Zombie);
        return true;
    }
    let Some(ageable) = mob.as_ageable() else {
        return false;
    };
    if !default_ageable_family(name) {
        return false;
    }
    let data = group.get_or_insert_with(|| SpawnGroupData::Ageable(AgeableGroupData::default()));
    let SpawnGroupData::Ageable(data) = data else {
        return false;
    };
    if data.finalize_member(context.random) {
        ageable.set_age(ageable.get_baby_start_age());
    }
    apply_base_spawn(mob, context.random);
    false
}

fn try_chicken_jockey(
    rider: &std::sync::Arc<dyn EntityBase>,
    random: &mut dyn SpawnRandom,
    create: bool,
) {
    use crate::entity::mob::Mob;
    use pumpkin_data::entity::EntityType;
    let entity = rider.get_entity();
    let world = entity.world.load_full();
    let chicken: Option<std::sync::Arc<dyn EntityBase>> = if create {
        let chicken = crate::entity::passive::chicken::ChickenEntity::new(
            crate::entity::Entity::new(world.clone(), entity.pos.load(), &EntityType::CHICKEN),
        );
        chicken
            .get_mob_entity()
            .living_entity
            .entity
            .set_rotation(entity.yaw.load(), entity.pitch.load());
        Some(chicken)
    } else {
        let bounds = entity.bounding_box.load().expand(5.0, 3.0, 5.0);
        // Stable world traversal. Exact section-query ordering remains a separate
        // parity item when multiple chickens occupy the search volume.
        world
            .entities
            .load()
            .iter()
            .find(|candidate| {
                let base = candidate.get_entity();
                base.entity_type == &EntityType::CHICKEN
                    && !base.is_removed()
                    && !base.has_passengers()
                    && !base.has_vehicle()
                    && bounds.intersects(&base.bounding_box.load())
            })
            .cloned()
    };
    if let Some(chicken) = chicken {
        if create {
            finalize_spawn_group(
                &chicken,
                &mut SpawnContext {
                    reason: SpawnReason::Jockey,
                    random,
                },
                &mut None,
            );
            chicken.init_data_tracker();
        }
        if let Some(mob) = chicken.get_mob()
            && let Some(chicken) = mob.as_chicken()
        {
            chicken
                .is_chicken_jockey
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        chicken
            .get_entity()
            .add_passenger(chicken.clone(), rider.clone());
        if create {
            world.spawn_entity_non_save(chicken);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Tape<'a> {
        draws: &'a [serde_json::Value],
        index: usize,
    }
    impl Tape<'_> {
        fn draw(&mut self, method: &str) -> f64 {
            let d = &self.draws[self.index];
            assert_eq!(
                d["method"].as_str(),
                Some(method),
                "wrong draw order at {}",
                self.index
            );
            self.index += 1;
            let bits: u64 = d["bits"].as_str().unwrap().parse().unwrap();
            if method == "float" {
                f64::from(f32::from_bits(bits as u32))
            } else {
                f64::from_bits(bits)
            }
        }
    }
    impl SpawnRandom for Tape<'_> {
        fn next_float(&mut self) -> f32 {
            self.draw("float") as f32
        }
        fn next_double(&mut self) -> f64 {
            self.draw("double")
        }
    }
    #[test]
    fn java_oracle_zombie_group_phase_and_draw_order() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/groups-java-26.2.json"))
                .unwrap();
        for c in fixture["zombie_group_phase"].as_array().unwrap() {
            let mut group = match c["mode"].as_i64().unwrap() {
                0 => None,
                mode => Some(ZombieGroupData {
                    baby: mode == 2,
                    can_spawn_jockey: c["jockey"].as_bool().unwrap(),
                }),
            };
            for (index, member) in c["members"].as_array().unwrap().iter().enumerate() {
                let mut random = Tape {
                    draws: member["draws"].as_array().unwrap(),
                    index: 0,
                };
                let base = base_spawn_data(&mut random, false);
                let mut baby = false;
                let mut pickup = false;
                let mut doors = false;
                let mut lookups = 0;
                let mut equipment = 0;
                zombie_group_phase(
                    &mut random,
                    &mut group,
                    c["special_multiplier"].as_f64().unwrap() as f32,
                    c["conversion"].as_bool().unwrap(),
                    false,
                    |event, _| match event {
                        ZombieSpawnEvent::Baby => baby = true,
                        ZombieSpawnEvent::Pickup(value) => pickup = value,
                        ZombieSpawnEvent::Doors(value) => doors = value,
                        ZombieSpawnEvent::FindChicken => lookups += 1,
                        ZombieSpawnEvent::CreateChicken => {
                            panic!("Reference cases have no new-chicken branch")
                        }
                        ZombieSpawnEvent::Equipment => equipment += 1,
                    },
                );
                assert_eq!(
                    (baby, pickup, doors, base.left_handed),
                    (
                        member["baby"].as_bool().unwrap(),
                        member["pickup"].as_bool().unwrap(),
                        member["doors"].as_bool().unwrap(),
                        member["left"].as_bool().unwrap()
                    ),
                    "member {index}: {c}"
                );
                assert_eq!(lookups, member["lookups"].as_i64().unwrap(), "{c}");
                assert_eq!(
                    equipment,
                    member["equipment_calls"].as_i64().unwrap(),
                    "{c}"
                );
                assert_eq!(equipment, member["enchant_calls"].as_i64().unwrap(), "{c}");
                let data = group.as_ref().unwrap();
                assert_eq!(
                    (data.baby, data.can_spawn_jockey),
                    (
                        member["group_baby"].as_bool().unwrap(),
                        member["group_jockey"].as_bool().unwrap()
                    )
                );
                assert_eq!(
                    random.index,
                    random.draws.len(),
                    "unused draws in member {index}: {c}"
                );
            }
        }
    }

    #[test]
    fn java_oracle_ageable_group_state_and_base_draw_order() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/groups-java-26.2.json"))
                .unwrap();
        for c in fixture["ageable"].as_array().unwrap() {
            let mut group = if c["initial_null"].as_bool().unwrap() {
                AgeableGroupData::default()
            } else {
                AgeableGroupData {
                    size: c["count"].as_i64().unwrap() as i32,
                    should_spawn_baby: c["babies"].as_bool().unwrap(),
                    baby_chance: c["chance"].as_f64().unwrap() as f32,
                }
            };
            let mut random = Tape {
                draws: c["draws"].as_array().unwrap(),
                index: 0,
            };
            let baby = group.finalize_member(&mut random);
            let base = base_spawn_data(&mut random, c["existing_bonus"].as_bool().unwrap());
            assert_eq!(
                if baby { -24000 } else { 0 },
                c["age"].as_i64().unwrap(),
                "{c}"
            );
            assert_eq!(base.left_handed, c["left"].as_bool().unwrap(), "{c}");
            assert_eq!(
                base.follow_bonus.unwrap_or(0.25).to_bits(),
                c["bonus_bits"].as_str().unwrap().parse::<u64>().unwrap(),
                "{c}"
            );
            assert_eq!(group.size, c["group_size"].as_i64().unwrap() as i32, "{c}");
            assert_eq!(
                group.should_spawn_baby,
                c["group_babies"].as_bool().unwrap(),
                "{c}"
            );
            assert_eq!(
                group.baby_chance,
                c["group_chance"].as_f64().unwrap() as f32,
                "{c}"
            );
            assert_eq!(random.index, random.draws.len(), "unused RNG draws: {c}");
        }
    }
}

/// Existing equipment implementation, now restricted to new entity insertion.
/// Keep this separate until each species has a reference-verified finalizer.
pub fn initialize_spawn_equipment(entity: &dyn EntityBase) {
    let Some(mob) = entity.get_mob() else {
        return;
    };
    let world = entity.get_entity().world.load();
    crate::entity::mob::equipment::equip_mob_on_spawn(entity, &world);
    if let Some(def) = crate::entity::mob::equipment::EQUIPMENT_REGISTRY
        .get(entity.get_entity().entity_type.resource_name)
        && def.can_pick_up_loot
    {
        let difficulty = crate::entity::mob::equipment::RegionalDifficulty::at(
            &world,
            entity.get_entity().pos.load(),
        );
        mob.get_mob_entity()
            .set_can_pick_up_loot(rand::random::<f32>() < 0.55 * difficulty.special_multiplier);
    }
}
