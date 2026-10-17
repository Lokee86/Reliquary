use crate::relationship_codec::{decode_format, decode_record, decode_tombstone, decode_version};
use crate::relationship_model::RelationshipRecord;
use crate::{ObjectRef, RelationshipError};
use std::collections::HashMap;

pub(crate) struct RelationshipOpenState {
    store: crate::relationship_store::RelationshipStore,
    pending_records: HashMap<ObjectRef, RelationshipRecord>,
    pending_tombstones: HashMap<ObjectRef, crate::relationship_codec::RelationshipTombstone>,
    format_seen: bool,
}

impl RelationshipOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::relationship_store::RelationshipStore::empty(),
            pending_records: HashMap::new(),
            pending_tombstones: HashMap::new(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), RelationshipError> {
        if decode_format(payload) {
            if self.format_seen {
                return Err(RelationshipError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(record) = decode_record(payload)? {
            self.pending_records.insert(chunk, record);
            return Ok(());
        }
        if let Some(tombstone) = decode_tombstone(payload)? {
            self.pending_tombstones.insert(chunk, tombstone);
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.global_version == 0 || version.global_version > latest_global_version {
                return Err(RelationshipError::InvalidVersion);
            }
            if let Some(tombstone) = self.pending_tombstones.remove(&version.record) {
                self.store.insert_tombstone_rebuilt(
                    tombstone,
                    version.global_version,
                    version.relationship_version,
                )?;
                return Ok(());
            }
            let mut record = self
                .pending_records
                .remove(&version.record)
                .ok_or(RelationshipError::InvalidVersion)?;
            record.global_version = version.global_version;
            record.relationship_version = version.relationship_version;
            self.store.insert_rebuilt(record)?;
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> crate::relationship_store::RelationshipStore {
        self.store
    }
}
