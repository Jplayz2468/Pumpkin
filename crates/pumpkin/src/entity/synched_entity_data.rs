use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::BufMut;
use pumpkin_data::meta_data_type::MetaDataType;
use pumpkin_data::tracked_data::{TrackedData, TrackedId};
use pumpkin_protocol::java::client::play::{Metadata, MetadataSerializer};
use pumpkin_protocol::ser::WritingError;
use pumpkin_util::version::JavaMinecraftVersion;

pub trait ErasedSerializer: Send + Sync {
    fn write(
        &self,
        index: TrackedId,
        r#type: MetaDataType,
        writer: &mut dyn std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError>;

    fn write_canonical(&self, index: TrackedId, r#type: MetaDataType) -> Vec<u8>;
}

struct SerializerHolder<T> {
    value: T,
}

impl<T: MetadataSerializer + Clone + Send + Sync + 'static> ErasedSerializer
    for SerializerHolder<T>
{
    fn write(
        &self,
        index: TrackedId,
        r#type: MetaDataType,
        writer: &mut dyn std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let meta = Metadata::new_raw(index, r#type, &self.value);
        meta.write(writer, version)
    }

    fn write_canonical(&self, _index: TrackedId, _type: MetaDataType) -> Vec<u8> {
        // Serialize the VALUE only, never through `Metadata::write`.
        //
        // `Metadata::write` emits nothing when the field's id resolves to 255 — the
        // sentinel for "this field does not exist in that protocol version". Canonical
        // bytes are only a change-detection key, so going through it made every value of
        // a version-absent field compare equal, and `set` would then decide nothing had
        // changed and never mark the item dirty. Fields that exist on 1.21.x but were
        // renamed in 26.x (creeper CHARGED, cat CAT_VARIANT, shulker COLOR, ...) stopped
        // updating on the versions where they DO exist, because the key was computed
        // against 26.2. The index and type are fixed per `TrackedData`, which is the map
        // key, so leaving them out of the comparison loses nothing.
        let mut buf = Vec::new();
        let _ = self
            .value
            .write_metadata(&mut buf, &JavaMinecraftVersion::V_26_2);
        buf
    }
}

pub struct DataItem {
    pub tracked: TrackedData,
    pub serializer: Box<dyn ErasedSerializer>,
    pub canonical_bytes: Vec<u8>,
    pub dirty: bool,
    pub is_default: bool,
}

pub struct SynchedEntityData {
    items: Mutex<HashMap<TrackedData, DataItem>>,
    is_dirty: AtomicBool,
}

impl Default for SynchedEntityData {
    fn default() -> Self {
        Self::new()
    }
}

impl SynchedEntityData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
            is_dirty: AtomicBool::new(false),
        }
    }

