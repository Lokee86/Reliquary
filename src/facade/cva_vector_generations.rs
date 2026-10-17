use crate::compatibility_profile_probe::{validate_embedding_batch, verify_endpoint};
use crate::vector_generation_validation::{
    has_complete_current_coverage, validate_generation_reference, vector_generation_id,
};
use crate::{
    CompatibilityProfile, CompatibilityProfileId, Cva, EmbeddingEndpoint, EmbeddingMode,
    PackedVectors, ScalarType, VectorGeneration, VectorGenerationError, VectorGenerationId,
    VectorGenerationStats, VectorSchema,
};

impl Cva {
    pub fn build_archive_vector_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        let profile = self
            .compatibility_profiles
            .get_active(compatibility_profile_id)
            .cloned()
            .ok_or(VectorGenerationError::MissingProfile)?;
        let compatibility = verify_endpoint(&profile, endpoint)?;
        if !compatibility.compatible {
            return Err(VectorGenerationError::IncompatibleEndpoint);
        }
        self.build_archive_vector_generation_verified(profile, endpoint)
    }

    pub(crate) fn build_archive_vector_generation_verified(
        &mut self,
        profile: CompatibilityProfile,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        let compatibility_profile_id = profile.id;
        let fragments = self.fragments();
        if fragments.is_empty() {
            return Err(VectorGenerationError::EmptyPopulation);
        }
        let source_archive_version = self.archive_version();
        let fragment_ids: Vec<_> = fragments.iter().map(|fragment| fragment.id).collect();
        let mut texts = Vec::with_capacity(fragment_ids.len());
        for fragment_id in &fragment_ids {
            texts.push(self.fragment_text(*fragment_id)?);
        }
        let vectors = endpoint.embed(EmbeddingMode::Document, &texts)?;
        validate_embedding_batch(&profile, &vectors, texts.len())?;
        let schema = VectorSchema::new(profile.dimensions, ScalarType::F32)
            .map_err(|_| VectorGenerationError::InvalidEmbeddingMatrix)?;
        let row_bytes = usize::try_from(profile.dimensions)
            .ok()
            .and_then(|dimensions| dimensions.checked_mul(4))
            .ok_or(VectorGenerationError::InvalidEmbeddingMatrix)?;
        let capacity = vectors
            .len()
            .checked_mul(row_bytes)
            .ok_or(VectorGenerationError::InvalidEmbeddingMatrix)?;
        let mut bytes = Vec::with_capacity(capacity);
        for vector in vectors {
            for value in vector {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let packed = PackedVectors::from_bytes(schema, bytes)
            .map_err(|_| VectorGenerationError::InvalidEmbeddingMatrix)?;
        let packed_id = self.put_packed_vectors(packed)?;
        let archive_vector_id = self.put_archive_vectors(packed_id, fragment_ids)?;
        self.publish_vector_generation(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )
    }

    pub fn publish_vector_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.publish_vector_generation_inner(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
            true,
        )
    }

    pub(crate) fn publish_vector_generation_without_live_semantic(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.publish_vector_generation_inner(
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
            false,
        )
    }

    fn publish_vector_generation_inner(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: crate::ArchiveVectorId,
        source_archive_version: u64,
        process_live_semantic: bool,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)
            .ok_or(VectorGenerationError::MissingProfile)?;
        let candidate = VectorGeneration {
            id: vector_generation_id(
                compatibility_profile_id,
                archive_vector_id,
                source_archive_version,
            ),
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
            global_version: 0,
            vector_version: 0,
        };
        validate_generation_reference(
            &self.archive,
            &self.packed_vectors,
            &self.archive_vectors,
            &self.compatibility_profiles,
            &candidate,
        )?;
        if !has_complete_current_coverage(&self.archive, &self.archive_vectors, &candidate) {
            return Err(VectorGenerationError::IncompleteCoverage);
        }
        let generation = self.vector_generations.publish(
            &mut self.container,
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )?;
        if process_live_semantic {
            self.process_semantic_freshness(generation.id, 64)
                .map_err(|error| VectorGenerationError::Freshness(error.to_string()))?;
        }
        Ok(generation)
    }

    pub fn vector_generation(
        &self,
        id: VectorGenerationId,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.vector_generations
            .generation(id)
            .ok_or(VectorGenerationError::MissingGeneration)
    }

    pub fn current_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.compatibility_profiles
            .get_active(compatibility_profile_id)?;
        let generation = self
            .vector_generations
            .current_generation(compatibility_profile_id)?;
        has_complete_current_coverage(&self.archive, &self.archive_vectors, &generation)
            .then_some(generation)
    }

    pub fn vector_generation_at(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        vector_version: u64,
    ) -> Option<VectorGeneration> {
        self.vector_generations
            .generation_at(compatibility_profile_id, vector_version)
    }

    pub fn historical_vector_generation(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.vector_generations
            .current_generation(compatibility_profile_id)
    }

    pub fn vector_version(&self) -> u64 {
        self.vector_generations.vector_version()
    }

    pub fn vector_generation_stats(&self) -> VectorGenerationStats {
        let mut stats = self.vector_generations.stats();
        stats.active_profiles = self.active_archive_compatibility_profiles().len();
        stats
    }
}
