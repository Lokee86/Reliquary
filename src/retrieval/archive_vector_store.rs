use crate::{Archive, ArchiveVectorError, FragmentId, PackedVectorId};
use reliquary_vector_store::PackedVectorStore;
use std::collections::HashSet;

/// Product-boundary authorization: prove the mapping names an accepted packed
/// object and real Archive fragments, preserving the previous check order.
pub(crate) fn validate_mapping(
    archive: &Archive,
    packed_vectors: &PackedVectorStore,
    packed_vector_id: PackedVectorId,
    fragment_ids: &[FragmentId],
) -> Result<(u64, u64), ArchiveVectorError> {
    let packed = packed_vectors
        .info(packed_vector_id)
        .ok_or(ArchiveVectorError::MissingPackedVector)?;
    let rows = u64::try_from(fragment_ids.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?;
    if packed.count != rows {
        return Err(ArchiveVectorError::RowCountMismatch);
    }
    let mut seen = HashSet::with_capacity(fragment_ids.len());
    let mut max_archive_version = 0_u64;
    for fragment_id in fragment_ids {
        let archive_version = archive
            .source_fragment_archive_version(*fragment_id)
            .ok_or(ArchiveVectorError::MissingFragment)?;
        max_archive_version = max_archive_version.max(archive_version);
        if !seen.insert(*fragment_id) {
            return Err(ArchiveVectorError::DuplicateFragment);
        }
    }
    Ok((rows, max_archive_version))
}
