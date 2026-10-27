use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::vector_generation_store::VectorGenerationStore;
use crate::vector_generation_validation::validate_generation_reference;
use crate::{Archive, VectorGenerationError};
use reliquary_container::ObjectRef;
use reliquary_vector_store::ArchiveVectorStore;
use reliquary_vector_store::PackedVectorStore;

pub(crate) struct VectorGenerationOpenState {
    inner: reliquary_vector_store::generation::GenerationOpenState,
}

impl VectorGenerationOpenState {
    pub(crate) fn new() -> Self {
        Self {
            inner: reliquary_vector_store::generation::GenerationOpenState::new(),
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
        latest_global: u64,
    ) -> Result<(), VectorGenerationError> {
        self.inner
            .ingest(chunk, payload, latest_global)
            .map_err(Into::into)
    }

    pub(crate) fn finish(
        self,
        archive: &Archive,
        packed_vectors: &PackedVectorStore,
        archive_vectors: &ArchiveVectorStore,
        profiles: &CompatibilityProfileStore,
    ) -> Result<VectorGenerationStore, VectorGenerationError> {
        let generations = self.inner.finish_records()?;
        let mut store = VectorGenerationStore::empty();
        for generation in generations {
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
