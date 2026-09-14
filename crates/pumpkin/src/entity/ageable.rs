use pumpkin_data::tracked_data;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering::Relaxed};

use crate::entity::mob::Mob;

pub const BABY_START_AGE: i32 = -24000;
pub const FORCED_AGE_PARTICLE_TICKS: i32 = 40;

pub struct AgeableData {
    pub forced_age: AtomicI32,
    pub forced_age_timer: AtomicI32,
    pub age_locked: AtomicBool,
    pub age_lock_particle_timer: AtomicI32,
}

impl Default for AgeableData {
    fn default() -> Self {
        Self {
            forced_age: AtomicI32::new(0),
            forced_age_timer: AtomicI32::new(0),
            age_locked: AtomicBool::new(false),
            age_lock_particle_timer: AtomicI32::new(0),
        }
    }
}

pub trait AgeableMob: Mob {
    fn get_ageable_data(&self) -> &AgeableData;

    fn get_baby_start_age(&self) -> i32 {
        if self.get_entity().entity_type.resource_name == "sniffer" {
            -48000
        } else {
            BABY_START_AGE
        }
    }

    fn is_baby(&self) -> bool {
        self.can_be_a_baby() && self.get_mob_entity().living_entity.entity.age.load(Relaxed) < 0
    }

    fn set_baby(&self, baby: bool) {
        if self.can_be_a_baby() {
            self.set_age(if baby { self.get_baby_start_age() } else { 0 });
        }
    }

    fn get_age(&self) -> i32 {
        self.get_mob_entity().living_entity.entity.age.load(Relaxed)
    }

    fn set_age(&self, new_age: i32) {
        let mob = self.get_mob_entity();
        let entity = &mob.living_entity.entity;
        let old_age = entity.age.swap(new_age, Relaxed);

        if (old_age < 0 && new_age >= 0) || (old_age >= 0 && new_age < 0) {
            let is_baby = self.can_be_a_baby() && new_age < 0;
            entity.set_synced_data(tracked_data::ageable_mob::DATA_BABY_ID, is_baby);
            crate::entity::baby_dimensions::refresh(&mob.living_entity, is_baby);
        }
    }

    fn is_age_locked(&self) -> bool {
        self.get_ageable_data().age_locked.load(Relaxed)
    }

    fn set_age_locked(&self, locked: bool) {
        self.get_ageable_data().age_locked.store(locked, Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::ageable_mob::AGE_LOCKED, locked);
    }

    fn can_age_up(&self) -> bool {
        self.is_baby() && !self.is_age_locked()
    }

    fn age_up(&self, seconds: i32, forced: bool) {
        apply_age_up(
            self.get_ageable_data(),
            self.get_age(),
            seconds,
            forced,
            |age| {
                self.set_age(age);
            },
        );
    }

    #[must_use]
    fn get_speed_up_seconds_when_feeding(ticks_until_adult: i32) -> i32
    where
        Self: Sized,
    {
        feeding_speedup_seconds(ticks_until_adult)
    }

    fn write_ageable_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        if self.can_be_a_baby() {
            nbt.put_int("Age", self.get_age());
            nbt.put_int(
                "ForcedAge",
                self.get_ageable_data().forced_age.load(Relaxed),
            );
            nbt.put_bool("AgeLocked", self.is_age_locked());
        }
    }

    fn read_ageable_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        if self.can_be_a_baby() {
            self.set_age(nbt.get_int("Age").unwrap_or(0));
            self.get_ageable_data()
                .forced_age
                .store(nbt.get_int("ForcedAge").unwrap_or(0), Relaxed);
            self.set_age_locked(nbt.get_bool("AgeLocked").unwrap_or(false));
        }
    }

    fn can_be_a_baby(&self) -> bool {
        crate::entity::baby_dimensions::can_be_baby(self.get_entity().entity_type.resource_name)
    }

    fn ageable_ai_step(&self) {
        if self.get_mob_entity().living_entity.health.load() <= 0.0
            || self.get_entity().is_removed()
        {
            return;
        }
        if self.can_age_up() {
            let age = self.get_age() + 1;
            self.set_age(age);
        } else if self.get_age() > 0 {
            let age = self.get_age() - 1;
            self.set_age(age);
        }
    }
}

/// Java AgeableMob.ageUp, including Java's wrapping int arithmetic and the
/// intermediate setAge call (which can cross the baby/adult metadata boundary).
fn apply_age_up(
    data: &AgeableData,
    old_age: i32,
    seconds: i32,
    forced: bool,
    mut set_age: impl FnMut(i32),
) {
    let age = old_age.wrapping_add(seconds.wrapping_mul(20)).min(0);
    let delta = age.wrapping_sub(old_age);
    set_age(age);
    if forced {
        data.forced_age.fetch_add(delta, Relaxed);
        if data.forced_age_timer.load(Relaxed) == 0 {
            data.forced_age_timer
                .store(FORCED_AGE_PARTICLE_TICKS, Relaxed);
        }
    }
    if age == 0 {
        set_age(data.forced_age.load(Relaxed));
    }
}

pub fn feeding_speedup_seconds(ticks_until_adult: i32) -> i32 {
    (ticks_until_adult as f32 / 20.0 * 0.1) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_oracle_age_up_preserves_writes_forced_age_and_overflow() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/age-java-26.2.json")).unwrap();
        for c in fixture["growth"].as_array().unwrap() {
            let int = |key: &str| c[key].as_i64().unwrap() as i32;
            let data = AgeableData::default();
            data.forced_age.store(int("previous_forced_age"), Relaxed);
            data.forced_age_timer.store(int("previous_timer"), Relaxed);
            let mut writes = Vec::new();
            apply_age_up(
                &data,
                int("age"),
                int("seconds"),
                c["forced"].as_bool().unwrap(),
                |age| writes.push(age),
            );
            let expected: Vec<i32> = serde_json::from_value(c["writes"].clone()).unwrap();
            assert_eq!(writes, expected, "{c}");
            assert_eq!(*writes.last().unwrap(), int("expected_age"), "{c}");
            assert_eq!(
                data.forced_age.load(Relaxed),
                int("expected_forced_age"),
                "{c}"
            );
            assert_eq!(
                data.forced_age_timer.load(Relaxed),
                int("expected_timer"),
                "{c}"
            );
        }
    }

    #[test]
    fn java_oracle_feeding_rounds_to_whole_seconds() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/age-java-26.2.json")).unwrap();
        for c in fixture["feeding"].as_array().unwrap() {
            assert_eq!(
                feeding_speedup_seconds(c["ticks"].as_i64().unwrap() as i32),
                c["seconds"].as_i64().unwrap() as i32,
                "{c}"
            );
        }
    }
}
