use std::collections::HashMap;

use pumpkin_util::identifier::Identifier;
use pumpkin_util::random::RandomImpl;
use pumpkin_util::random::xoroshiro128::Xoroshiro;

/// A single random sequence wrapper.
pub struct RandomSequence {
    pub(crate) rng: Xoroshiro,
}

impl RandomSequence {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            rng: Xoroshiro::from_seed(seed),
        }
    }

    pub fn random_between_inclusive(&mut self, min: i32, max: i32) -> i32 {
        self.rng
            .next_bounded_i32(max.wrapping_sub(min).wrapping_add(1))
            .wrapping_add(min)
    }
}

/// Persistent/runtime manager for server random sequences.
pub struct RandomSequences {
    salt: i32,
    include_world_seed: bool,
    include_sequence_id: bool,
    sequences: HashMap<String, RandomSequence>,
}

impl Default for RandomSequences {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomSequences {
    #[must_use]
    pub fn new() -> Self {
        Self {
            salt: 0,
            include_world_seed: true,
            include_sequence_id: true,
            sequences: HashMap::new(),
        }
    }

    fn create_sequence(
        sequence: &Identifier,
        world_seed: i64,
        salt: i32,
        include_world_seed: bool,
        include_sequence_id: bool,
    ) -> RandomSequence {
        let seed = (if include_world_seed { world_seed } else { 0 }) ^ i64::from(salt);
        let key = sequence.to_string();
        RandomSequence {
            rng: Xoroshiro::from_sequence_seed(
                seed as u64,
                include_sequence_id.then_some(key.as_str()),
            ),
        }
    }

    pub fn get_or_create(&mut self, sequence: &Identifier, world_seed: i64) -> &mut RandomSequence {
        let key = sequence.to_string();
        let salt = self.salt;
        let include_world_seed = self.include_world_seed;
        let include_sequence_id = self.include_sequence_id;
        self.sequences.entry(key).or_insert_with(|| {
            Self::create_sequence(
                sequence,
                world_seed,
                salt,
                include_world_seed,
                include_sequence_id,
            )
        })
    }

    pub fn reset(&mut self, sequence: &Identifier, world_seed: i64) {
        let key = sequence.to_string();
        let sequence = Self::create_sequence(
            sequence,
            world_seed,
            self.salt,
            self.include_world_seed,
            self.include_sequence_id,
        );
        self.sequences.insert(key, sequence);
    }

    pub fn reset_with_options(
        &mut self,
        sequence: &Identifier,
        world_seed: i64,
        salt: i32,
        include_world_seed: bool,
        include_sequence_id: bool,
    ) {
        let key = sequence.to_string();
        let sequence = Self::create_sequence(
            sequence,
            world_seed,
            salt,
            include_world_seed,
            include_sequence_id,
        );
        self.sequences.insert(key, sequence);
    }

    pub fn clear(&mut self) -> usize {
        let count = self.sequences.len();
        self.sequences.clear();
        count
    }

    pub const fn set_seed_defaults(
        &mut self,
        salt: i32,
        include_world_seed: bool,
        include_sequence_id: bool,
    ) {
        self.salt = salt;
        self.include_world_seed = include_world_seed;
        self.include_sequence_id = include_sequence_id;
    }

    #[must_use]
    pub fn get_sequence_keys(&self) -> Vec<String> {
        self.sequences.keys().cloned().collect()
    }
}

impl RandomSequences {
    pub fn load(folder: &std::path::Path) -> Result<Self, String> {
        let path = pumpkin_world::world_info::data_files::minecraft_data_dir(folder)
            .join("random_sequences.dat");
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let root =
            pumpkin_nbt::nbt_compress::read_gzip_compound_tag(file).map_err(|e| e.to_string())?;
        Self::from_nbt(
            root.get_compound("data")
                .ok_or("Missing random sequence data")?,
        )
    }

