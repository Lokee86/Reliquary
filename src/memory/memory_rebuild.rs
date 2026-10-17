use crate::memory_codec::{
    decode_body, decode_format, decode_record, decode_tombstone, decode_version,
};
use crate::memory_model::{MemoryRecord, memory_body_id};
use crate::{Container, MemoryError, MemoryRoutingMetadata, ObjectRef};
use std::collections::HashMap;

pub(crate) struct MemoryOpenState {
    store: crate::memory_store::MemoryStore,
    pending_records: HashMap<ObjectRef, MemoryRecord>,
    pending_tombstones: HashMap<ObjectRef, crate::memory_codec::MemoryTombstone>,
    pending_routing_metadata: Vec<MemoryRoutingMetadata>,
    format_seen: bool,
}

impl MemoryOpenState {
    pub(crate) fn new() -> Self {
        Self {
            store: crate::memory_store::MemoryStore::empty(),
            pending_records: HashMap::new(),
            pending_tombstones: HashMap::new(),
            pending_routing_metadata: Vec::new(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global_version: u64,
    ) -> Result<(), MemoryError> {
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
        if let Some(record) = decode_record(payload)? {
            self.pending_records.insert(chunk, record);
            return Ok(());
        }
        if let Some(tombstone) = decode_tombstone(payload)? {
            self.pending_tombstones.insert(chunk, tombstone);
            return Ok(());
        }
        if let Some(metadata) = crate::memory_routing_codec::decode(payload)? {
            self.pending_routing_metadata.push(metadata);
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.global_version == 0 || version.global_version > latest_global_version {
                return Err(MemoryError::InvalidVersion);
            }
            if let Some(tombstone) = self.pending_tombstones.remove(&version.record) {
                self.store.insert_tombstone_rebuilt(
                    tombstone,
                    version.global_version,
                    version.memory_version,
                )?;
                return Ok(());
            }
            let mut record = self
                .pending_records
                .remove(&version.record)
                .ok_or(MemoryError::InvalidVersion)?;
            if version.memory_version != self.store.memory_version() + 1 {
                return Err(MemoryError::InvalidVersion);
            }
            record.global_version = version.global_version;
            record.memory_version = version.memory_version;
            self.store.insert_rebuilt(record)?;
        }
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        container: &mut Container,
    ) -> Result<crate::memory_store::MemoryStore, MemoryError> {
        if !self.format_seen {
            return Err(MemoryError::MissingFormat);
        }
        if !self.pending_records.is_empty() {
            // Unversioned records are intentionally inert and may be left behind by an interrupted write.
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
