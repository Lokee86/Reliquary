use crate::RelationshipError;
use crate::relationship_codec::decode_format;
use reliquary_container::ObjectRef;

pub struct RelationshipOpenState {
    store: crate::relationship_store::RelationshipStore,
    format_seen: bool,
}

impl RelationshipOpenState {
    pub fn new() -> Self {
        Self {
            store: crate::relationship_store::RelationshipStore::empty(),
            format_seen: false,
        }
    }

    pub fn ingest(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), RelationshipError> {
        if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? {
            self.ingest_publication_envelope(envelope, latest_global_version)?;
            return Ok(());
        }
        if decode_format(payload) {
            if self.format_seen {
                return Err(RelationshipError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        Ok(())
    }

    pub fn ingest_publication_envelope(
        &mut self,
        envelope: reliquary_container::publication_envelope::PublicationEnvelope<'_>,
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

        Ok(true)
    }

    pub fn finish(
        self,
        _container: &reliquary_container::Container,
    ) -> Result<crate::relationship_store::RelationshipStore, RelationshipError> {
        // Unavailability is provisional until REL/PHY file-level validation
        // authenticates the exact terminal descriptor, manifest and history.

        Ok(self.store)
    }
}
