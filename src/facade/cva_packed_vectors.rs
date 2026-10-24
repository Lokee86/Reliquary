use crate::owner_lifecycle::HistorySource;
use crate::{
    CompatibilityProfileId, Cva, CvaError, PackedVectorError, PackedVectorId, PackedVectorInfo,
    PackedVectorStats,
};
use lodestone_packed::PackedVectors;

impl Cva {
    pub fn packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.owner.packed_vectors.get(&mut self.owner.container, id)
    }

    pub fn packed_vector_infos(&self) -> Vec<PackedVectorInfo> {
        self.packed_vectors.infos()
    }

    pub fn packed_vector_stats(&self) -> PackedVectorStats {
        self.packed_vectors.stats()
    }

    pub(crate) fn prunable_packed_vector_profiles(
        &self,
    ) -> Vec<(PackedVectorId, CompatibilityProfileId)> {
        crate::history::reliquary_vector_retention::eligible_packed_vector_profiles(
            &self.packed_vectors,
            &self.memory_vectors,
            &self.archive_vectors,
            &self.vector_generations,
            &self.compatibility_profiles,
        )
    }

    pub(crate) fn packed_vector_prune_marker(
        &self,
        id: PackedVectorId,
        manifest_digest: [u8; 32],
    ) -> Result<Vec<u8>, PackedVectorError> {
        crate::history::reliquary_vector_retention::packed_vector_retention_marker(
            &self.packed_vectors,
            id,
            manifest_digest,
        )
    }

    pub(crate) fn packed_vector_retention_descriptor(
        &self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        watermark: u64,
    ) -> Result<crate::owner_maintenance::HistoryRetentionDescriptor, CvaError> {
        let source = HistorySource::new(&self.container);
        crate::history::reliquary_vector_retention::packed_vector_retention_descriptor(
            &source,
            &self.packed_vectors,
            &self.memory_vectors,
            &self.archive_vectors,
            &self.compatibility_profiles,
            &self.vector_generations,
            profile_id,
            packed_id,
            watermark,
        )
    }

    pub(crate) fn validate_pruned_packed_vectors(&self) -> Result<(), CvaError> {
        let source = HistorySource::new(&self.container);
        crate::history::reliquary_vector_retention::validate_packed_vector_retention(
            source,
            &self.packed_vectors,
            &self.memory_vectors,
            &self.archive_vectors,
            &self.compatibility_profiles,
            &self.vector_generations,
            &self.owner_maintenance,
        )
    }
}
