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
        self.archive_vectors.put(
            &mut self.container,
            &self.archive,
            &self.packed_vectors,
            packed_vector_id,
            fragment_ids,
        )
    }

    pub fn archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        self.archive_vectors.get(&mut self.container, id)
    }

    pub fn archive_vector_infos(&self) -> Vec<ArchiveVectorInfo> {
        self.archive_vectors.infos()
    }

    pub fn archive_vector_stats(&self) -> ArchiveVectorStats {
        self.archive_vectors.stats()
    }
}
