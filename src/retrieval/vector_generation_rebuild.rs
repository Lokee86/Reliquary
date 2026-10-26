use crate::archive_vector_store::ArchiveVectorStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::vector_generation_codec::{
    GenerationPayload, decode_format, decode_generation, decode_version,
};
use crate::vector_generation_store::VectorGenerationStore;
use crate::vector_generation_validation::{validate_generation_reference, vector_generation_id};
use crate::{Archive, VectorGeneration, VectorGenerationError};
use reliquary_container::ObjectRef;
use std::collections::{HashMap, HashSet};

pub(crate) struct VectorGenerationOpenState {
    pending: HashMap<ObjectRef, GenerationPayload>,
    versioned: HashSet<ObjectRef>,
    generations: Vec<VectorGeneration>,
    next_vector_version: u64,
    format_seen: bool,
}

impl VectorGenerationOpenState {
    pub(crate) fn new() -> Self {
        Self {
            pending: HashMap::new(),
            versioned: HashSet::new(),
            generations: Vec::new(),
            next_vector_version: 1,
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), VectorGenerationError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(VectorGenerationError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(version) = decode_version(payload)? {
            if version.vector_version != self.next_vector_version
                || version.global_version == 0
                || version.global_version > latest_global
                || self
                    .generations
                    .last()
                    .is_some_and(|last| last.global_version >= version.global_version)
                || !version.record.precedes(chunk)
            {
                return Err(VectorGenerationError::InvalidGenerationVersion);
            }
            let source = if let Some(source) = self.pending.remove(&version.record) {
                self.versioned.insert(version.record);
                source
            } else if self.versioned.contains(&version.record) {
                return Err(VectorGenerationError::InvalidGenerationVersion);
            } else {
                return Err(VectorGenerationError::InvalidGenerationVersion);
            };
            if vector_generation_id(
                source.compatibility_profile_id,
                source.archive_vector_id,
                source.source_archive_version,
            ) != source.id
            {
                return Err(VectorGenerationError::HashCollision);
            }
            self.generations.push(VectorGeneration {
                id: source.id,
                compatibility_profile_id: source.compatibility_profile_id,
                archive_vector_id: source.archive_vector_id,
                source_archive_version: source.source_archive_version,
                global_version: version.global_version,
                vector_version: version.vector_version,
            });
            self.next_vector_version = self
                .next_vector_version
                .checked_add(1)
                .ok_or(VectorGenerationError::VectorVersionExhausted)?;
            return Ok(());
        }
        if let Some(generation) = decode_generation(payload)? {
            self.pending.insert(chunk, generation);
        }
        Ok(())
    }

    pub(crate) fn finish(
        self,
        archive: &Archive,
        packed_vectors: &PackedVectorStore,
        archive_vectors: &ArchiveVectorStore,
        profiles: &CompatibilityProfileStore,
    ) -> Result<VectorGenerationStore, VectorGenerationError> {
        if !self.format_seen {
            return Err(VectorGenerationError::MissingFormat);
        }
        let mut store = VectorGenerationStore::empty();
        for generation in self.generations {
            validate_generation_reference(
                archive,
                packed_vectors,
                archive_vectors,
                profiles,
                &generation,
            )?;
            store.insert_rebuilt(generation)?;
        }
        Ok(store)
    }
}