    pub fn from_nbt(data: &pumpkin_nbt::compound::NbtCompound) -> Result<Self, String> {
        let mut result = Self::new();
        result.salt = data.get_int("salt").ok_or("Missing random sequence salt")?;
        result.include_world_seed = data.get_bool("include_world_seed").unwrap_or(true);
        result.include_sequence_id = data.get_bool("include_sequence_id").unwrap_or(true);
        let sequences = data
            .get_compound("sequences")
            .ok_or("Missing sequences map")?;
        for (key, tag) in &sequences.child_tags {
            let pumpkin_nbt::tag::NbtTag::Compound(sequence) = tag else {
                return Err(format!("Invalid sequence {key}"));
            };
            let state = sequence
                .get_long_array("source")
                .ok_or_else(|| format!("Missing source for {key}"))?;
            let [lo, hi] = state else {
                return Err(format!("Invalid source length for {key}"));
            };
            let key = Identifier::parse(key)
                .map_err(|e| e.to_string())?
                .to_string();
            result.sequences.insert(
                key,
                RandomSequence {
                    rng: Xoroshiro::from_state(*lo as u64, *hi as u64),
                },
            );
        }
        Ok(result)
    }

    pub fn to_nbt(&self) -> pumpkin_nbt::compound::NbtCompound {
        use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
        let mut data = NbtCompound::new();
        data.put_int("salt", self.salt);
        data.put_bool("include_world_seed", self.include_world_seed);
        data.put_bool("include_sequence_id", self.include_sequence_id);
        let mut sequences = NbtCompound::new();
        for (key, sequence) in &self.sequences {
            let mut value = NbtCompound::new();
            value.put("source", NbtTag::LongArray(sequence.rng.state().to_vec()));
            sequences.put_compound(key, value);
        }
        data.put_compound("sequences", sequences);
        data
    }

    pub fn save(&self, folder: &std::path::Path) -> Result<(), String> {
        let dir = pumpkin_world::world_info::data_files::minecraft_data_dir(folder);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut root = pumpkin_nbt::compound::NbtCompound::new();
        root.put_int("DataVersion", 4903);
        root.put_compound("data", self.to_nbt());
        let temp = dir.join("random_sequences.dat.tmp");
        pumpkin_nbt::nbt_compress::write_gzip_compound_tag(
            root,
            std::fs::File::create(&temp).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temp, dir.join("random_sequences.dat")).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
    use serde_json::Value;

    fn draws(sequence: &mut RandomSequence) -> Vec<i64> {
        let mut output = Vec::new();
        for bound in [
            1,
            2,
            3,
            7,
            17,
            1_073_741_825,
            i32::MAX,
            1_073_741_825,
            31,
            1000,
        ] {
            for _ in 0..8 {
                output.push(i64::from(sequence.random_between_inclusive(0, bound - 1)));
            }
        }
        output.push(sequence.rng.next_i64());
        output
    }

    fn expected(case: &Value, key: &str) -> Vec<i64> {
        case[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap())
            .collect()
    }

    #[test]
    fn matches_java_sequences_salt_flags_large_bounds_and_restarts() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("random_sequence_cases.json")).unwrap();
        assert_eq!(cases.len(), 96);
        let key = Identifier::parse_static("minecraft:blocks/diamond_ore");
        for case in cases {
            let seed = case["seed"].as_i64().unwrap();
            let salt = case["salt"].as_i64().unwrap() as i32;
            let flags = case["flags"].as_u64().unwrap();
            let mut sequences = RandomSequences::new();
            sequences.set_seed_defaults(salt, flags & 1 != 0, flags & 2 != 0);
            assert_eq!(
                draws(sequences.get_or_create(&key, seed)),
                expected(&case, "draw")
            );
            let saved = sequences.to_nbt();
            let source = saved
                .get_compound("sequences")
                .unwrap()
                .get_compound(&key.to_string())
                .unwrap()
                .get_long_array("source")
                .unwrap();
            let java_state: Vec<i64> = case["saved"]["sequences"][key.to_string()]["source"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            assert_eq!(source, java_state);
            let mut loaded = RandomSequences::from_nbt(&saved).unwrap();
            assert_eq!(
                draws(loaded.get_or_create(&key, seed)),
                expected(&case, "resumed")
            );
            loaded.reset(&key, seed);
            assert_eq!(
                draws(loaded.get_or_create(&key, seed)),
                expected(&case, "reset")
            );
            loaded.reset_with_options(&key, seed, -99, false, true);
            assert_eq!(
                draws(loaded.get_or_create(&key, seed)),
                expected(&case, "override")
            );
            assert_eq!(loaded.clear(), 1);
            assert_eq!(loaded.clear(), 0);
        }
    }

