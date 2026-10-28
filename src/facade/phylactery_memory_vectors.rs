use crate::{
    CompatibilityProfileId, EmbeddingEndpoint, EmbeddingMode, MemoryBodyId,
    MemoryVectorBuildResult, MemoryVectorError, MemoryVectorId, MemoryVectorInfo,
    MemoryVectorLocation, MemoryVectorSet, MemoryVectorStats, PackedVectorId, Phylactery,
};
use reliquary_vectors::compatibility_profile_probe::verify_endpoint;

impl Phylactery {
    pub fn put_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        packed_vector_id: PackedVectorId,
        memory_body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.memory_vectors.put(
            &mut self.container,
            &self.memories,
            &self.compatibility_profiles,
            &self.packed_vectors,
            compatibility_profile_id,
            packed_vector_id,
            memory_body_ids,
        )
    }

    pub fn memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.memory_vectors.get(&mut self.container, id)
    }

    pub fn memory_vector_location(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        memory_body_id: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.memory_vectors
            .location(compatibility_profile_id, memory_body_id)
    }

    pub fn memory_vector_infos(&self) -> Vec<MemoryVectorInfo> {
        self.memory_vectors.infos()
    }

    pub fn memory_vector_stats(&self) -> MemoryVectorStats {
        self.memory_vectors.stats()
    }

    pub fn build_missing_memory_vectors(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<MemoryVectorBuildResult, MemoryVectorError> {
        let profile = self
            .compatibility_profiles
            .get_active(compatibility_profile_id)
            .cloned()
            .ok_or(MemoryVectorError::MissingProfile)?;
        if !verify_endpoint(&profile, endpoint)?.compatible {
            return Err(MemoryVectorError::IncompatibleEndpoint);
        }

        let body_ids = self.memories.current_body_ids();
        if body_ids.is_empty() {
            return Err(MemoryVectorError::EmptyPopulation);
        }
        let missing = self
            .memory_vectors
            .missing_body_ids(compatibility_profile_id, &body_ids);
        let already_present = body_ids.len().saturating_sub(missing.len());
        if missing.is_empty() {
            return Ok(MemoryVectorBuildResult {
                created_set: None,
                embedded: 0,
                already_present,
            });
        }

        let mut texts = Vec::with_capacity(missing.len());
        for body_id in &missing {
            texts.push(
                self.memories
                    .body_embedding_text(&mut self.container, *body_id)?,
            );
        }
        let vectors = endpoint.embed(EmbeddingMode::Document, &texts)?;
        let packed = reliquary_vectors::pack_embedding_batch(&profile, vectors, texts.len())
            .map_err(|error| match error {
                reliquary_vectors::BatchPackingError::Validation(error) => {
                    MemoryVectorError::Profile(error.into())
                }
                reliquary_vectors::BatchPackingError::InvalidMatrix
                | reliquary_vectors::BatchPackingError::Schema(_)
                | reliquary_vectors::BatchPackingError::Packed(_) => {
                    MemoryVectorError::InvalidEmbeddingMatrix
                }
            })?;
        let packed_vector_id = self.put_packed_vectors(packed)?;
        let set_id =
            self.put_memory_vectors(compatibility_profile_id, packed_vector_id, missing.clone())?;
        Ok(MemoryVectorBuildResult {
            created_set: Some(set_id),
            embedded: missing.len(),
            already_present,
        })
    }
}