    pub fn define<T: MetadataSerializer + Clone + Send + Sync + 'static>(
        &self,
        tracked: TrackedData,
        value: T,
    ) {
        let holder = SerializerHolder { value };
        let canonical_bytes = holder.write_canonical(tracked.id, tracked.r#type);
        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.insert(
            tracked,
            DataItem {
                tracked,
                serializer: Box::new(holder),
                canonical_bytes,
                dirty: false,
                is_default: true,
            },
        );
    }

    pub fn set<T: MetadataSerializer + Clone + Send + Sync + 'static>(
        &self,
        tracked: TrackedData,
        value: T,
    ) -> bool {
        let holder = SerializerHolder { value };
        let new_canonical = holder.write_canonical(tracked.id, tracked.r#type);

        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(item) = items.get_mut(&tracked) {
            if item.canonical_bytes == new_canonical {
                return false;
            }
            item.canonical_bytes = new_canonical;
            item.serializer = Box::new(holder);
            item.dirty = true;
            item.is_default = false;
        } else {
            items.insert(
                tracked,
                DataItem {
                    tracked,
                    serializer: Box::new(holder),
                    canonical_bytes: new_canonical,
                    dirty: true,
                    is_default: false,
                },
            );
        }
        self.is_dirty.store(true, Ordering::Release);
        true
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }

    pub fn pack_dirty_for_version(&self, version: &JavaMinecraftVersion) -> Option<Box<[u8]>> {
        if !self.is_dirty.load(Ordering::Acquire) {
            return None;
        }

        let items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut buf = Vec::new();
        let mut has_any = false;

        for item in items.values() {
            if item.dirty {
                let before_len = buf.len();
                if item
                    .serializer
                    .write(item.tracked.id, item.tracked.r#type, &mut buf, version)
                    .is_ok()
                    && buf.len() > before_len
                {
                    has_any = true;
                }
            }
        }

        if !has_any {
            return None;
        }

        buf.put_u8(255);
        Some(buf.into_boxed_slice())
    }

    pub fn clear_dirty(&self) {
        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for item in items.values_mut() {
            item.dirty = false;
        }
        self.is_dirty.store(false, Ordering::Release);
    }

    pub fn get_non_default_values_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> Option<Box<[u8]>> {
        let items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut buf = Vec::new();
        let mut has_any = false;

        for item in items.values() {
            if !item.is_default {
                let before_len = buf.len();
                if item
                    .serializer
                    .write(item.tracked.id, item.tracked.r#type, &mut buf, version)
                    .is_ok()
                    && buf.len() > before_len
                {
                    has_any = true;
                }
            }
        }

        if !has_any {
            return None;
        }

        buf.put_u8(255);
        Some(buf.into_boxed_slice())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::tracked_data as td;

    const ABSENT: u8 = 255;

    /// The renamed-field pairs that [`crate::entity::Entity::set_synced_data_compat`]
    /// call sites rely on. Each must be the same field under two names — same metadata
    /// type, the historical one absent on 26.2, the 26.x one present — or a call site
    /// would be mirroring a value onto an unrelated field.
    #[test]
    fn renamed_field_pairs_are_two_names_for_one_field() {
        for (legacy, modern, what) in [
            (td::creeper::CHARGED, td::creeper::DATA_IS_POWERED, "creeper charged"),
            (td::cat::CAT_VARIANT, td::cat::DATA_VARIANT_ID, "cat variant"),
            (td::frog::VARIANT, td::frog::DATA_VARIANT_ID, "frog variant"),
            (td::chicken::VARIANT, td::chicken::DATA_VARIANT_ID, "chicken variant"),
            (td::shulker::COLOR, td::shulker::DATA_COLOR_ID, "shulker colour"),
            (td::wolf::COLLAR_COLOR, td::wolf::DATA_COLLAR_COLOR, "wolf collar"),
            (td::ocelot::TRUSTING, td::ocelot::DATA_TRUSTING, "ocelot trusting"),
            (td::sniffer::STATE, td::sniffer::DATA_STATE, "sniffer state"),
            (td::item_frame::ROTATION, td::item_frame::DATA_ROTATION, "item frame rotation"),
            (td::end_crystal::SHOW_BOTTOM, td::end_crystal::DATA_SHOW_BOTTOM, "end crystal base"),
        ] {
            assert_eq!(legacy.r#type, modern.r#type, "{what}: type must not change");
            assert_eq!(
                legacy.id.get(&JavaMinecraftVersion::V_26_2),
                ABSENT,
                "{what}: the historical name is expected to be absent on 26.2"
            );
            assert_ne!(
                modern.id.get(&JavaMinecraftVersion::V_26_2),
                ABSENT,
                "{what}: the 26.x name must resolve, or the field never reaches the client"
            );
            assert_eq!(
                modern.id.get(&JavaMinecraftVersion::V_1_21_5),
                ABSENT,
                "{what}: the 26.x name must not also claim an id on 1.21.5"
            );
            assert_ne!(
                legacy.id.get(&JavaMinecraftVersion::V_1_21_5),
                ABSENT,
                "{what}: the historical name must still resolve for older clients"
            );
        }
    }

    /// A field written under only its historical name produces no metadata at all for a
    /// 26.2 client. This is the bug that left charged creepers without their aura.
    #[test]
    fn a_historical_name_alone_reaches_no_26_2_client() {
        let data = SynchedEntityData::new();
        assert!(data.set(td::creeper::CHARGED, true));
        assert!(
            data.pack_dirty_for_version(&JavaMinecraftVersion::V_26_2)
                .is_none(),
            "writing only the pre-26.1 name must produce nothing for 26.2 — \
             which is why call sites write both names"
        );
    }

    /// Writing both names, as the call sites now do, reaches either client with exactly
    /// the id that client expects.
    #[test]
    fn writing_both_names_reaches_both_client_eras() {
        let data = SynchedEntityData::new();
        assert!(data.set(td::creeper::CHARGED, true));
        assert!(data.set(td::creeper::DATA_IS_POWERED, true));

        for version in [JavaMinecraftVersion::V_26_2, JavaMinecraftVersion::V_1_21_5] {
            let packed = data
                .pack_dirty_for_version(&version)
                .unwrap_or_else(|| panic!("{version:?} should receive the charged flag"));
            let expected = if version == JavaMinecraftVersion::V_26_2 {
                td::creeper::DATA_IS_POWERED.id.get(&version)
            } else {
                td::creeper::CHARGED.id.get(&version)
            };
            // Only the name that exists for this version is emitted; the other is
            // skipped, so the field id leads and is followed by the terminator.
            assert_eq!(packed.first().copied(), Some(expected));
            assert_eq!(packed.last().copied(), Some(ABSENT), "terminator");
        }
    }

    /// Canonical bytes are only a change-detection key. Deriving them through
    /// `Metadata::write` made every value of a version-absent field compare equal, so a
    /// change under the historical name was dropped and never marked dirty — older
    /// clients then stopped seeing updates to fields that do exist for them.
    #[test]
    fn changing_a_version_absent_field_still_counts_as_a_change() {
        let data = SynchedEntityData::new();
        assert!(data.set(td::creeper::CHARGED, true));
        data.clear_dirty();
        assert!(
            data.set(td::creeper::CHARGED, false),
            "flipping the value must register as a change"
        );
        assert!(
            !data.set(td::creeper::CHARGED, false),
            "but re-writing the same value must not"
        );
    }
}
