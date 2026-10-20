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
}

impl RelationshipOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::relationship_store::RelationshipStore::empty(),
            pending_records: HashMap::new(),
            pending_tombstones: HashMap::new(),
            format_seen: false,
            compact_seen: false,
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
                self.store.pruned_revisions.insert(identity);
            }
        }
        Ok(())
    }

    pub(crate) fn ingest_publication_envelope(
        &mut self,
        envelope: crate::container::publication_envelope::PublicationEnvelope<'_>,
        latest_global_version: u64,
    ) -> Result<bool, RelationshipError> {
        use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
        let Some(publication) = publication::decode(envelope.body, envelope.global_version)? else {
            return Ok(false);
        };
        if envelope.global_version > latest_global_version {
            return Err(RelationshipError::InvalidVersion);
        }
        match publication {
            RelationshipPublication::Creation(creation) => {
                let identity = (creation.record.id, creation.record.revision);
                let pruned = creation.description_proof.is_some();
                self.store.insert_rebuilt(creation.record)?;
                if pruned {
                    self.store.pruned_revisions.insert(identity);
                }
            }
            RelationshipPublication::Delta(delta) => {
                let previous = self.store.current_record_for_replay(delta.id)?;
                let record = crate::relationship_compact_apply::apply_delta(previous, &delta)?;
                let identity = (record.id, record.revision);
                self.store.insert_rebuilt(record)?;
                if delta.description_proof.is_some() {
                    self.store.pruned_revisions.insert(identity);
                }
            }
            RelationshipPublication::Tombstone {
                id,
                revision,
                relationship_version,
            } => {
                self.store.insert_tombstone_rebuilt(
                    crate::relationship_codec::RelationshipTombstone { id, revision },
                    envelope.global_version,
                    relationship_version,
                )?;
            }
        }
        self.compact_seen = true;
        Ok(true)
    }

    pub(crate) fn finish(
        self,
        container: &crate::Container,
    ) -> Result<crate::relationship_store::RelationshipStore, RelationshipError> {
        // Unavailability is provisional until REL/PHY file-level validation
        // authenticates the exact terminal descriptor, manifest and history.
        if self.compact_seen && container.version() != (crate::FormatVersion { major: 2, minor: 0 })
        {
            return Err(RelationshipError::ConflictingFormat);
        }
        Ok(self.store)
    }
}

#[cfg(test)]
#[path = "relationship_publication_replay_tests.rs"]
mod publication_replay_tests;

#[cfg(test)]
#[path = "relationship_publication_prune_tests.rs"]
mod publication_prune_tests;

#[cfg(test)]
#[path = "relationship_publication_copy_tests.rs"]
mod publication_copy_tests;

#[cfg(test)]
#[path = "relationship_publication_history_tests.rs"]
mod publication_history_tests;

#[cfg(test)]
#[path = "relationship_publication_legacy_prune_tests.rs"]
mod publication_legacy_prune_tests;
