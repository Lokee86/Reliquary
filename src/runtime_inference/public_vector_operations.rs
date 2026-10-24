//! Native vector operations used by the public REL and PHY bindings.
use crate::{
    ArchiveVectorId, ArchiveVectorSet, CompatibilityProfile, CompatibilityProfileError,
    CompatibilityProfileId, CompatibilityProfileStats, CompatibilityReport, EmbeddingEndpoint,
    EmbeddingMode, FragmentId, MemoryBodyId, MemoryVectorBuildResult, MemoryVectorError,
    MemoryVectorId, MemoryVectorSet, PackedVectorError, PackedVectorId, VectorGeneration,
    VectorGenerationError, VectorGenerationId, VectorGenerationStats,
};
use lodestone_packed::PackedVectors;

impl crate::RelExecutionOwner {
    pub(crate) fn public_prunable_packed_vector_profiles(
        &self,
    ) -> Vec<(PackedVectorId, CompatibilityProfileId)> {
        crate::history::reliquary_vector_retention::eligible_packed_vector_profiles(
            &self.owner.packed_vectors,
            &self.owner.memory_vectors,
            &self.owner.archive_vectors,
            &self.owner.vector_generations,
            &self.owner.compatibility_profiles,
        )
    }

    pub(crate) fn public_packed_vector_prune_marker(
        &self,
        id: PackedVectorId,
        manifest_digest: [u8; 32],
    ) -> Result<Vec<u8>, PackedVectorError> {
        crate::history::reliquary_vector_retention::packed_vector_retention_marker(
            &self.owner.packed_vectors,
            id,
            manifest_digest,
        )
    }

