//! Shared pending save images: failed writes remain authoritative for subsequent reads.
use crate::{
    chunk::{ChunkData, format::anvil::SingleChunkDataSerializer, io::Dirtiable},
    level::SyncChunk,
};
use std::sync::{Arc, Mutex};

pub(crate) struct PendingWrite {
    source: SyncChunk,
    image: Mutex<Option<SyncChunk>>,
}

impl PendingWrite {
    pub fn new(source: SyncChunk) -> Self {
        Self {
            source,
            image: Mutex::new(None),
        }
    }

    pub fn snapshot(&self) -> Result<SyncChunk, String> {
        let mut image = self
            .image
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(image) = image.as_ref() {
            return Ok(image.clone());
        }
        // Claim before serializing. Later source mutations stay dirty, and this
        // fixed image keeps saved relative tick delays unchanged during retries.
        self.source.take_dirty();
        let result = (|| {
            let bytes = self.source.to_bytes().map_err(|e| e.to_string())?;
            let pos = pumpkin_util::math::vector2::Vector2::new(self.source.x, self.source.z);
            let captured = Arc::new(ChunkData::from_bytes(&bytes, pos).map_err(|e| e.to_string())?);
            captured.mark_dirty(true);
            Ok(captured)
        })();
        match result {
            Ok(captured) => {
                *image = Some(captured.clone());
                Ok(captured)
            }
            Err(error) => {
                self.failed();
                Err(error)
            }
        }
    }

    pub fn failed(&self) {
        self.source.mark_dirty(true);
    }

    pub fn read(&self) -> Result<SyncChunk, String> {
        let image = self.snapshot()?;
        let pos = pumpkin_util::math::vector2::Vector2::new(image.x, image.z);
        let bytes = image.to_bytes().map_err(|e| e.to_string())?;
        // A loaded chunk binds its tick queues to the world clock. Never bind the
        // retained save image itself: its delays must stay frozen until reload.
        Ok(Arc::new(
            ChunkData::from_bytes(&bytes, pos).map_err(|e| e.to_string())?,
        ))
    }
}
