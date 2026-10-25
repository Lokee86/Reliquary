//! Concrete vector-owner read contracts; returned profiles, locations and statistics carry no store authority.
use crate::{
    ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorStats, CompatibilityProfile,
    CompatibilityProfileId, CompatibilityProfileStats, FragmentId, MemoryBodyId, MemoryVectorInfo,
    MemoryVectorLocation, MemoryVectorStats, PackedVectorInfo, PackedVectorStats, VectorGeneration,
    VectorGenerationId, VectorGenerationStats,
};
macro_rules! common_vector_reads {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn vector_profile_compatible_with(
                &self,
                candidate: &CompatibilityProfile,
            ) -> Option<&CompatibilityProfile> {
                self.compatibility_profiles.compatible_with(candidate)
            }
            pub(crate) fn vector_profile_get(
                &self,
                id: CompatibilityProfileId,
            ) -> Option<&CompatibilityProfile> {
                self.compatibility_profiles.get(id)
            }
            pub(crate) fn vector_profile_get_active(
                &self,
                id: CompatibilityProfileId,
            ) -> Option<&CompatibilityProfile> {
                self.compatibility_profiles.get_active(id)
            }
            pub(crate) fn vector_profile_is_retired(&self, id: CompatibilityProfileId) -> bool {
                self.compatibility_profiles.is_retired(id)
            }
            pub(crate) fn vector_profile_profiles(&self) -> Vec<CompatibilityProfile> {
                self.compatibility_profiles.profiles()
            }
            pub(crate) fn vector_profile_stats(&self) -> CompatibilityProfileStats {
                self.compatibility_profiles.stats()
            }
            pub(crate) fn vector_memory_vectors_location(
                &self,
                profile_id: CompatibilityProfileId,
                body_id: MemoryBodyId,
            ) -> Option<MemoryVectorLocation> {
                self.memory_vectors.location(profile_id, body_id)
            }
            pub(crate) fn vector_memory_vectors_missing_body_ids(
                &self,
                profile_id: CompatibilityProfileId,
                body_ids: &[MemoryBodyId],
            ) -> Vec<MemoryBodyId> {
                self.memory_vectors.missing_body_ids(profile_id, body_ids)
            }
            pub(crate) fn vector_memory_vectors_has_complete_coverage(
                &self,
                profile_id: CompatibilityProfileId,
                body_ids: &[MemoryBodyId],
            ) -> bool {
                self.memory_vectors
                    .has_complete_coverage(profile_id, body_ids)
            }
            pub(crate) fn vector_memory_vectors_infos(&self) -> Vec<MemoryVectorInfo> {
                self.memory_vectors.infos()
            }
            pub(crate) fn vector_memory_vectors_stats(&self) -> MemoryVectorStats {
                self.memory_vectors.stats()
            }
            pub(crate) fn vector_packed_vectors_infos(&self) -> Vec<PackedVectorInfo> {
                self.packed_vectors.infos()
            }
            pub(crate) fn vector_packed_vectors_stats(&self) -> PackedVectorStats {
                self.packed_vectors.stats()
            }
        }
    };
}
common_vector_reads!(crate::owner_lifecycle::ReliquaryOwner);
common_vector_reads!(crate::owner_lifecycle::PhylacteryOwner);
impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn vector_archive_vectors_has_exact_fragment_coverage(
        &self,
        id: ArchiveVectorId,
        expected: &[FragmentId],
    ) -> bool {
        self.archive_vectors
            .has_exact_fragment_coverage(id, expected)
    }
    pub(crate) fn vector_archive_vectors_infos(&self) -> Vec<ArchiveVectorInfo> {
        self.archive_vectors.infos()
    }
    pub(crate) fn vector_archive_vectors_stats(&self) -> ArchiveVectorStats {
        self.archive_vectors.stats()
    }
    pub(crate) fn vector_generation_generation(
        &self,
        id: VectorGenerationId,
    ) -> Option<VectorGeneration> {
        self.vector_generations.generation(id)
    }
    pub(crate) fn vector_generation_current_generation(
        &self,
        profile_id: CompatibilityProfileId,
    ) -> Option<VectorGeneration> {
        self.vector_generations.current_generation(profile_id)
    }
    pub(crate) fn vector_generation_generation_at(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        vector_version: u64,
    ) -> Option<VectorGeneration> {
        self.vector_generations
            .generation_at(compatibility_profile_id, vector_version)
    }
    pub(crate) fn vector_generation_vector_version(&self) -> u64 {
        self.vector_generations.vector_version()
    }
    pub(crate) fn vector_generation_latest_by_version(&self) -> Option<VectorGeneration> {
        self.vector_generations
            .generations()
            .iter()
            .max_by_key(|generation| generation.vector_version)
            .copied()
    }
    pub(crate) fn vector_archive_vector_info(
        &self,
        id: crate::ArchiveVectorId,
    ) -> Option<ArchiveVectorInfo> {
        self.archive_vectors.info(id)
    }
    pub(crate) fn vector_packed_vector_info(
        &self,
        id: crate::PackedVectorId,
    ) -> Option<PackedVectorInfo> {
        self.packed_vectors.info(id)
    }
    pub(crate) fn vector_generation_stats(&self) -> VectorGenerationStats {
        self.vector_generations.stats()
    }
}
