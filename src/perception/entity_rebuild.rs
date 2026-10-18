use crate::entity_codec::{
    decode_direct_tombstone, decode_format, decode_record, decode_tombstone, decode_version,
};
use crate::entity_model::EntityRecord;
use crate::{EntityError, ObjectRef};
use std::collections::HashMap;

pub(crate) struct EntityOpenState {
    store: crate::entity_store::EntityStore,
    pending_records: HashMap<ObjectRef, EntityRecord>,
    pending_tombstones: HashMap<ObjectRef, crate::entity_codec::EntityTombstone>,
    pending_direct_tombstones: HashMap<ObjectRef, crate::entity_codec::DirectEntityTombstone>,
    format_seen: bool,
}

impl EntityOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::entity_store::EntityStore::empty(),
            pending_records: HashMap::new(),
            pending_tombstones: HashMap::new(),
            pending_direct_tombstones: HashMap::new(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), EntityError> {
        if decode_format(payload) {
            if self.format_seen {
                return Err(EntityError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(record) = decode_record(payload)? {
            if payload.starts_with(b"CVAEPRN1") {
                self.store
                    .pruned_revisions
                    .insert((record.id, record.revision));
            }
            self.pending_records.insert(chunk, record);
            return Ok(());
        }
        if let Some(tombstone) = decode_tombstone(payload)? {
            self.pending_tombstones.insert(chunk, tombstone);
            return Ok(());
        }
        if let Some(tombstone) = decode_direct_tombstone(payload)? {
            self.pending_direct_tombstones.insert(chunk, tombstone);
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.global_version == 0 || version.global_version > latest_global_version {
                return Err(EntityError::InvalidVersion);
            }
            if let Some(mut record) = self.pending_records.remove(&version.record) {
                record.global_version = version.global_version;
                record.entity_version = version.entity_version;
                self.store.insert_rebuilt(record)?;
            } else if let Some(tombstone) = self.pending_direct_tombstones.remove(&version.record) {
                self.store.insert_direct_tombstone_rebuilt(
                    tombstone,
                    version.global_version,
                    version.entity_version,
                )?;
            } else if let Some(tombstone) = self.pending_tombstones.remove(&version.record) {
                self.store.insert_tombstone_rebuilt(
                    tombstone,
                    version.global_version,
                    version.entity_version,
                )?;
            } else {
                return Err(EntityError::InvalidVersion);
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<crate::entity_store::EntityStore, EntityError> {
        // Entity ownership was introduced after existing REL/PHY files existed.
        // Absence of the format marker therefore means an empty legacy Entity owner.
        Ok(self.store)
    }
}
