use crate::{
    ArchiveVectorError, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorSet, ArchiveVectorStats,
    Cva, FragmentId, PackedVectorId,
};
use reliquary_vector_store::{AcceptedArchiveMapping, ArchiveFragmentId};

impl Cva {
    pub fn put_archive_vectors(
        &mut self,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        // Preserve public error precedence: packed existence, row count, then
        // Archive provenance and duplicate rows, followed by pruned payload.
        let (rows, max_fragment_archive_version) = crate::archive_vector_store::validate_mapping(
            &self.archive,
            &self.packed_vectors,
            packed_vector_id,
            &fragment_ids,
        )?;
        if self.packed_vectors.is_pruned(packed_vector_id) {
            return Err(crate::PackedVectorError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned,
            )
            .into());
        }
        self.archive_vectors
            .put(
                &mut self.container,
                &self.packed_vectors,
                AcceptedArchiveMapping {
                    packed_vector_id,
                    packed_rows: rows,
                    fragment_ids: fragment_ids
                        .into_iter()
                        .map(|id| ArchiveFragmentId(id.0))
                        .collect(),
                    max_fragment_archive_version,
                },
            )
            .map_err(Into::into)
    }

    pub fn archive_vectors(
        &mut self,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        let record = self.archive_vectors.get(&mut self.container, id)?;
        Ok(ArchiveVectorSet {
            id: record.id,
            packed_vector_id: record.packed_vector_id,
            fragment_ids: record
                .fragment_ids
                .into_iter()
                .map(|id| FragmentId(id.0))
                .collect(),
        })
    }

    pub fn archive_vector_infos(&self) -> Vec<ArchiveVectorInfo> {
        self.archive_vectors.infos()
    }
    pub fn archive_vector_stats(&self) -> ArchiveVectorStats {
        self.archive_vectors.stats()
    }
}
