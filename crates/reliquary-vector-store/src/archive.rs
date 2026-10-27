use crate::archive_codec::{
    decode_archive_format, decode_archive_object, encode_archive_format, encode_archive_object,
};
use crate::{
    ArchiveFragmentId, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorRecord, ArchiveVectorStats,
    PackedVectorId, PackedVectorStore,
};
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug)]
pub enum ArchiveVectorError {
    Container(reliquary_container::ContainerError),
    PackedVector(crate::PackedVectorError),
    CorruptRecord(&'static str),
    MissingFormat,
    ConflictingFormat,
    MissingObject,
    MissingPackedVector,
    RowCountMismatch,
    DuplicateFragment,
    HashCollision,
    SizeOverflow,
}
impl From<reliquary_container::ContainerError> for ArchiveVectorError {
    fn from(e: reliquary_container::ContainerError) -> Self {
        Self::Container(e)
    }
}
impl From<crate::PackedVectorError> for ArchiveVectorError {
    fn from(e: crate::PackedVectorError) -> Self {
        Self::PackedVector(e)
    }
}
impl fmt::Display for ArchiveVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Archive vector store error: {self:?}")
    }
}
impl std::error::Error for ArchiveVectorError {}

/// Constructed by the product boundary only after it authenticates Archive provenance.
#[derive(Clone, Debug)]
pub struct AcceptedArchiveMapping {
    pub packed_vector_id: PackedVectorId,
    pub packed_rows: u64,
    pub fragment_ids: Vec<ArchiveFragmentId>,
    pub max_fragment_archive_version: u64,
}
#[derive(Default)]
pub struct ArchiveVectorStore {
    objects: HashMap<ArchiveVectorId, Entry>,
}
#[derive(Clone)]
struct Entry {
    info: ArchiveVectorInfo,
    chunk: ObjectRef,
    fragment_ids: Vec<ArchiveFragmentId>,
}

