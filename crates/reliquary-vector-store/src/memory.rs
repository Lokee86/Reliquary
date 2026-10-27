use crate::CompatibilityProfileStore;
use crate::memory_codec::{decode_format, decode_object, encode_format, encode_object};
use crate::{
    CompatibilityProfileId, MemoryBodyId, MemoryVectorId, MemoryVectorInfo, MemoryVectorLocation,
    MemoryVectorRecord, MemoryVectorStats, PackedVectorError, PackedVectorId, PackedVectorStore,
};
use lodestone_packed::ScalarType;
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt};
#[derive(Debug)]
pub enum MemoryVectorError {
    Container(reliquary_container::ContainerError),
    PackedVector(PackedVectorError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    MissingProfile,
    RowCountMismatch,
    DimensionMismatch,
    UnsupportedScalar(ScalarType),
    DuplicateMemoryBody,
    DuplicateBinding,
    HashCollision,
    SizeOverflow,
}
impl From<reliquary_container::ContainerError> for MemoryVectorError {
    fn from(x: reliquary_container::ContainerError) -> Self {
        Self::Container(x)
    }
}
impl From<PackedVectorError> for MemoryVectorError {
    fn from(x: PackedVectorError) -> Self {
        Self::PackedVector(x)
    }
}
impl fmt::Display for MemoryVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Memory vector store error: {self:?}")
    }
}
impl std::error::Error for MemoryVectorError {}
pub struct AcceptedMemoryMapping {
    pub profile: CompatibilityProfileId,
    pub packed: PackedVectorId,
    pub packed_rows: u64,
    pub body_ids: Vec<MemoryBodyId>,
}
#[derive(Default)]
pub struct MemoryVectorStore {
    objects: HashMap<MemoryVectorId, Entry>,
    bindings: HashMap<(CompatibilityProfileId, MemoryBodyId), MemoryVectorLocation>,
    profile_counts: HashMap<CompatibilityProfileId, usize>,
}
#[derive(Clone)]
struct Entry {
    info: MemoryVectorInfo,
    chunk: ObjectRef,
}
pub struct MemoryVectorOpenState {
    records: Vec<MemoryVectorReplayRecord>,
    format_seen: bool,
}
pub struct MemoryVectorReplayRecord {
    pub info: MemoryVectorInfo,
    pub chunk: ObjectRef,
    pub bodies: Vec<MemoryBodyId>,
}
pub struct DecodedMemoryVectorObject {
    pub id: MemoryVectorId,
    pub compatibility_profile_id: CompatibilityProfileId,
    pub packed_vector_id: PackedVectorId,
    pub memory_body_ids: Vec<MemoryBodyId>,
}
pub fn decode_memory_vector_object(
    bytes: &[u8],
) -> Result<Option<DecodedMemoryVectorObject>, MemoryVectorError> {
    Ok(decode_object(bytes)?.map(|d| DecodedMemoryVectorObject {
        id: d.id,
        compatibility_profile_id: d.profile,
        packed_vector_id: d.packed,
        memory_body_ids: d.bodies,
    }))
}
impl MemoryVectorStore {
    pub fn initialize(&self, c: &mut Container) -> Result<(), MemoryVectorError> {
        c.append(&encode_format())?;
        Ok(())
    }
    pub fn put(
        &mut self,
        c: &mut Container,
        p: &PackedVectorStore,
        profiles: &CompatibilityProfileStore,
        a: AcceptedMemoryMapping,
    ) -> Result<MemoryVectorId, MemoryVectorError> {
        let packed = p
            .info(a.packed)
            .ok_or(MemoryVectorError::MissingPackedVector)?;
        let profile = profiles
            .get(a.profile)
            .ok_or(MemoryVectorError::MissingProfile)?;
        let rows = u64::try_from(a.body_ids.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
        if rows != a.packed_rows || rows != packed.count {
            return Err(MemoryVectorError::RowCountMismatch);
        }
        if packed.schema.dimensions != profile.dimensions {
            return Err(MemoryVectorError::DimensionMismatch);
        }
        if packed.schema.scalar != ScalarType::F32 {
            return Err(MemoryVectorError::UnsupportedScalar(packed.schema.scalar));
        }
        if p.is_pruned(a.packed) {
            return Err(
                PackedVectorError::PayloadUnavailable(crate::PayloadUnavailable::Pruned).into(),
            );
        }
        ensure_unique(&a.body_ids)?;
        let id = memory_vector_id(a.profile, a.packed, &a.body_ids);
        if let Some(e) = self.objects.get(&id) {
            let d = decode_object(&c.read(e.chunk)?)?.ok_or(MemoryVectorError::MissingObject)?;
            if d.id != id
                || d.profile != a.profile
                || d.packed != a.packed
                || d.bodies != a.body_ids
            {
                return Err(MemoryVectorError::HashCollision);
            }
            return Ok(id);
        }
        for x in &a.body_ids {
            if self.bindings.contains_key(&(a.profile, *x)) {
                return Err(MemoryVectorError::DuplicateBinding);
            }
        }
        let payload = encode_object(id, a.profile, a.packed, &a.body_ids)?;
        let chunk = c.append(&payload)?;
        let info = MemoryVectorInfo {
            id,
            compatibility_profile_id: a.profile,
            packed_vector_id: a.packed,
            rows,
        };
        self.insert(info, chunk, a.body_ids)?;
        Ok(id)
    }
    pub fn get(
        &self,
        c: &mut Container,
        id: MemoryVectorId,
    ) -> Result<MemoryVectorRecord, MemoryVectorError> {
        let e = self
            .objects
            .get(&id)
            .ok_or(MemoryVectorError::MissingObject)?;
        let d = decode_object(&c.read(e.chunk)?)?.ok_or(MemoryVectorError::MissingObject)?;
        if d.id != id || memory_vector_id(d.profile, d.packed, &d.bodies) != id {
            return Err(MemoryVectorError::HashCollision);
        }
        Ok(MemoryVectorRecord {
            id,
            compatibility_profile_id: d.profile,
            packed_vector_id: d.packed,
            memory_body_ids: d.bodies,
        })
    }
    pub fn insert_rebuilt(
        &mut self,
        i: MemoryVectorInfo,
        ch: ObjectRef,
        b: Vec<MemoryBodyId>,
    ) -> Result<(), MemoryVectorError> {
        if i.rows != u64::try_from(b.len()).map_err(|_| MemoryVectorError::SizeOverflow)?
            || memory_vector_id(i.compatibility_profile_id, i.packed_vector_id, &b) != i.id
        {
            return Err(MemoryVectorError::HashCollision);
        }
        ensure_unique(&b)?;
        if let Some(e) = self.objects.get(&i.id) {
            if e.info != i {
                return Err(MemoryVectorError::HashCollision);
            }
            for (n, x) in b.iter().enumerate() {
                if self.bindings.get(&(i.compatibility_profile_id, *x))
                    != Some(&MemoryVectorLocation {
                        set_id: i.id,
                        row: u64::try_from(n).map_err(|_| MemoryVectorError::SizeOverflow)?,
                    })
                {
                    return Err(MemoryVectorError::HashCollision);
                }
            }
            return Ok(());
        }
        self.insert(i, ch, b)
    }
    fn insert(
        &mut self,
        i: MemoryVectorInfo,
        ch: ObjectRef,
        b: Vec<MemoryBodyId>,
    ) -> Result<(), MemoryVectorError> {
        ensure_unique(&b)?;
        if b.iter().any(|x| {
            self.bindings
                .contains_key(&(i.compatibility_profile_id, *x))
        }) {
            return Err(MemoryVectorError::DuplicateBinding);
        }
        for (n, x) in b.iter().enumerate() {
            self.bindings.insert(
                (i.compatibility_profile_id, *x),
                MemoryVectorLocation {
                    set_id: i.id,
                    row: u64::try_from(n).map_err(|_| MemoryVectorError::SizeOverflow)?,
                },
            );
        }
        *self
            .profile_counts
            .entry(i.compatibility_profile_id)
            .or_default() += b.len();
        self.objects.insert(i.id, Entry { info: i, chunk: ch });
        Ok(())
    }
    pub fn location(
        &self,
        p: CompatibilityProfileId,
        b: MemoryBodyId,
    ) -> Option<MemoryVectorLocation> {
        self.bindings.get(&(p, b)).copied()
    }
    pub fn missing_body_ids(
        &self,
        p: CompatibilityProfileId,
        b: &[MemoryBodyId],
    ) -> Vec<MemoryBodyId> {
        b.iter()
            .copied()
            .filter(|x| !self.bindings.contains_key(&(p, *x)))
            .collect()
    }
    pub fn binding_count(&self, p: CompatibilityProfileId) -> usize {
        self.profile_counts.get(&p).copied().unwrap_or(0)
    }
    pub fn has_complete_coverage(&self, p: CompatibilityProfileId, b: &[MemoryBodyId]) -> bool {
        b.iter().all(|x| self.bindings.contains_key(&(p, *x)))
    }
    pub fn infos(&self) -> Vec<MemoryVectorInfo> {
        let mut v: Vec<_> = self.objects.values().map(|e| e.info).collect();
        v.sort_by_key(|x| x.id.0);
        v
    }
    pub fn stats(&self) -> MemoryVectorStats {
        MemoryVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|e| e.info.rows).sum(),
            bindings: self.bindings.len(),
        }
    }
}

