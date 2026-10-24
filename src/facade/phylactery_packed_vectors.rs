use crate::{PackedVectorError, PackedVectorId, PackedVectorInfo, PackedVectorStats, Phylactery};
use lodestone_packed::PackedVectors;

impl Phylactery {
    pub fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.public_put_packed_vectors(packed)
    }

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
}
