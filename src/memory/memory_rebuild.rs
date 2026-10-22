use crate::memory_codec::{decode_body, decode_format};
use crate::memory_model::memory_body_id;
use crate::{Container, MemoryError, MemoryRoutingMetadata, ObjectRef};

pub(crate) struct MemoryOpenState {
    store: crate::memory_store::MemoryStore,
    pending_routing_metadata: Vec<MemoryRoutingMetadata>,
    format_seen: bool,
}

impl MemoryOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::memory_store::MemoryStore::empty(),
            pending_routing_metadata: Vec::new(),
            format_seen: false,
        }
    }

    /// Current accepted Memory-local replay head, including native envelopes.
    /// This can be queried before EOF without finalizing pending physical state.
    pub(crate) fn memory_version(&self) -> u64 {
        self.store.memory_version()
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
        if let Some(envelope) = crate::container::publication_envelope::decode(payload)? {
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
        if let Some(publication) = crate::external_source::publication::decode(payload)? {
            self.store
                .apply_grouped_bodies(chunk, &publication.bodies)?;
            for record in &publication.records {
                if record.global_version == 0
                    || record.global_version > latest_global_version
                    || record.memory_version != self.store.memory_version() + 1
                {
                    return Err(MemoryError::InvalidVersion);
                }
                self.store.insert_rebuilt(record.clone())?;
            }
            self.pending_routing_metadata
                .extend(publication.routing.clone());
            self.store.apply_external_publication(&publication)?;
            return Ok(());
        }
        if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(MemoryError::CorruptRecord)?
        {
            self.store.apply_grouped_bodies(chunk, &completion.bodies)?;
            for record in completion.records {
                if record.global_version == 0 || record.global_version > latest_global_version {
                    return Err(MemoryError::InvalidVersion);
                }
                if record.memory_version != self.store.memory_version() + 1 {
                    return Err(MemoryError::InvalidVersion);
                }
                self.store.insert_rebuilt(record)?;
            }
            self.pending_routing_metadata
                .extend(completion.routing_metadata);
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

    pub(crate) fn ingest_publication_envelope(
        &mut self,
        envelope: crate::container::publication_envelope::PublicationEnvelope<'_>,
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

    pub(crate) fn finish(
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
