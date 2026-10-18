use crate::archive_vector_codec::{decode_format, decode_object};
use crate::archive_vector_store::{ArchiveVectorStore, archive_vector_id, validate_mapping};
use crate::packed_vector_store::PackedVectorStore;
use crate::{Archive, ArchiveVectorError, ArchiveVectorInfo, FragmentId, ObjectRef};

pub(crate) struct ArchiveVectorOpenState {
    records: Vec<PendingArchiveVectors>,
    format_seen: bool,
}

struct PendingArchiveVectors {
    info: ArchiveVectorInfo,
    chunk: ObjectRef,
    fragment_ids: Vec<FragmentId>,
}

impl ArchiveVectorOpenState {
    pub(crate) fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }

    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), ArchiveVectorError> {
        if decode_format(payload)? {
            if self.format_seen {
                return Err(ArchiveVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        let Some(decoded) = decode_object(payload)? else {
            return Ok(());
        };
        if archive_vector_id(decoded.packed_vector_id, &decoded.fragment_ids) != decoded.id {
            return Err(ArchiveVectorError::HashCollision);
        }
        self.records.push(PendingArchiveVectors {
            info: ArchiveVectorInfo {
                id: decoded.id,
                packed_vector_id: decoded.packed_vector_id,
                rows: u64::try_from(decoded.fragment_ids.len())
                    .map_err(|_| ArchiveVectorError::SizeOverflow)?,
                max_fragment_archive_version: 0,
            },
            chunk,
            fragment_ids: decoded.fragment_ids,
        });
        Ok(())
    }

    pub(crate) fn finish(
        self,
        archive: &Archive,
        packed_vectors: &PackedVectorStore,
    ) -> Result<ArchiveVectorStore, ArchiveVectorError> {
        if !self.format_seen {
            return Err(ArchiveVectorError::MissingFormat);
        }
        let mut store = ArchiveVectorStore::default();
        for record in self.records {
            let mut info = record.info;
            info.max_fragment_archive_version = validate_mapping(
                archive,
                packed_vectors,
                info.packed_vector_id,
                &record.fragment_ids,
            )?;
            store.insert_rebuilt(info, record.chunk, record.fragment_ids)?;
        }
        Ok(store)
    }
}
