//! Projectile's persistent owner-exit latch and once-per-tick range check.
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;

#[derive(Default)]
pub struct OwnerCollision {
    pub left_owner: bool,
    checked: bool,
}

impl OwnerCollision {
    pub fn begin_tick(&mut self) {
        self.checked = false;
    }

    pub fn check(&mut self, outside: impl FnOnce() -> bool) {
        if !self.left_owner && !self.checked {
            self.left_owner = outside();
            self.checked = true;
        }
    }

    pub fn allows_hit(&self, can_be_hit: bool, has_owner: bool, same_vehicle: bool) -> bool {
        can_be_hit && (!has_owner || self.left_owner || !same_vehicle)
    }

    pub fn read(&mut self, nbt: &NbtCompound) {
        self.left_owner = nbt.get_bool("LeftOwner").unwrap_or(false);
        self.checked = false;
    }

    pub fn write(&self, nbt: &mut NbtCompound) {
        if self.left_owner {
            nbt.put_bool("LeftOwner", true);
        }
    }
}

pub fn outside_owner_range(
    search: BoundingBox,
    owner_tree: impl IntoIterator<Item = (bool, BoundingBox)>,
) -> bool {
    !owner_tree
        .into_iter()
        .any(|(pickable, bounds)| pickable && search.intersects(&bounds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::math::vector3::Vector3;

    #[test]
    fn java_owner_exit_latch_and_vehicle_immunity() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("owner_cases.json")).unwrap();
        let mut state = OwnerCollision::default();
        for (index, case) in cases.iter().enumerate() {
            let vector = |value: &serde_json::Value, offset: usize| {
                Vector3::new(
                    f64::from_bits(value[offset].as_u64().unwrap()),
                    f64::from_bits(value[offset + 1].as_u64().unwrap()),
                    f64::from_bits(value[offset + 2].as_u64().unwrap()),
                )
            };
            let bounds =
                |value: &serde_json::Value| BoundingBox::new(vector(value, 0), vector(value, 3));
            if case[0].as_bool().unwrap() {
                state = OwnerCollision::default();
            }
            if case[1].as_bool().unwrap() {
                state.begin_tick();
            }
            let has_owner = case[2].as_bool().unwrap();
            let mut queries = 0;
            state.check(|| {
                if !has_owner {
                    return true;
                }
                queries += 1;
                outside_owner_range(
                    bounds(&case[3]),
                    case[4]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|member| (member[0].as_bool().unwrap(), bounds(&member[1]))),
                )
            });
            assert_eq!(
                state.left_owner,
                case[5].as_bool().unwrap(),
                "latch {index}"
            );
            assert_eq!(queries, case[6].as_u64().unwrap(), "queries {index}");
            let eligible = case[7].as_bool().unwrap();
            assert_eq!(
                state.allows_hit(eligible, has_owner, true),
                case[8].as_bool().unwrap(),
                "same vehicle {index}"
            );
            assert_eq!(
                state.allows_hit(eligible, has_owner, false),
                case[9].as_bool().unwrap(),
                "other vehicle {index}"
            );
        }
    }

    #[test]
    fn save_keeps_exit_latch_but_load_resets_transient_check() {
        let mut state = OwnerCollision::default();
        state.check(|| false);
        let mut nbt = NbtCompound::new();
        state.write(&mut nbt);
        assert_eq!(nbt.get_bool("LeftOwner"), None);
        state.read(&nbt);
        state.check(|| true);
        assert!(state.left_owner);
        state.write(&mut nbt);
        let mut restored = OwnerCollision::default();
        restored.read(&nbt);
        restored.begin_tick();
        restored.check(|| panic!("a latched projectile must not query again"));
        assert!(restored.left_owner);
        restored.read(&NbtCompound::new());
        assert!(!restored.left_owner);
    }
}
