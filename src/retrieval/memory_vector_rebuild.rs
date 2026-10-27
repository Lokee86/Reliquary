use crate::memory_store::MemoryStore;
use crate::memory_vector_store::{MemoryVectorStore, validate_mapping};
use crate::{MemoryBodyId, MemoryVectorError, MemoryVectorInfo};
use reliquary_container::ObjectRef;
use reliquary_vector_store::{
    CompatibilityProfileStore as LowerProfiles, MemoryVectorOpenState as LowerOpenState,
    PackedVectorStore,
};

pub(crate) struct MemoryVectorOpenState {
    inner: LowerOpenState,
}
impl MemoryVectorOpenState {
    pub(crate) fn new() -> Self {
        Self {
            inner: LowerOpenState::new(),
        }
    }
    pub(crate) fn ingest(
        &mut self,
        chunk: ObjectRef,
        payload: &[u8],
    ) -> Result<(), MemoryVectorError> {
        self.inner.ingest(chunk, payload).map_err(map_error)
    }
    pub(crate) fn finish(
        self,
        memories: &MemoryStore,
        profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
        packed: &PackedVectorStore,
    ) -> Result<MemoryVectorStore, MemoryVectorError> {
        let records = self.inner.finish_records().map_err(map_error)?;
        let mut store = MemoryVectorStore::default();
        for r in records {
            let bodies: Vec<_> = r.bodies.iter().map(|x| MemoryBodyId(x.0)).collect();
            let info = MemoryVectorInfo {
                id: crate::MemoryVectorId(r.info.id.0),
                compatibility_profile_id: r.info.compatibility_profile_id,
                packed_vector_id: r.info.packed_vector_id,
                rows: r.info.rows,
            };
            validate_mapping(
                memories,
                profiles,
                packed,
                info.compatibility_profile_id,
                info.packed_vector_id,
                &bodies,
            )?;
            reliquary_vector_store::validate_replay_record(packed, profiles.lower(), r.info)
                .map_err(map_error)?;
            store.insert_rebuilt(info, r.chunk, &bodies)?;
        }
        Ok(store)
    }
}
fn map_error(e: reliquary_vector_store::MemoryVectorError) -> MemoryVectorError {
    use reliquary_vector_store::MemoryVectorError as L;
    match e {
        L::Container(x) => x.into(),
        L::PackedVector(x) => x.into(),
        L::CorruptRecord(x) => MemoryVectorError::CorruptRecord(x),
        L::MissingFormat => MemoryVectorError::MissingFormat,
        L::ConflictingFormat => MemoryVectorError::ConflictingFormat,
        L::MissingObject => MemoryVectorError::MissingObject,
        L::MissingPackedVector => MemoryVectorError::MissingPackedVector,
        L::MissingProfile => MemoryVectorError::MissingProfile,
        L::RowCountMismatch => MemoryVectorError::RowCountMismatch,
        L::DimensionMismatch => MemoryVectorError::DimensionMismatch,
        L::UnsupportedScalar(scalar) => MemoryVectorError::UnsupportedScalar(scalar),
        L::DuplicateMemoryBody => MemoryVectorError::DuplicateMemoryBody,
        L::DuplicateBinding => MemoryVectorError::DuplicateBinding,
        L::HashCollision => MemoryVectorError::HashCollision,
        L::SizeOverflow => MemoryVectorError::SizeOverflow,
    }
}
