use pumpkin_nbt::compound::NbtCompound;
use std::fs::{File, create_dir_all};
use std::io;
use std::path::PathBuf;
use tracing::{debug, error};
use uuid::Uuid;

/// Manages the storage and retrieval of player data from disk and memory cache.
///
/// This struct provides functions to load and save player data to/from NBT files,
/// with a memory cache to handle player disconnections temporarily.
pub struct PlayerDataStorage {
    /// Path to the directory where player data is stored
    data_path: PathBuf,
    /// Whether player data saving is enabled
    save_enabled: bool,
    save_generations:
        std::sync::Mutex<std::collections::HashMap<Uuid, std::sync::Arc<std::sync::Mutex<u64>>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerDataError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("NBT error: {0}")]
    Nbt(String),
}

impl PlayerDataStorage {
    /// Creates a new `PlayerDataStorage` with the specified data path and cache expiration time.
    pub fn new(data_path: impl Into<PathBuf>, enabled: bool) -> Self {
        let path = data_path.into();
        if !path.exists()
            && let Err(e) = create_dir_all(&path)
        {
            error!(
                "Failed to create player data directory at {}: {e}",
                path.display()
            );
        }

        Self {
            data_path: path,
            save_enabled: enabled,
            save_generations: std::sync::Mutex::default(),
        }
    }

    #[must_use]
    pub const fn get_data_path(&self) -> &PathBuf {
        &self.data_path
    }

    #[must_use]
    pub const fn is_save_enabled(&self) -> bool {
        self.save_enabled
    }

    pub const fn set_save_enabled(&mut self, enabled: bool) {
        self.save_enabled = enabled;
    }

    /// Returns the path for a player's data file based on their UUID.
    #[must_use]
    pub fn get_player_data_path(&self, uuid: &Uuid) -> PathBuf {
        self.get_data_path().join(format!("{uuid}.dat"))
    }

    /// Loads player data from NBT file or cache.
    ///
    /// This function first checks if player data exists in the cache.
    /// If not, it attempts to load the data from a .dat file on disk.
    ///
    /// # Arguments
    ///
    /// * `uuid` - The UUID of the player to load data for.
    ///
    /// # Returns
    ///
    /// A Result containing either the player's NBT data or an error.
    pub fn load_player_data(&self, uuid: &Uuid) -> Result<(bool, NbtCompound), PlayerDataError> {
        // If player data saving is disabled, return empty data
        if !self.is_save_enabled() {
            return Ok((false, NbtCompound::new()));
        }

        // If not in cache, load from disk
        let path = self.get_player_data_path(uuid);
        if !path.exists() {
            debug!("No player data file found for {uuid}");
            return Ok((false, NbtCompound::new()));
        }

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) => {
                error!("Failed to open player data file for {uuid}: {e}");
                return Err(PlayerDataError::Io(e));
            }
        };

        match pumpkin_nbt::nbt_compress::read_gzip_compound_tag(file) {
            Ok(nbt) => {
                debug!("Loaded player data for {uuid} from disk");
                Ok((true, nbt))
            }
            Err(e) => {
                error!("Failed to read player data for {uuid}: {e}");
                Err(PlayerDataError::Nbt(e.to_string()))
            }
        }
    }

    fn generation_lock(&self, uuid: &Uuid) -> std::sync::Arc<std::sync::Mutex<u64>> {
        self.save_generations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(*uuid)
            .or_default()
            .clone()
    }

    /// Reserve before collecting NBT, so a delayed periodic snapshot cannot
    /// overwrite a newer disconnect/manual snapshot of the same player.
    pub fn reserve_save(&self, uuid: &Uuid) -> u64 {
        let state = self.generation_lock(uuid);
        let mut generation = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        *generation
    }

    pub fn save_player_data_versioned(
        &self,
        uuid: &Uuid,
        data: NbtCompound,
        generation: u64,
    ) -> Result<(), PlayerDataError> {
        let state = self.generation_lock(uuid);
        let current = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *current != generation {
            return Ok(());
        }
        self.write_player_data(uuid, data)
    }

    /// Saves player data to NBT file and updates cache.
    ///
    /// This function saves the player's data to a .dat file on disk and also
    /// updates the in-memory cache with the latest data.
    ///
    /// # Arguments
    ///
    /// * `uuid` - The UUID of the player to save data for.
    /// * `data` - The NBT compound data to save.
    ///
    /// # Returns
    ///
    /// A Result indicating success or the error that occurred.
    pub fn save_player_data(&self, uuid: &Uuid, data: NbtCompound) -> Result<(), PlayerDataError> {
        let generation = self.reserve_save(uuid);
        self.save_player_data_versioned(uuid, data, generation)
    }

    fn write_player_data(&self, uuid: &Uuid, data: NbtCompound) -> Result<(), PlayerDataError> {
        // Skip saving if disabled in config
        if !self.is_save_enabled() {
            return Ok(());
        }

        let path = self.get_player_data_path(uuid);

        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && let Err(e) = create_dir_all(parent)
        {
            error!("Failed to create player data directory for {uuid}: {e}");
            return Err(PlayerDataError::Io(e));
        }

        // Keep the previous complete file until compression finishes successfully.
        let temporary = path.with_extension("dat.tmp");
        match File::create(&temporary) {
            Ok(file) => {
                if let Err(e) = pumpkin_nbt::nbt_compress::write_gzip_compound_tag(data, file) {
                    error!("Failed to write compressed player data for {uuid}: {e}");
                    Err(PlayerDataError::Nbt(e.to_string()))
                } else {
                    std::fs::rename(&temporary, &path)?;
                    debug!("Saved player data for {uuid} to disk");
                    Ok(())
                }
            }
            Err(e) => {
                error!("Failed to create player data file for {uuid}: {e}");
                Err(PlayerDataError::Io(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_background_snapshot_cannot_overwrite_disconnect_or_leave_a_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let storage = PlayerDataStorage::new(dir.path(), true);
        let uuid = Uuid::from_u128(1);
        let old_generation = storage.reserve_save(&uuid);
        let mut old = NbtCompound::new();
        old.put_int("state", 1);
        let mut current = NbtCompound::new();
        current.put_int("state", 2);
        storage.save_player_data(&uuid, current).unwrap();
        storage
            .save_player_data_versioned(&uuid, old, old_generation)
            .unwrap();
        assert_eq!(
            storage.load_player_data(&uuid).unwrap().1.get_int("state"),
            Some(2)
        );
        assert!(
            !storage
                .get_player_data_path(&uuid)
                .with_extension("dat.tmp")
                .exists()
        );
        let other = Uuid::from_u128(2);
        let other_generation = storage.reserve_save(&other);
        let mut data = NbtCompound::new();
        data.put_int("state", 3);
        storage
            .save_player_data_versioned(&other, data, other_generation)
            .unwrap();
        assert_eq!(
            storage.load_player_data(&other).unwrap().1.get_int("state"),
            Some(3)
        );
    }
}
