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
        self.public_packed_vectors(id)
    }

    pub fn packed_vector_infos(&self) -> Vec<PackedVectorInfo> {
        self.public_packed_vector_infos()
    }

    pub fn packed_vector_stats(&self) -> PackedVectorStats {
        self.public_packed_vector_stats()
    }

    pub(crate) fn prunable_packed_vector_profiles(
        &self,
    ) -> Vec<(PackedVectorId, CompatibilityProfileId)> {
        self.public_prunable_packed_vector_profiles()
    }

    pub(crate) fn packed_vector_prune_marker(
        &self,
        id: PackedVectorId,
        manifest_digest: [u8; 32],
    ) -> Result<Vec<u8>, PackedVectorError> {
        self.public_packed_vector_prune_marker(id, manifest_digest)
    }

    pub(crate) fn packed_vector_retention_descriptor(
        &self,
        profile_id: CompatibilityProfileId,
        packed_id: PackedVectorId,
        watermark: u64,
    ) -> Result<crate::owner_maintenance::HistoryRetentionDescriptor, CvaError> {
        self.public_packed_vector_retention_descriptor(profile_id, packed_id, watermark)
    }

    pub(crate) fn validate_pruned_packed_vectors(&self) -> Result<(), CvaError> {
        self.public_validate_pruned_packed_vectors()
    }
}
