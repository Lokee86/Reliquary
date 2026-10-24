use crate::{PackedVectorError, PackedVectorId, PackedVectorInfo, PackedVectorStats, Phylactery};
use lodestone_packed::PackedVectors;

impl Phylactery {
    pub fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.owner
            .packed_vectors
            .put(&mut self.owner.container, packed)
    }

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
}
