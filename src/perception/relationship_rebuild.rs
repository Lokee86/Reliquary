use crate::relationship_codec::{decode_format, decode_record, decode_tombstone, decode_version};
use crate::relationship_model::RelationshipRecord;
use crate::{ObjectRef, RelationshipError};
use std::collections::HashMap;

enum PendingRelationshipRecord {
    Legacy(RelationshipRecord),
    Creation(crate::relationship_compact_contract::RelationshipCreation),
    Delta(crate::relationship_compact_contract::RelationshipDelta),
}

pub(crate) struct RelationshipOpenState {
    store: crate::relationship_store::RelationshipStore,
    pending_records: HashMap<ObjectRef, PendingRelationshipRecord>,
    pending_tombstones: HashMap<ObjectRef, crate::relationship_codec::RelationshipTombstone>,
    format_seen: bool,
    compact_seen: bool,
    compact_proof_seen: bool,
}

impl RelationshipOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::relationship_store::RelationshipStore::empty(),
            pending_records: HashMap::new(),
            pending_tombstones: HashMap::new(),
            format_seen: false,
            compact_seen: false,
            compact_proof_seen: false,
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
        if payload.starts_with(b"CVARNC") {
            let value = crate::relationship_compact_codec::decode_creation(payload)?;
            self.pending_records
                .insert(chunk, PendingRelationshipRecord::Creation(value));
            return Ok(());
        }
        if payload.starts_with(b"CVARND") {
            let value = crate::relationship_compact_codec::decode_delta(payload)?;
            self.pending_records
                .insert(chunk, PendingRelationshipRecord::Delta(value));
            return Ok(());
        }
        if let Some(record) = decode_record(payload)? {
            if payload.starts_with(b"CVARPRN1") {
                self.store
                    .pruned_revisions
                    .insert((record.id, record.revision));
            }
            self.pending_records
                .insert(chunk, PendingRelationshipRecord::Legacy(record));
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
            let pending = self
                .pending_records
                .remove(&version.record)
                .ok_or(RelationshipError::InvalidVersion)?;
            let pruned = match &pending {
                PendingRelationshipRecord::Creation(value) => value.description_proof.is_some(),
                PendingRelationshipRecord::Delta(value) => value.description_proof.is_some(),
                PendingRelationshipRecord::Legacy(_) => false,
            };
            let record = match pending {
                PendingRelationshipRecord::Legacy(mut record) => {
                    record.global_version = version.global_version;
                    record.relationship_version = version.relationship_version;
                    record
                }
                PendingRelationshipRecord::Creation(value) => {
                    if value.record.global_version != version.global_version
                        || value.record.relationship_version != version.relationship_version
                    {
                        return Err(RelationshipError::InvalidVersion);
                    }
                    self.compact_seen = true;
                    value.record
                }
                PendingRelationshipRecord::Delta(value) => {
                    if value.global_version != version.global_version
                        || value.relationship_version != version.relationship_version
                    {
                        return Err(RelationshipError::InvalidVersion);
                    }
                    let previous = self.store.current_record_for_replay(value.id)?;
                    let record = crate::relationship_compact_apply::apply_delta(previous, &value)?;
                    self.compact_seen = true;
                    record
                }
            };
            let identity = (record.id, record.revision);
            self.store.insert_rebuilt(record)?;
            if pruned {
                self.compact_proof_seen = true;
                self.store.pruned_revisions.insert(identity);
            }
        }
        Ok(())
    }

    pub(crate) fn finish(
        self,
        container: &crate::Container,
    ) -> Result<crate::relationship_store::RelationshipStore, RelationshipError> {
        // Step 118 supplies authenticated compact-proof file admission. Until
        // then, no accepted compact proof may return an unauthenticated owner.
        if self.compact_proof_seen {
            return Err(RelationshipError::CorruptRecord(
                "compact Relationship proof admission unavailable",
            ));
        }
        if self.compact_seen && container.version() != (crate::FormatVersion { major: 2, minor: 0 })
        {
            return Err(RelationshipError::ConflictingFormat);
        }
        Ok(self.store)
    }
}