pub fn validate_replay_record(
    packed_vectors: &PackedVectorStore,
    profiles: &CompatibilityProfileStore,
    info: MemoryVectorInfo,
) -> Result<(), MemoryVectorError> {
    let profile = profiles
        .get(info.compatibility_profile_id)
        .ok_or(MemoryVectorError::MissingProfile)?;
    let packed = packed_vectors
        .info(info.packed_vector_id)
        .ok_or(MemoryVectorError::MissingPackedVector)?;
    if packed.count != info.rows {
        return Err(MemoryVectorError::RowCountMismatch);
    }
    if packed.schema.dimensions != profile.dimensions {
        return Err(MemoryVectorError::DimensionMismatch);
    }
    if packed.schema.scalar != ScalarType::F32 {
        return Err(MemoryVectorError::UnsupportedScalar(packed.schema.scalar));
    }
    Ok(())
}
pub fn memory_vector_id(
    p: CompatibilityProfileId,
    v: PackedVectorId,
    b: &[MemoryBodyId],
) -> MemoryVectorId {
    let mut h = Sha256::new();
    h.update(b"CVA-MEMORY-VECTORS-V1\0");
    h.update(p.0);
    h.update(v.0);
    for x in b {
        h.update(x.0)
    }
    MemoryVectorId(h.finalize().into())
}
fn ensure_unique(b: &[MemoryBodyId]) -> Result<(), MemoryVectorError> {
    let mut s = std::collections::HashSet::new();
    for x in b {
        if !s.insert(*x) {
            return Err(MemoryVectorError::DuplicateMemoryBody);
        }
    }
    Ok(())
}
impl MemoryVectorOpenState {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }
    pub fn ingest(&mut self, ch: ObjectRef, p: &[u8]) -> Result<(), MemoryVectorError> {
        if decode_format(p)? {
            if self.format_seen {
                return Err(MemoryVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        if let Some(d) = decode_object(p)? {
            if memory_vector_id(d.profile, d.packed, &d.bodies) != d.id {
                return Err(MemoryVectorError::HashCollision);
            }
            let rows =
                u64::try_from(d.bodies.len()).map_err(|_| MemoryVectorError::SizeOverflow)?;
            self.records.push(MemoryVectorReplayRecord {
                info: MemoryVectorInfo {
                    id: d.id,
                    compatibility_profile_id: d.profile,
                    packed_vector_id: d.packed,
                    rows,
                },
                chunk: ch,
                bodies: d.bodies,
            })
        }
        Ok(())
    }
    pub fn finish_records(self) -> Result<Vec<MemoryVectorReplayRecord>, MemoryVectorError> {
        if !self.format_seen {
            return Err(MemoryVectorError::MissingFormat);
        }
        Ok(self.records)
    }
}
