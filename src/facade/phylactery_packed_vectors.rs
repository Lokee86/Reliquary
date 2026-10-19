use crate::{PackedVectorError, PackedVectorId, PackedVectorInfo, PackedVectorStats, Phylactery};
use lodestone_packed::PackedVectors;

impl Phylactery {
    pub fn put_packed_vectors(
        &mut self,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        self.container.require_storage_format_writable()?;
        self.packed_vectors.put(&mut self.container, packed)
    }

    pub fn packed_vectors(
        &mut self,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        self.packed_vectors.get(&mut self.container, id)
    }

    pub fn packed_vector_infos(&self) -> Vec<PackedVectorInfo> {
        self.packed_vectors.infos()
    }

    pub fn packed_vector_stats(&self) -> PackedVectorStats {
        self.packed_vectors.stats()
    }
}
