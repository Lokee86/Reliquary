//! Public and crate-internal entry points for live semantic Freshness.

pub use crate::public_semantic_operations::SemanticFreshnessReport;
use crate::{Cva, CvaError, VectorGenerationId};

impl Cva {
    /// Number of retained eligible fragments after the accepted semantic floor
    /// without a durable completion.
    pub fn semantic_freshness_pending_fragments(&self) -> Result<usize, CvaError> {
        self.runtime_semantic_freshness_pending_fragments()
    }

    /// Process at most `limit` uncompleted retained fragments against one generation.
    /// F8 is intrinsic; this is a scheduling surface, not an activation control.
    pub fn process_semantic_freshness(
        &mut self,
        generation: VectorGenerationId,
        limit: usize,
    ) -> Result<SemanticFreshnessReport, CvaError> {
        self.runtime_process_semantic_freshness(generation, limit)
    }
}

#[cfg(test)]
impl Cva {
    pub(crate) fn prepare_semantic_archive_batch(
        &mut self,
        profile_id: crate::CompatibilityProfileId,
    ) -> Result<Option<crate::semantic_archive_batch::SemanticArchiveBatch>, CvaError> {
        if let Some(generation) = self.runtime_current_vector_generation(profile_id) {
            self.process_semantic_freshness(generation.id, 64)?;
        }
        let read = crate::owner_lifecycle::semantic_freshness_operations::archive_read(&self.owner);
        crate::semantic_archive_batch::prepare_batch(
            &read,
            profile_id,
            self.runtime_current_vector_generation(profile_id),
        )
    }

    pub(crate) fn commit_semantic_archive_batch(
        &mut self,
        batch: crate::semantic_archive_batch::SemanticArchiveBatch,
        vectors: Vec<Vec<f32>>,
    ) -> Result<(), CvaError> {
        let current = self.runtime_current_vector_generation(batch.profile.id);
        if let Some(plan) = crate::semantic_archive_batch::pack_batch(batch, vectors, current)? {
            let generation = crate::owner_lifecycle::vector_publication::publish_semantic_archive(
                &mut self.owner.container,
                &self.owner.archive,
                &self.owner.compatibility_profiles,
                &mut self.owner.packed_vectors,
                &mut self.owner.archive_vectors,
                &mut self.owner.vector_generations,
                plan,
            )?;
            if self.runtime_owner_uuid().is_some() {
                self.process_semantic_freshness(generation.id, 64)
                    .map_err(|e| crate::VectorGenerationError::Freshness(e.to_string()))?;
            }
        }
        Ok(())
    }
}
