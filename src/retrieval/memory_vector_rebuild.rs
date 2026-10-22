use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_vector_codec::{decode_format, decode_object};
use crate::memory_vector_store::{MemoryVectorStore, memory_vector_id, validate_mapping_shape};
use crate::packed_vector_store::PackedVectorStore;
use crate::{MemoryBodyId, MemoryVectorError, MemoryVectorInfo, ObjectRef};

pub(crate) struct MemoryVectorOpenState {
    records: Vec<PendingMemoryVectors>,
    format_seen: bool,
}

struct PendingMemoryVectors {
    info: MemoryVectorInfo,
    chunk: ObjectRef,
    body_ids: Vec<MemoryBodyId>,
}

impl MemoryVectorOpenState {
    pub(crate) fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), MemoryVectorError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(MemoryVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        let Some(decoded) = decode_object(payload)? else {
            return Ok(());
        };
        if memory_vector_id(
            decoded.compatibility_profile_id,
            decoded.packed_vector_id,
            &decoded.memory_body_ids,
        ) != decoded.id
        {
            return Err(MemoryVectorError::HashCollision);
        }
        self.records.push(PendingMemoryVectors {
            info: MemoryVectorInfo {
                id: decoded.id,
                compatibility_profile_id: decoded.compatibility_profile_id,
                packed_vector_id: decoded.packed_vector_id,
                rows: u64::try_from(decoded.memory_body_ids.len())
                    .map_err(|_| MemoryVectorError::SizeOverflow)?,
            },
            chunk,
            body_ids: decoded.memory_body_ids,
        });
        Ok(())
    }

    pub(crate) fn referenced_body_ids(&self) -> impl Iterator<Item = MemoryBodyId> + '_ {
        self.records
            .iter()
            .flat_map(|record| record.body_ids.iter().copied())
    }

    pub(crate) fn finish(
        self,
        accepted_bodies: &std::collections::HashSet<MemoryBodyId>,
        profiles: &CompatibilityProfileStore,
        packed_vectors: &PackedVectorStore,
    ) -> Result<MemoryVectorStore, MemoryVectorError> {
        if !self.format_seen {
            return Err(MemoryVectorError::MissingFormat);
        }
        let mut store = MemoryVectorStore::default();
        for record in self.records {
            validate_mapping_shape(
                profiles,
                packed_vectors,
                record.info.compatibility_profile_id,
                record.info.packed_vector_id,
                &record.body_ids,
            )?;
            let mut seen = std::collections::HashSet::new();
            for body in &record.body_ids {
                if !accepted_bodies.contains(body) {
                    return Err(MemoryVectorError::MissingMemoryBody);
                }
                if !seen.insert(*body) {
                    return Err(MemoryVectorError::DuplicateMemoryBody);
                }
            }
            store.insert_rebuilt(record.info, record.chunk, &record.body_ids)?;
        }
        Ok(store)
    }
}
