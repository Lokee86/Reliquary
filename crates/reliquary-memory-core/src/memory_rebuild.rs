use crate::memory_codec::{decode_body, decode_format};
use crate::memory_model::memory_body_id;
use crate::{MemoryError, MemoryRoutingMetadata};
use reliquary_container::{Container, ObjectRef};

pub use crate::memory_store::{
    MemoryGroupedBatch, MemoryGroupedBody, MemoryGroupedBodyEncoding, MemoryGroupedBodySpan,
};

pub struct MemoryOpenState {
    store: crate::memory_store::MemoryStore,
    pending_routing_metadata: Vec<MemoryRoutingMetadata>,
    format_seen: bool,
}

impl MemoryOpenState {
    pub fn new() -> Self {
        Self {
            store: crate::memory_store::MemoryStore::empty(),
            pending_routing_metadata: Vec::new(),
            format_seen: false,
        }
    }

    /// Current accepted Memory-local replay head, including native envelopes.
    /// This can be queried before EOF without finalizing pending physical state.
    pub fn memory_version(&self) -> u64 {
        self.store.memory_version()
    }

    pub fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
        if let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? {
            if !self.ingest_publication_envelope(envelope, latest_global_version)? {
                return Err(MemoryError::CorruptRecord(
                    "unsupported publication envelope body",
                ));
            }
            return Ok(());
        }
        if self.store.prune.ingest(chunk, payload)? {
            return Ok(());
        }
        if decode_format(payload)? {
            if self.format_seen {
                return Err(MemoryError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some((id, body)) = decode_body(payload)? {
            let (title, content) = decode_body_fields(&body)?;
            if memory_body_id(&title, &content) != id {
                return Err(MemoryError::CorruptBody);
            }
            self.store.insert_body(id, chunk);
            return Ok(());
        }
        if let Some(metadata) = crate::memory_routing_codec::decode(payload)? {
            self.pending_routing_metadata.push(metadata);
        }
        Ok(())
    }

    pub fn ingest_publication_envelope(
        &mut self,
        envelope: reliquary_container::publication_envelope::PublicationEnvelope<'_>,
        latest_global_version: u64,
    ) -> Result<bool, MemoryError> {
        use crate::memory_compact_codec::publication::{self, MemoryPublication};
        let Some(publication) = publication::decode(envelope.body, envelope.global_version)? else {
            return Ok(false);
        };
        if envelope.global_version > latest_global_version {
            return Err(MemoryError::InvalidVersion);
        }
        match publication {
            MemoryPublication::Creation(creation) => {
                crate::memory_store::MemoryStore::validate_compact_record(&creation.record)?;
                self.store.insert_rebuilt(creation.record)?;
            }
            MemoryPublication::Delta(delta) => {
                let previous = self.store.current_record_for_replay(delta.id)?;
                let record = crate::memory_compact_apply::apply_delta(previous, &delta)?;
                crate::memory_store::MemoryStore::validate_compact_record(&record)?;
                self.store.insert_rebuilt(record)?;
            }
            MemoryPublication::Tombstone {
                id,
                revision,
                memory_version,
            } => {
                self.store.insert_tombstone_rebuilt(
                    crate::memory_codec::MemoryTombstone { id, revision },
                    envelope.global_version,
                    memory_version,
                )?;
            }
        }

        Ok(true)
    }

    /// Apply a fully decoded, typed Memory batch supplied by the owner that
    /// owns its grouped physical carrier.
    pub fn ingest_grouped_batch(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        batch: MemoryGroupedBatch,
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
        self.store
            .apply_grouped_batch(chunk, payload, batch, latest_global_version)
    }

    pub fn finish(
        mut self,
        container: &mut Container,
    ) -> Result<crate::memory_store::MemoryStore, MemoryError> {
        if !self.format_seen {
            return Err(MemoryError::MissingFormat);
        }

        self.store.validate_bodies()?;
        for metadata in self.pending_routing_metadata {
            self.store.insert_routing_metadata_rebuilt(metadata)?;
        }
        self.store.validate_routing_metadata(container)?;
        Ok(self.store)
    }
}

fn decode_body_fields(bytes: &[u8]) -> Result<(String, String), MemoryError> {
    if bytes.len() < 16 {
        return Err(MemoryError::CorruptBody);
    }
    let title_len = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
    let title_end = 8usize
        .checked_add(title_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content_len_end = title_end.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let title = bytes.get(8..title_end).ok_or(MemoryError::CorruptBody)?;
    let content_len = u64::from_le_bytes(
        bytes
            .get(title_end..content_len_end)
            .ok_or(MemoryError::CorruptBody)?
            .try_into()
            .unwrap(),
    ) as usize;
    let content_end = content_len_end
        .checked_add(content_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content = bytes
        .get(content_len_end..content_end)
        .ok_or(MemoryError::CorruptBody)?;
    if content_end != bytes.len() {
        return Err(MemoryError::CorruptBody);
    }
    Ok((
        String::from_utf8(title.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
        String::from_utf8(content.to_vec()).map_err(|_| MemoryError::InvalidUtf8)?,
    ))
}
