use crate::{
    ArchiveVectorError, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorSet, ArchiveVectorStats,
    Cva, FragmentId, PackedVectorId,
};

impl Cva {
    pub fn put_archive_vectors(
        &mut self,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        self.public_put_archive_vectors(packed_vector_id, fragment_ids)
    }

    pub fn archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        self.public_archive_vectors(id)
    }

    pub fn archive_vector_infos(&self) -> Vec<ArchiveVectorInfo> {
        self.public_archive_vector_infos()
    }

    pub fn archive_vector_stats(&self) -> ArchiveVectorStats {
        self.public_archive_vector_stats()
    }
}
