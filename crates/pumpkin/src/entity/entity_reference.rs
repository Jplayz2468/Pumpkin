//! UUID identity with lazy resolution and invalidation of removed cached entities.
use pumpkin_nbt::compound::NbtCompound;
use uuid::Uuid;

pub struct EntityReference<T> {
    pub uuid: Uuid,
    cached: Option<T>,
}

impl<T> EntityReference<T> {
    pub const fn new(uuid: Uuid) -> Self {
        Self { uuid, cached: None }
    }

    pub const fn with_cached(uuid: Uuid, cached: T) -> Self {
        Self {
            uuid,
            cached: Some(cached),
        }
    }

    pub fn resolve<R>(
        &mut self,
        retain: impl FnOnce(&T) -> Option<R>,
        lookup: impl FnOnce(Uuid) -> Option<(T, R)>,
    ) -> Option<R> {
        if let Some(cached) = &self.cached
            && let Some(entity) = retain(cached)
        {
            return Some(entity);
        }
        self.cached = None;
        let (cached, entity) = lookup(self.uuid)?;
        self.cached = Some(cached);
        Some(entity)
    }

    pub fn read(nbt: &NbtCompound, key: &str) -> Option<Self> {
        nbt.get_uuid(key).map(Self::new)
    }

    pub fn write(&self, nbt: &mut NbtCompound, key: &str) {
        nbt.put_uuid(key, self.uuid);
    }
}

pub type EntityOwner = EntityReference<std::sync::Weak<dyn super::EntityBase>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_reference_cache_invalidation_and_uuid_codec() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("entity_reference_cases.json")).unwrap();
        let main = Uuid::from_u128(0x123456789abcdef0fedcba9876543210);
        let identities = [
            main,
            main,
            Uuid::from_u128(0x10000000000000012000000000000002),
        ];
        let mut reference = EntityReference::<usize>::new(main);
        for (index, case) in cases.iter().enumerate() {
            let target = case[1].as_u64().unwrap() as usize;
            match case[0].as_u64().unwrap() {
                0 => reference = EntityReference::new(main),
                1 => reference = EntityReference::with_cached(identities[target], target),
                _ => {}
            }
            let removed = |id: usize| case[3][id].as_bool().unwrap();
            let mut queries = 0;
            let result = reference.resolve(
                |&id| (!removed(id)).then_some(id),
                |uuid| {
                    queries += 1;
                    let id = usize::try_from(case[2].as_i64().unwrap()).ok()?;
                    (identities[id] == uuid && !removed(id)).then_some((id, id))
                },
            );
            assert_eq!(
                result.map_or(-1, |id| id as i64),
                case[4].as_i64().unwrap(),
                "target {index}"
            );
            assert_eq!(queries, case[5].as_u64().unwrap(), "queries {index}");
            let mut nbt = NbtCompound::new();
            reference.write(&mut nbt, "Owner");
            let expected: Vec<i32> = case[6]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            assert_eq!(
                nbt.get_int_array("Owner").unwrap(),
                expected,
                "codec {index}"
            );
            let restored = EntityReference::<usize>::read(&nbt, "Owner").unwrap();
            assert_eq!(restored.uuid, reference.uuid);
            assert!(restored.cached.is_none());
        }
    }

    #[test]
    fn unresolved_identity_survives_save_and_owner_can_reappear_with_a_new_runtime_id() {
        let uuid = Uuid::from_u128(7);
        let mut owner = EntityReference::<i32>::new(uuid);
        assert_eq!(owner.resolve(|_| None::<i32>, |_| None), None);
        let mut nbt = NbtCompound::new();
        owner.write(&mut nbt, "Owner");
        let mut owner = EntityReference::<i32>::read(&nbt, "Owner").unwrap();
        assert_eq!(
            owner.resolve(|_| None, |id| (id == uuid).then_some((42, 42))),
            Some(42)
        );
        assert_eq!(
            owner.resolve(|_| None, |id| (id == uuid).then_some((99, 99))),
            Some(99)
        );
        assert!(EntityReference::<i32>::read(&NbtCompound::new(), "Owner").is_none());
        nbt.put("Owner", pumpkin_nbt::tag::NbtTag::IntArray(vec![1, 2, 3]));
        assert!(EntityReference::<i32>::read(&nbt, "Owner").is_none());
    }
}
