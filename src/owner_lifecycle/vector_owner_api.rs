//! Narrow durable vector operations owned by the mounted storage owners.
use crate::owner_lifecycle::{PhylacteryOwner, ReliquaryOwner};
use crate::{
    ArchiveVectorError, ArchiveVectorId, ArchiveVectorSet, CompatibilityProfile,
    CompatibilityProfileError, CompatibilityProfileId, FragmentId, MemoryBodyId, MemoryVectorError,
    MemoryVectorId, MemoryVectorSet, PackedVectorError, PackedVectorId, VectorGeneration,
    VectorGenerationError,
};
use lodestone_packed::PackedVectors;

impl ReliquaryOwner {
    pub(crate) fn get_archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        self.archive_vectors.get(&mut self.container, id)
    }

    pub(crate) fn put_archive_vectors(
        &mut self,
        packed_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        crate::archive_vector_store::validate_mapping_shape(
            &self.packed_vectors,
            packed_id,
            &fragment_ids,
        )?;
        let mut seen = std::collections::HashSet::with_capacity(fragment_ids.len());
        let mut max_version = 0;
        for id in &fragment_ids {
            let version = self
                .archive
                .fragment_archive_version(*id)
                .ok_or(ArchiveVectorError::MissingFragment)?;
            if !seen.insert(*id) {
                return Err(ArchiveVectorError::DuplicateFragment);
            }
            max_version = max_version.max(version);
        }
        self.archive_vectors.put(
            &mut self.container,
            &self.packed_vectors,
            max_version,
            packed_id,
            fragment_ids,
        )
    }

    pub(crate) fn get_packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }

    pub(crate) fn get_memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.memory_vectors.get(&mut self.container, id)
    }

    pub(crate) fn retire_compatibility_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.compatibility_profiles.retire(&mut self.container, id)
    }

    pub(crate) fn publish_vector_generation(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        archive_vector_id: ArchiveVectorId,
        source_archive_version: u64,
    ) -> Result<VectorGeneration, VectorGenerationError> {
        self.vector_generations.publish(
            &mut self.container,
            compatibility_profile_id,
            archive_vector_id,
            source_archive_version,
        )
    }
}

impl PhylacteryOwner {
    pub(crate) fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.packed_vectors.put(&mut self.container, packed)
    }

    pub(crate) fn get_packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }

    pub(crate) fn get_memory_vectors(
        &mut self,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        self.memory_vectors.get(&mut self.container, id)
    }

    pub(crate) fn put_memory_vectors(
        &mut self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        body_ids: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.compatibility_profiles
            .get_active(profile_id)
            .ok_or(MemoryVectorError::MissingProfile)?;
        crate::memory_vector_store::validate_mapping_shape(
            &self.compatibility_profiles,
            &self.packed_vectors,
            profile_id,
            packed_id,
            &body_ids,
        )?;
        let mut seen = std::collections::HashSet::with_capacity(body_ids.len());
        for body in &body_ids {
            if !self.memories.contains_body(*body) {
                return Err(MemoryVectorError::MissingMemoryBody);
            }
            if !seen.insert(*body) {
                return Err(MemoryVectorError::DuplicateMemoryBody);
            }
        }
        self.memory_vectors.put(
            &mut self.container,
            &self.compatibility_profiles,
            &self.packed_vectors,
            profile_id,
            packed_id,
            body_ids,
        )
    }

    pub(crate) fn put_compatibility_profile(
        &mut self,
        profile: CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, CompatibilityProfileError> {
        self.compatibility_profiles
            .put(&mut self.container, profile)
    }

    pub(crate) fn retire_compatibility_profile(
        &mut self,
        id: CompatibilityProfileId,
    ) -> Result<bool, CompatibilityProfileError> {
        self.compatibility_profiles.retire(&mut self.container, id)
    }
}

impl ReliquaryOwner {
    pub(crate) fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.packed_vectors.put(&mut self.container, packed)
    }
    pub(crate) fn put_compatibility_profile(
        &mut self,
        profile: CompatibilityProfile,
    ) -> Result<CompatibilityProfileId, CompatibilityProfileError> {
        self.compatibility_profiles
            .put(&mut self.container, profile)
    }
    pub(crate) fn put_prepared_memory_vector_binding(
        &mut self,
        profile: CompatibilityProfileId,
        packed: PackedVectorId,
        bodies: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        self.memory_vectors.put(
            &mut self.container,
            &self.compatibility_profiles,
            &self.packed_vectors,
            profile,
            packed,
            bodies,
        )
    }
    pub(crate) fn validate_vector_generation_reference(
        &self,
        archive_version: u64,
        generation: &VectorGeneration,
    ) -> Result<(), VectorGenerationError> {
        crate::vector_generation_validation::validate_generation_reference(
            archive_version,
            &self.packed_vectors,
            &self.archive_vectors,
            &self.compatibility_profiles,
            generation,
        )
    }
    pub(crate) fn vector_read_view(&self) -> crate::vector_generation_store::VectorReadView<'_> {
        crate::vector_generation_store::VectorReadView::new(
            &self.vector_generations,
            &self.archive_vectors,
            &self.packed_vectors,
            &self.container,
        )
    }
}
macro_rules! memory_vector_view {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn memory_vector_read_view(
                &self,
            ) -> crate::memory_vector_store::MemoryVectorReadView<'_> {
                crate::memory_vector_store::MemoryVectorReadView::new(
                    &self.memory_vectors,
                    &self.packed_vectors,
                    &self.container,
                )
            }
        }
    };
}
memory_vector_view!(ReliquaryOwner);
memory_vector_view!(PhylacteryOwner);
