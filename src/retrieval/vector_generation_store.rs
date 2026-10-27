use crate::{
    CompatibilityProfileId, VectorGeneration, VectorGenerationError, VectorGenerationId,
    VectorGenerationStats,
};
use reliquary_container::Container;

pub(crate) struct VectorGenerationStore {
    inner: reliquary_vector_store::generation::VectorGenerationStore,
}
impl VectorGenerationStore {
    pub(crate) fn empty() -> Self {
        Self {
            inner: reliquary_vector_store::generation::VectorGenerationStore::empty(),
        }
    }
    pub(crate) fn from_inner(
        inner: reliquary_vector_store::generation::VectorGenerationStore,
    ) -> Self {
        Self { inner }
    }
    pub(crate) fn initialize(
        &self,
        container: &mut Container,
    ) -> Result<(), VectorGenerationError> {
        self.inner.initialize(container).map_err(Into::into)
    }
    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        profile: CompatibilityProfileId,
        archive: crate::ArchiveVectorId,
        source: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.inner
            .publish(container, profile, archive, source)
            .map_err(Into::into)
    }
    pub(crate) fn insert_rebuilt(
        &mut self,
        generation: VectorGeneration,
    ) -> Result<(), VectorGenerationError> {
        self.inner.insert_rebuilt(generation).map_err(Into::into)
    }
    pub(crate) fn generation(&self, id: VectorGenerationId) -> Option<VectorGeneration> {
        self.inner.generation(id)
    }
    pub(crate) fn current_generation(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.inner.current_generation(id)
    }
    pub(crate) fn generation_at(
        &self,
        id: CompatibilityProfileId,
        version: u64,
    ) -> Option<VectorGeneration> {
        self.inner.generation_at(id, version)
    }
    pub(crate) fn generations(&self) -> &[VectorGeneration] {
        self.inner.generations()
    }
    pub(crate) fn vector_version(&self) -> u64 {
        self.inner.vector_version()
    }
    pub(crate) fn stats(&self) -> VectorGenerationStats {
        self.inner.stats()
    }
}
