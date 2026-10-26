use crate::EntityError;
use crate::entity_codec::decode_format;
use reliquary_container::ObjectRef;

pub struct EntityOpenState {
    store: crate::entity_store::EntityStore,
    format_seen: bool,
}

impl EntityOpenState {
    pub fn new() -> Self {
        Self {
            store: crate::entity_store::EntityStore::empty(),
            format_seen: false,
        }
    }

    pub fn ingest(
        &mut self,
        _chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), EntityError> {
        if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? {
            self.ingest_publication_envelope(envelope, latest_global_version)?;
            return Ok(());
        }
        if decode_format(payload) {
            if self.format_seen {
                return Err(EntityError::ConflictingFormat);
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
    ) -> Result<bool, EntityError> {
        use crate::entity_compact_codec::publication::{self, EntityPublication};
        let Some(publication) = publication::decode(envelope.body, envelope.global_version)? else {
            return Ok(false);
        };
        if envelope.global_version > latest_global_version {
            return Err(EntityError::InvalidVersion);
        }
        match publication {
            EntityPublication::Creation(creation) => {
                let identity = (creation.record.id, creation.record.revision);
                let pruned = creation.description_proof.is_some();
                self.store.insert_rebuilt(creation.record)?;
                if pruned {
                    self.store.pruned_revisions.insert(identity);
                }
            }
            EntityPublication::Delta(delta) => {
                let previous = self.store.current_record_for_replay(delta.id)?;
                let record = crate::entity_compact_apply::apply_delta(previous, &delta)?;
                let identity = (record.id, record.revision);
                self.store.insert_rebuilt(record)?;
                if delta.description_proof.is_some() {
                    self.store.pruned_revisions.insert(identity);
                }
            }
            EntityPublication::DirectTombstone {
                id,
                revision,
                entity_version,
            } => {
                self.store.insert_direct_tombstone_rebuilt(
                    crate::entity_codec::DirectEntityTombstone { id, revision },
                    envelope.global_version,
                    entity_version,
                )?;
            }
            EntityPublication::MergeTombstone {
                id,
                revision,
                entity_version,
                replacement_id,
            } => {
                self.store.insert_tombstone_rebuilt(
                    crate::entity_codec::EntityTombstone {
                        id,
                        revision,
                        replacement_id,
                    },
                    envelope.global_version,
                    entity_version,
                )?;
            }
        }

        Ok(true)
    }

    pub fn finish(
        self,
        _container: &reliquary_container::Container,
    ) -> Result<crate::entity_store::EntityStore, EntityError> {
        // An owner without Entity publications may omit its format marker.
        Ok(self.store)
    }
}