impl ArchiveVectorStore {
    pub fn initialize(&self, c: &mut Container) -> Result<(), ArchiveVectorError> {
        c.append(&encode_archive_format())?;
        Ok(())
    }
    pub fn put(
        &mut self,
        c: &mut Container,
        packed_vectors: &PackedVectorStore,
        a: AcceptedArchiveMapping,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        let packed = packed_vectors
            .info(a.packed_vector_id)
            .ok_or(ArchiveVectorError::MissingPackedVector)?;
        let rows =
            u64::try_from(a.fragment_ids.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?;
        if rows != a.packed_rows || rows != packed.count {
            return Err(ArchiveVectorError::RowCountMismatch);
        }
        if packed_vectors.is_pruned(a.packed_vector_id) {
            return Err(crate::PackedVectorError::PayloadUnavailable(
                crate::PayloadUnavailable::Pruned,
            )
            .into());
        }
        ensure_unique(&a.fragment_ids)?;
        let id = archive_vector_id(a.packed_vector_id, &a.fragment_ids);
        if let Some(e) = self.objects.get(&id) {
            let p = c.read(e.chunk)?;
            let d = decode_archive_object(&p)?.ok_or(ArchiveVectorError::MissingObject)?;
            if d.id != id
                || d.packed_vector_id != a.packed_vector_id
                || d.fragment_ids != a.fragment_ids
                || e.fragment_ids != a.fragment_ids
            {
                return Err(ArchiveVectorError::HashCollision);
            }
            return Ok(id);
        }
        let p = encode_archive_object(id, a.packed_vector_id, &a.fragment_ids)?;
        let chunk = c.append(&p)?;
        let info = ArchiveVectorInfo {
            id,
            packed_vector_id: a.packed_vector_id,
            rows,
            max_fragment_archive_version: a.max_fragment_archive_version,
        };
        self.objects.insert(
            id,
            Entry {
                info,
                chunk,
                fragment_ids: a.fragment_ids,
            },
        );
        Ok(id)
    }
    pub fn get(
        &self,
        c: &mut Container,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorRecord, ArchiveVectorError> {
        let e = self
            .objects
            .get(&id)
            .ok_or(ArchiveVectorError::MissingObject)?;
        let p = c.read(e.chunk)?;
        let d = decode_archive_object(&p)?.ok_or(ArchiveVectorError::MissingObject)?;
        if d.id != id
            || archive_vector_id(d.packed_vector_id, &d.fragment_ids) != id
            || d.fragment_ids != e.fragment_ids
        {
            return Err(ArchiveVectorError::HashCollision);
        }
        Ok(ArchiveVectorRecord {
            id,
            packed_vector_id: d.packed_vector_id,
            fragment_ids: d.fragment_ids,
        })
    }
    pub fn insert_rebuilt(
        &mut self,
        info: ArchiveVectorInfo,
        chunk: ObjectRef,
        ids: Vec<ArchiveFragmentId>,
    ) -> Result<(), ArchiveVectorError> {
        if info.rows != u64::try_from(ids.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?
            || archive_vector_id(info.packed_vector_id, &ids) != info.id
        {
            return Err(ArchiveVectorError::HashCollision);
        }
        ensure_unique(&ids)?;
        if let Some(e) = self.objects.get(&info.id) {
            return if e.info == info && e.fragment_ids == ids {
                Ok(())
            } else {
                Err(ArchiveVectorError::HashCollision)
            };
        }
        self.objects.insert(
            info.id,
            Entry {
                info,
                chunk,
                fragment_ids: ids,
            },
        );
        Ok(())
    }
    pub fn info(&self, id: ArchiveVectorId) -> Option<ArchiveVectorInfo> {
        self.objects.get(&id).map(|e| e.info)
    }
    pub fn has_exact_fragment_coverage(
        &self,
        id: ArchiveVectorId,
        expected: &[ArchiveFragmentId],
    ) -> bool {
        let Some(e) = self.objects.get(&id) else {
            return false;
        };
        if e.fragment_ids.len() != expected.len() {
            return false;
        }
        let set: HashSet<_> = expected.iter().copied().collect();
        set.len() == e.fragment_ids.len() && e.fragment_ids.iter().all(|x| set.contains(x))
    }
    pub fn infos(&self) -> Vec<ArchiveVectorInfo> {
        let mut x: Vec<_> = self.objects.values().map(|e| e.info).collect();
        x.sort_by_key(|i| i.id.0);
        x
    }
    pub fn stats(&self) -> ArchiveVectorStats {
        ArchiveVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|e| e.info.rows).sum(),
        }
    }
}
pub fn archive_vector_id(p: PackedVectorId, ids: &[ArchiveFragmentId]) -> ArchiveVectorId {
    let mut h = Sha256::new();
    h.update(b"CVA-ARCHIVE-VECTORS-V1\0");
    h.update(p.0);
    for x in ids {
        h.update(x.0)
    }
    ArchiveVectorId(h.finalize().into())
}
fn ensure_unique(ids: &[ArchiveFragmentId]) -> Result<(), ArchiveVectorError> {
    let mut s = HashSet::with_capacity(ids.len());
    for x in ids {
        if !s.insert(*x) {
            return Err(ArchiveVectorError::DuplicateFragment);
        }
    }
    Ok(())
}

pub struct ArchiveVectorOpenState {
    records: Vec<Pending>,
    format_seen: bool,
}
pub struct ArchiveVectorReplayRecord {
    pub info: ArchiveVectorInfo,
    pub chunk: ObjectRef,
    pub fragment_ids: Vec<ArchiveFragmentId>,
}
struct Pending {
    info: ArchiveVectorInfo,
    chunk: ObjectRef,
    ids: Vec<ArchiveFragmentId>,
}
impl ArchiveVectorOpenState {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }
    pub fn ingest(&mut self, chunk: ObjectRef, p: &[u8]) -> Result<(), ArchiveVectorError> {
        if decode_archive_format(p)? {
            if self.format_seen {
                return Err(ArchiveVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        let Some(d) = decode_archive_object(p)? else {
            return Ok(());
        };
        if archive_vector_id(d.packed_vector_id, &d.fragment_ids) != d.id {
            return Err(ArchiveVectorError::HashCollision);
        }
        let rows =
            u64::try_from(d.fragment_ids.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?;
        self.records.push(Pending {
            info: ArchiveVectorInfo {
                id: d.id,
                packed_vector_id: d.packed_vector_id,
                rows,
                max_fragment_archive_version: 0,
            },
            chunk,
            ids: d.fragment_ids,
        });
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<ArchiveVectorReplayRecord>, ArchiveVectorError> {
        if !self.format_seen {
            return Err(ArchiveVectorError::MissingFormat);
        }
        Ok(self
            .records
            .into_iter()
            .map(|p| ArchiveVectorReplayRecord {
                info: p.info,
                chunk: p.chunk,
                fragment_ids: p.ids,
            })
            .collect())
    }
}
pub fn archive_vector_object_packed_id(
    p: &[u8],
) -> Result<Option<(ArchiveVectorId, PackedVectorId)>, ArchiveVectorError> {
    Ok(decode_archive_object(p)?.map(|d| (d.id, d.packed_vector_id)))
}