    pub(crate) fn public_packed_vector_retention_descriptor(
        &self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        watermark: u64,
    ) -> Result<crate::owner_maintenance::HistoryRetentionDescriptor, crate::CvaError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.owner.container);
        crate::history::reliquary_vector_retention::packed_vector_retention_descriptor(
            &source,
            &self.owner.packed_vectors,
            &self.owner.memory_vectors,
            &self.owner.archive_vectors,
            &self.owner.compatibility_profiles,
            &self.owner.vector_generations,
            profile_id,
            packed_id,
            watermark,
        )
    }

    pub(crate) fn public_validate_pruned_packed_vectors(&self) -> Result<(), crate::CvaError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.owner.container);
        crate::history::reliquary_vector_retention::validate_packed_vector_retention(
            source,
            &self.owner.packed_vectors,
            &self.owner.memory_vectors,
            &self.owner.archive_vectors,
            &self.owner.compatibility_profiles,
            &self.owner.vector_generations,
            &self.owner.owner_maintenance,
        )
    }

    pub(crate) fn public_archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, crate::ArchiveVectorError> {
        self.owner.get_archive_vectors(id)
    }

    pub(crate) fn public_put_archive_vectors(
        &mut self,
        packed_id: PackedVectorId,
        fragments: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, crate::ArchiveVectorError> {
        self.owner.put_archive_vectors(packed_id, fragments)
    }

    pub(crate) fn public_packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.owner.get_packed_vectors(id)
    }

    pub(crate) fn public_memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.owner.get_memory_vectors(id)
    }

    pub(crate) fn public_archive_vector_infos(&self) -> Vec<crate::ArchiveVectorInfo> {
        self.owner.archive_vectors.infos()
    }

    pub(crate) fn public_archive_vector_stats(&self) -> crate::ArchiveVectorStats {
        self.owner.archive_vectors.stats()
    }

    pub(crate) fn public_memory_vector_infos(&self) -> Vec<crate::MemoryVectorInfo> {
        self.owner.memory_vectors.infos()
    }

    pub(crate) fn public_memory_vector_stats(&self) -> crate::MemoryVectorStats {
        self.owner.memory_vectors.stats()
    }

    pub(crate) fn public_packed_vector_infos(&self) -> Vec<crate::PackedVectorInfo> {
        self.owner.packed_vectors.infos()
    }

    pub(crate) fn public_packed_vector_stats(&self) -> crate::PackedVectorStats {
        self.owner.packed_vectors.stats()
    }

    pub(crate) fn public_active_memory_profiles(&self) -> Vec<CompatibilityProfile> {
        self.runtime_active_memory_compatibility_profiles()
    }

    pub(crate) fn public_active_memory_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        if self.owner.owner_id().is_none() || self.owner.import_epochs.open().is_some() {
            return None;
        }
        let profile = self.owner.compatibility_profiles.get_active(id)?;
        self.owner
            .memory_vectors
            .has_complete_coverage(id, &self.owner.memories.current_body_ids())
            .then(|| profile.clone())
    }

    pub(crate) fn public_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        self.owner
            .compatibility_profiles
            .get(id)
            .cloned()
            .ok_or(CompatibilityProfileError::MissingProfile)
    }

    pub(crate) fn public_profiles(&self) -> Vec<CompatibilityProfile> {
        self.owner.compatibility_profiles.profiles()
    }

    pub(crate) fn public_retire_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.owner.retire_compatibility_profile(id)
    }

    pub(crate) fn public_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.owner.compatibility_profiles.is_retired(id)
    }

    pub(crate) fn public_profile_stats(&self) -> CompatibilityProfileStats {
        self.owner.compatibility_profiles.stats()
    }

    pub(crate) fn public_verify_compatibility_endpoint(
        &self,
        id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityReport, CompatibilityProfileError> {
        let profile = self
            .owner
            .compatibility_profiles
            .get(id)
            .ok_or(CompatibilityProfileError::MissingProfile)?;
        crate::compatibility_profile_probe::verify_endpoint(profile, endpoint)
    }

    pub(crate) fn public_build_archive_vector_generation(
        &mut self,
        profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        let profile = self
            .owner
            .compatibility_profiles
            .get_active(profile_id)
            .cloned()
            .ok_or(VectorGenerationError::MissingProfile)?;
        let compatibility =
            crate::compatibility_profile_probe::verify_endpoint(&profile, endpoint)?;
        if !compatibility.compatible {
            return Err(VectorGenerationError::IncompatibleEndpoint);
        }
        self.public_build_archive_vector_generation_verified(profile, endpoint)
    }

    pub(crate) fn public_build_archive_vector_generation_verified(
        &mut self,
        profile: CompatibilityProfile,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        let profile_id = profile.id;
        let fragments = self.owner.archive.fragments();
        if fragments.is_empty() {
            return Err(VectorGenerationError::EmptyPopulation);
        }
        let source_archive_version = self.owner.archive.archive_version();
        let fragment_ids: Vec<_> = fragments.iter().map(|f| f.id).collect();
        let mut texts = Vec::with_capacity(fragment_ids.len());
        for id in &fragment_ids {
            texts.push(
                self.owner
                    .archive
                    .fragment_text(&mut self.owner.container, *id)?,
            );
        }
        let vectors = endpoint.embed(EmbeddingMode::Document, &texts)?;
        crate::compatibility_profile_probe::validate_embedding_batch(
            &profile,
            &vectors,
            texts.len(),
        )?;
        let schema = crate::VectorSchema::new(profile.dimensions, crate::ScalarType::F32)
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
        let packed_id = self
            .owner
            .packed_vectors
            .put(&mut self.owner.container, packed)?;
        let archive_vector_id = self.owner.put_archive_vectors(packed_id, fragment_ids)?;
        self.public_publish_vector_generation(profile_id, archive_vector_id, source_archive_version)
    }

    pub(crate) fn public_publish_vector_generation(
        &mut self,
        profile_id: CompatibilityProfileId,
        archive_vector_id: ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_publish_vector_generation_inner(
            profile_id,
            archive_vector_id,
            source_archive_version,
            true,
            true,
        )
    }

    pub(crate) fn public_publish_vector_generation_without_live_semantic(
        &mut self,
        profile_id: CompatibilityProfileId,
        archive_vector_id: ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.public_publish_vector_generation_inner(
            profile_id,
            archive_vector_id,
            source_archive_version,
            false,
            false,
        )
    }

    fn public_publish_vector_generation_inner(
        &mut self,
        profile_id: CompatibilityProfileId,
        archive_vector_id: ArchiveVectorId,
        source_archive_version: u64,
        process_live_semantic: bool,
        require_current_coverage: bool,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        if process_live_semantic && self.owner.import_epochs.open().is_some() {
            return Err(VectorGenerationError::Freshness(
                "live vector publication requires a completed import boundary".into(),
            ));
        }
        self.owner
            .compatibility_profiles
            .get_active(profile_id)
            .ok_or(VectorGenerationError::MissingProfile)?;
        let candidate = VectorGeneration {
            id: crate::vector_generation_validation::vector_generation_id(
                profile_id,
                archive_vector_id,
                source_archive_version,
            ),
            compatibility_profile_id: profile_id,
            archive_vector_id,
            source_archive_version,
            global_version: 0,
            vector_version: 0,
        };
        let archive_version = self.owner.archive.archive_version();
        if candidate.source_archive_version == 0
            || candidate.source_archive_version > archive_version
        {
            return Err(VectorGenerationError::SourceArchiveVersion);
        }
        crate::vector_generation_validation::validate_generation_reference(
            archive_version,
            &self.owner.packed_vectors,
            &self.owner.archive_vectors,
            &self.owner.compatibility_profiles,
            &candidate,
        )?;
        if require_current_coverage
            && (candidate.source_archive_version != archive_version
                || !self.owner.archive_vectors.has_exact_fragment_coverage(
                    candidate.archive_vector_id,
                    &self
                        .owner
                        .archive
                        .fragments()
                        .iter()
                        .map(|fragment| fragment.id)
                        .collect::<Vec<_>>(),
                ))
        {
            return Err(VectorGenerationError::IncompleteCoverage);
        }
        let generation = self.owner.publish_vector_generation(
            profile_id,
            archive_vector_id,
            source_archive_version,
        )?;
        if process_live_semantic && self.owner.owner_uuid().is_some() {
            self.process_semantic_freshness(generation.id, 64)
                .map_err(|error| VectorGenerationError::Freshness(error.to_string()))?;
        }
        Ok(generation)
    }

    pub(crate) fn public_vector_generation(
        &self,
        id: VectorGenerationId,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.owner
            .vector_generations
            .generation(id)
            .ok_or(VectorGenerationError::MissingGeneration)
    }

    pub(crate) fn public_vector_generation_at(
        &self,
        profile_id: CompatibilityProfileId,
        version: u64,
    ) -> Option<VectorGeneration> {
        self.owner
            .vector_generations
            .generation_at(profile_id, version)
    }

    pub(crate) fn public_historical_vector_generation(
        &self,
        profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.owner.vector_generations.current_generation(profile_id)
    }

    pub(crate) fn public_current_vector_generation(
        &self,
        profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.runtime_current_vector_generation(profile_id)
    }

    pub(crate) fn public_vector_version(&self) -> u64 {
        self.owner.vector_generations.vector_version()
    }

    pub(crate) fn public_vector_generation_stats(&self) -> VectorGenerationStats {
        let mut stats = self.owner.vector_generations.stats();
        stats.active_profiles = self.public_active_archive_profiles().len();
        stats
    }

    pub(crate) fn public_active_archive_profiles(&self) -> Vec<CompatibilityProfile> {
        self.owner
            .compatibility_profiles
            .profiles()
            .into_iter()
            .filter(|profile| self.runtime_current_vector_generation(profile.id).is_some())
            .collect()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn public_active_memory_profiles(&self) -> Vec<CompatibilityProfile> {
        self.owner
            .compatibility_profiles
            .profiles()
            .into_iter()
            .filter_map(|profile| self.public_active_memory_profile(profile.id))
            .collect()
    }

    pub(crate) fn public_memory_vector_infos(&self) -> Vec<crate::MemoryVectorInfo> {
        self.owner.memory_vectors.infos()
    }

    pub(crate) fn public_memory_vector_stats(&self) -> crate::MemoryVectorStats {
        self.owner.memory_vectors.stats()
    }

    pub(crate) fn public_packed_vector_infos(&self) -> Vec<crate::PackedVectorInfo> {
        self.owner.packed_vectors.infos()
    }

    pub(crate) fn public_packed_vector_stats(&self) -> crate::PackedVectorStats {
        self.owner.packed_vectors.stats()
    }

    pub(crate) fn public_active_memory_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        let profile = self.owner.compatibility_profiles.get_active(id)?;
        self.owner
            .memory_vectors
            .has_complete_coverage(id, &self.owner.memories.current_body_ids())
            .then(|| profile.clone())
    }

    pub(crate) fn public_memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.owner.get_memory_vectors(id)
    }

    pub(crate) fn public_put_memory_vectors(
        &mut self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        bodies: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.owner.put_memory_vectors(profile_id, packed_id, bodies)
    }

    pub(crate) fn public_put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.owner.put_packed_vectors(packed)
    }

    pub(crate) fn public_packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.owner.get_packed_vectors(id)
    }

    pub(crate) fn public_establish_profile(
        &mut self,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        let candidate = crate::compatibility_profile_probe::profile_from_endpoint(endpoint)?;
        if let Some(existing) = self
            .owner
            .compatibility_profiles
            .compatible_with(&candidate)
        {
            return Ok(existing.clone());
        }
        self.owner.put_compatibility_profile(candidate.clone())?;
        Ok(candidate)
    }

    pub(crate) fn public_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Result<CompatibilityProfile, CompatibilityProfileError> {
        self.owner
            .compatibility_profiles
            .get(id)
            .cloned()
            .ok_or(CompatibilityProfileError::MissingProfile)
    }

    pub(crate) fn public_profiles(&self) -> Vec<CompatibilityProfile> {
        self.owner.compatibility_profiles.profiles()
    }

    pub(crate) fn public_retire_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.owner.retire_compatibility_profile(id)
    }

    pub(crate) fn public_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
        self.owner.compatibility_profiles.is_retired(id)
    }

    pub(crate) fn public_profile_stats(&self) -> CompatibilityProfileStats {
        self.owner.compatibility_profiles.stats()
    }

    pub(crate) fn public_verify_compatibility_endpoint(
        &self,
        id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityReport, CompatibilityProfileError> {
        let profile = self
            .owner
            .compatibility_profiles
            .get(id)
            .ok_or(CompatibilityProfileError::MissingProfile)?;
        crate::compatibility_profile_probe::verify_endpoint(profile, endpoint)
    }

    pub(crate) fn public_build_missing_memory_vectors(
        &mut self,
        profile_id: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<MemoryVectorBuildResult, MemoryVectorError> {
        let profile = self
            .owner
            .compatibility_profiles
            .get_active(profile_id)
            .cloned()
            .ok_or(MemoryVectorError::MissingProfile)?;
        if !crate::compatibility_profile_probe::verify_endpoint(&profile, endpoint)?.compatible {
            return Err(MemoryVectorError::IncompatibleEndpoint);
        }
        let body_ids = self.owner.memories.current_body_ids();
        if body_ids.is_empty() {
            return Err(MemoryVectorError::EmptyPopulation);
        }
        let missing = self
            .owner
            .memory_vectors
            .missing_body_ids(profile_id, &body_ids);
        let already_present = body_ids.len().saturating_sub(missing.len());
        if missing.is_empty() {
            return Ok(MemoryVectorBuildResult {
                created_set: None,
                embedded: 0,
                already_present,
            });
        }
        let mut texts = Vec::with_capacity(missing.len());
        for id in &missing {
            texts.push(
                self.owner
                    .memories
                    .body_embedding_text(&mut self.owner.container, *id)?,
            );
        }
        let vectors = endpoint.embed(EmbeddingMode::Document, &texts)?;
        crate::compatibility_profile_probe::validate_embedding_batch(
            &profile,
            &vectors,
            texts.len(),
        )?;
        let schema = crate::VectorSchema::new(profile.dimensions, crate::ScalarType::F32)
            .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
        let row_bytes = usize::try_from(profile.dimensions)
            .ok()
            .and_then(|dimensions| dimensions.checked_mul(4))
            .ok_or(MemoryVectorError::InvalidEmbeddingMatrix)?;
        let capacity = vectors
            .len()
            .checked_mul(row_bytes)
            .ok_or(MemoryVectorError::InvalidEmbeddingMatrix)?;
        let mut bytes = Vec::with_capacity(capacity);
        for vector in vectors {
            for value in vector {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let packed = PackedVectors::from_bytes(schema, bytes)
            .map_err(|_| MemoryVectorError::InvalidEmbeddingMatrix)?;
        let packed_id = self.owner.put_packed_vectors(packed)?;
        let set_id = self
            .owner
            .put_memory_vectors(profile_id, packed_id, missing.clone())?;
        Ok(MemoryVectorBuildResult {
            created_set: Some(set_id),
            embedded: missing.len(),
            already_present,
        })
    }
}
