use crate::archive_vector_store::validate_mapping_shape;
use crate::{
    ArchiveVectorError, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorSet, ArchiveVectorStats,
    Cva, FragmentId, PackedVectorId,
};
use std::collections::HashSet;

impl Cva {
    pub fn put_archive_vectors(
        &mut self,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        validate_mapping_shape(&self.packed_vectors, packed_vector_id, &fragment_ids)?;
        let mut seen = HashSet::with_capacity(fragment_ids.len());
        let mut max_fragment_archive_version = 0;
        for fragment_id in &fragment_ids {
            let version = self
                .archive
                .fragment_archive_version(*fragment_id)
                .ok_or(ArchiveVectorError::MissingFragment)?;
            if !seen.insert(*fragment_id) {
                return Err(ArchiveVectorError::DuplicateFragment);
            }
            max_fragment_archive_version = max_fragment_archive_version.max(version);
        }
        self.archive_vectors.put(
            &mut self.container,
            &self.packed_vectors,
            max_fragment_archive_version,
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
