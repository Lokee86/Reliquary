use crate::memory_store::MemoryStore;
use crate::{
    CompatibilityProfileId, MemoryBodyId, MemoryVectorError, MemoryVectorId, MemoryVectorInfo,
    MemoryVectorLocation, MemoryVectorSet, MemoryVectorStats, PackedVectorId,
};
use reliquary_container::{Container, ObjectRef};
use reliquary_vector_store::{
    AcceptedMemoryMapping, MemoryVectorStore as LowerStore, PackedVectorStore,
};

#[derive(Default)]
pub(crate) struct MemoryVectorStore {
    inner: LowerStore,
}
impl MemoryVectorStore {
    pub(crate) fn initialize(&self, c: &mut Container) -> Result<(), MemoryVectorError> {
        self.inner.initialize(c).map_err(map_error)
    }
    pub(crate) fn put(
        &mut self,
        c: &mut Container,
        memories: &MemoryStore,
        profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
        packed: &PackedVectorStore,
        profile: CompatibilityProfileId,
        packed_id: PackedVectorId,
        bodies: Vec<MemoryBodyId>,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        profiles
            .get_active(profile)
            .ok_or(MemoryVectorError::MissingProfile)?;
        validate_mapping(memories, profiles, packed, profile, packed_id, &bodies)?;
        let rows = u64::try_from(bodies.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
        self.inner
            .put(
                c,
                packed,
                profiles.lower(),
                AcceptedMemoryMapping {
                    profile,
                    packed: packed_id,
                    packed_rows: rows,
                    body_ids: bodies
                        .into_iter()
                        .map(|x| reliquary_vector_store::MemoryBodyId(x.0))
                        .collect(),
                },
            )
            .map(|x| MemoryVectorId(x.0))
            .map_err(map_error)
    }
    pub(crate) fn get(
        &self,
        c: &mut Container,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorSet, MemoryVectorError> {
        let v = self
            .inner
            .get(c, reliquary_vector_store::MemoryVectorId(id.0))
            .map_err(map_error)?;
        Ok(MemoryVectorSet {
            id: MemoryVectorId(v.id.0),
            compatibility_profile_id: v.compatibility_profile_id,
            packed_vector_id: v.packed_vector_id,
            memory_body_ids: v
                .memory_body_ids
                .into_iter()
                .map(|x| MemoryBodyId(x.0))
                .collect(),
        })
    }
    pub(crate) fn insert_rebuilt(
        &mut self,
        info: MemoryVectorInfo,
        chunk: ObjectRef,
        bodies: &[MemoryBodyId],
    ) -> Result<(), MemoryVectorError> {
        self.inner
            .insert_rebuilt(
                reliquary_vector_store::MemoryVectorInfo {
                    id: reliquary_vector_store::MemoryVectorId(info.id.0),
                    compatibility_profile_id: info.compatibility_profile_id,
                    packed_vector_id: info.packed_vector_id,
                    rows: info.rows,
                },
                chunk,
                bodies
                    .iter()
                    .map(|x| reliquary_vector_store::MemoryBodyId(x.0))
                    .collect(),
            )
            .map_err(map_error)
    }
    pub(crate) fn location(
        &self,
        p: CompatibilityProfileId,
        b: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.inner
            .location(p, reliquary_vector_store::MemoryBodyId(b.0))
            .map(|x| MemoryVectorLocation {
                set_id: MemoryVectorId(x.set_id.0),
                row: x.row,
            })
    }
    pub(crate) fn missing_body_ids(
        &self,
        p: CompatibilityProfileId,
        b: &[MemoryBodyId],
    ) -> Vec<MemoryBodyId> {
        b.iter()
            .copied()
            .filter(|id| self.location(p, *id).is_none())
            .collect()
    }
    pub(crate) fn binding_count(&self, p: CompatibilityProfileId) -> usize {
        self.inner.binding_count(p)
    }
    pub(crate) fn has_complete_coverage(
        &self,
        p: CompatibilityProfileId,
        b: &[MemoryBodyId],
    ) -> bool {
        b.iter().all(|id| self.location(p, *id).is_some())
    }
    pub(crate) fn infos(&self) -> Vec<MemoryVectorInfo> {
        self.inner
            .infos()
            .into_iter()
            .map(|x| MemoryVectorInfo {
                id: MemoryVectorId(x.id.0),
                compatibility_profile_id: x.compatibility_profile_id,
                packed_vector_id: x.packed_vector_id,
                rows: x.rows,
            })
            .collect()
    }
    pub(crate) fn stats(&self) -> MemoryVectorStats {
        let x = self.inner.stats();
        MemoryVectorStats {
            objects: x.objects,
            rows: x.rows,
            bindings: x.bindings,
        }
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
pub(crate) fn validate_mapping(
    memories: &MemoryStore,
    profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    packed: &PackedVectorStore,
    p: CompatibilityProfileId,
    v: PackedVectorId,
    b: &[MemoryBodyId],
) -> Result<(), MemoryVectorError> {
    let profile = profiles.get(p).ok_or(MemoryVectorError::MissingProfile)?;
    let packed = packed
        .info(v)
        .ok_or(MemoryVectorError::MissingPackedVector)?;
    let rows = u64::try_from(b.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
    if packed.count != rows {
        return Err(MemoryVectorError::RowCountMismatch);
    }
    if packed.schema.dimensions != profile.dimensions {
        return Err(MemoryVectorError::DimensionMismatch);
    }
    if packed.schema.scalar != crate::ScalarType::F32 {
        return Err(MemoryVectorError::UnsupportedScalar(packed.schema.scalar));
    }
    let mut seen = std::collections::HashSet::new();
    for id in b {
        if !memories.contains_body(*id) {
            return Err(MemoryVectorError::MissingMemoryBody);
        }
        if !seen.insert(*id) {
            return Err(MemoryVectorError::DuplicateMemoryBody);
        }
    }
    Ok(())
}