    #[test]
    fn saves_and_loads_real_file_without_resetting_the_stream() {
        let directory = tempfile::tempdir().unwrap();
        let key = Identifier::parse_static("minecraft:chests/simple_dungeon");
        let mut sequences = RandomSequences::load(directory.path()).unwrap();
        sequences.set_seed_defaults(-27, false, true);
        draws(sequences.get_or_create(&key, 262));
        sequences.save(directory.path()).unwrap();
        let mut loaded = RandomSequences::load(directory.path()).unwrap();
        assert_eq!(
            draws(loaded.get_or_create(&key, 262)),
            draws(sequences.get_or_create(&key, 262))
        );
        let other = Identifier::parse_static("minecraft:blocks/gold_ore");
        assert_eq!(
            draws(loaded.get_or_create(&other, 262)),
            draws(sequences.get_or_create(&other, 262))
        );
        loaded.save(directory.path()).unwrap();
        let mut twice = RandomSequences::load(directory.path()).unwrap();
        assert_eq!(
            draws(twice.get_or_create(&key, 262)),
            draws(loaded.get_or_create(&key, 262))
        );
        let path = pumpkin_world::world_info::data_files::minecraft_data_dir(directory.path())
            .join("random_sequences.dat");
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(RandomSequences::load(directory.path()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(loaded.save(directory.path()).is_err());
    }

    #[test]
    fn loads_the_java_written_compressed_saved_data() {
        let directory = tempfile::tempdir().unwrap();
        let folder = pumpkin_world::world_info::data_files::minecraft_data_dir(directory.path());
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("random_sequences.dat"),
            include_bytes!("random_sequence_java.dat"),
        )
        .unwrap();
        let mut sequences = RandomSequences::load(directory.path()).unwrap();
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("random_sequence_cases.json")).unwrap();
        let case = cases
            .iter()
            .find(|case| case["seed"] == 262 && case["salt"] == 17 && case["flags"] == 3)
            .unwrap();
        let key = Identifier::parse_static("minecraft:blocks/diamond_ore");
        assert_eq!(
            draws(sequences.get_or_create(&key, 262)),
            expected(case, "resumed")
        );
        sequences.save(directory.path()).unwrap();
        let mut restored = RandomSequences::load(directory.path()).unwrap();
        assert_eq!(
            draws(sequences.get_or_create(&key, 262)),
            draws(restored.get_or_create(&key, 262))
        );
    }

    #[test]
    fn java_optional_defaults_and_invalid_state_length() {
        let mut data = NbtCompound::new();
        data.put_int("salt", 0);
        let mut entries = NbtCompound::new();
        let mut entry = NbtCompound::new();
        entry.put("source", NbtTag::LongArray(vec![0, 0]));
        entries.put_compound("test", entry.clone());
        data.put_compound("sequences", entries.clone());
        let loaded = RandomSequences::from_nbt(&data).unwrap();
        assert!(loaded.include_world_seed && loaded.include_sequence_id);
        assert_eq!(
            loaded.sequences["minecraft:test"].rng.state(),
            Xoroshiro::from_state(0, 0).state()
        );
        entry.put("source", NbtTag::LongArray(vec![1]));
        entries.put_compound("test", entry);
        data.put_compound("sequences", entries);
        assert!(RandomSequences::from_nbt(&data).is_err());
    }
}
