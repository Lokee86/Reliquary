use crate::archive_codec::{
    decode_archive_format, decode_archive_object, encode_archive_format, encode_archive_object,
};
use crate::{
    ArchiveFragmentId, ArchiveVectorError, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorRecord,
    ArchiveVectorStats, PackedVectorId, PackedVectorStore,
};
use reliquary_container::{Container, ObjectRef};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct AcceptedArchiveMapping {
    pub packed_vector_id: PackedVectorId,
    pub packed_rows: u64,
    pub fragment_ids: Vec<ArchiveFragmentId>,
    pub max_fragment_archive_version: u64,
}

#[derive(Clone)]
struct Entry {
    info: ArchiveVectorInfo,
    chunk: ObjectRef,
    fragment_ids: Vec<ArchiveFragmentId>,
}

#[derive(Default)]
pub struct ArchiveVectorStore {
    objects: HashMap<ArchiveVectorId, Entry>,
}

impl ArchiveVectorStore {
    pub fn initialize(&self, container: &mut Container) -> Result<(), ArchiveVectorError> {
        let format = encode_archive_format();
        if crate::has_format(container, &format)? {
            return Err(ArchiveVectorError::ConflictingFormat);
        }
        container.append(&format)?;
        Ok(())
    }

    pub fn open(
        container: &Container,
        packed: &PackedVectorStore,
    ) -> Result<Self, ArchiveVectorError> {
        let mut replay = ArchiveVectorOpenState::new();
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let mut payload = Vec::new();
            std::io::Read::read_to_end(&mut frame, &mut payload)
                .map_err(|_| ArchiveVectorError::CorruptRecord("archive vector frame read"))?;
            replay.ingest(object, &payload)?;
        }
        let mut store = Self::default();
        for replayed in replay.finish()? {
            validate_packed_backing(packed, replayed.info)?;
            store.insert(replayed.info, replayed.chunk, replayed.fragment_ids)?;
        }
        Ok(store)
    }

    pub fn put(
        &mut self,
        container: &mut Container,
        packed: &PackedVectorStore,
        accepted: AcceptedArchiveMapping,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        let packed_info = packed
            .info(accepted.packed_vector_id)
            .ok_or(ArchiveVectorError::MissingPackedVector)?;
        if packed.is_pruned(accepted.packed_vector_id) {
            return Err(crate::PackedVectorError::PayloadUnavailable(
                crate::PayloadUnavailable::Pruned,
            )
            .into());
        }
        let rows = u64::try_from(accepted.fragment_ids.len())
            .map_err(|_| ArchiveVectorError::SizeOverflow)?;
        if rows != accepted.packed_rows || rows != packed_info.count {
            return Err(ArchiveVectorError::RowCountMismatch);
        }
        ensure_unique(&accepted.fragment_ids)?;
        let id = crate::archive_vector_id(accepted.packed_vector_id, &accepted.fragment_ids);
        if let Some(existing) = self.objects.get(&id) {
            if existing.info.packed_vector_id != accepted.packed_vector_id
                || existing.info.max_fragment_archive_version
                    != accepted.max_fragment_archive_version
                || existing.info.rows != rows
                || existing.fragment_ids != accepted.fragment_ids
            {
                return Err(ArchiveVectorError::HashCollision);
            }
            return Ok(id);
        }
        let payload = encode_archive_object(
            id,
            accepted.packed_vector_id,
            &accepted.fragment_ids,
            accepted.max_fragment_archive_version,
        )?;
        let chunk = container.append(&payload)?;
        self.insert(
            ArchiveVectorInfo {
                id,
                packed_vector_id: accepted.packed_vector_id,
                rows,
                max_fragment_archive_version: accepted.max_fragment_archive_version,
            },
            chunk,
            accepted.fragment_ids,
        )?;
        Ok(id)
    }

    fn insert(
        &mut self,
        info: ArchiveVectorInfo,
        chunk: ObjectRef,
        fragments: Vec<ArchiveFragmentId>,
    ) -> Result<(), ArchiveVectorError> {
        ensure_unique(&fragments)?;
        if info.rows
            != u64::try_from(fragments.len()).map_err(|_| ArchiveVectorError::SizeOverflow)?
            || crate::archive_vector_id(info.packed_vector_id, &fragments) != info.id
        {
            return Err(ArchiveVectorError::HashCollision);
        }
        if let Some(existing) = self.objects.get(&info.id) {
            return if existing.info == info && existing.fragment_ids == fragments {
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
                fragment_ids: fragments,
            },
        );
        Ok(())
    }

    pub fn get(&self, id: ArchiveVectorId) -> Result<ArchiveVectorRecord, ArchiveVectorError> {
        let entry = self
            .objects
            .get(&id)
            .ok_or(ArchiveVectorError::MissingObject)?;
        if crate::archive_vector_id(entry.info.packed_vector_id, &entry.fragment_ids) != id {
            return Err(ArchiveVectorError::HashCollision);
        }
        Ok(ArchiveVectorRecord {
            id,
            packed_vector_id: entry.info.packed_vector_id,
            fragment_ids: entry.fragment_ids.clone(),
        })
    }

    pub fn info(&self, id: ArchiveVectorId) -> Option<ArchiveVectorInfo> {
        self.objects.get(&id).map(|entry| entry.info)
    }

    pub fn object_ref(&self, id: ArchiveVectorId) -> Option<ObjectRef> {
        self.objects.get(&id).map(|entry| entry.chunk)
    }

    pub fn infos(&self) -> Vec<ArchiveVectorInfo> {
        let mut infos: Vec<_> = self.objects.values().map(|entry| entry.info).collect();
        infos.sort_by_key(|info| info.id.0);
        infos
    }

    pub fn has_exact_fragment_coverage(
        &self,
        id: ArchiveVectorId,
        expected: &[ArchiveFragmentId],
    ) -> bool {
        let Some(entry) = self.objects.get(&id) else {
            return false;
        };
        let expected: HashSet<_> = expected.iter().copied().collect();
        expected.len() == entry.fragment_ids.len()
            && entry
                .fragment_ids
                .iter()
                .all(|fragment| expected.contains(fragment))
    }

    pub fn stats(&self) -> ArchiveVectorStats {
        ArchiveVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|entry| entry.info.rows).sum(),
        }
    }
}

fn validate_packed_backing(
    packed: &PackedVectorStore,
    info: ArchiveVectorInfo,
) -> Result<(), ArchiveVectorError> {
    let backing = packed
        .info(info.packed_vector_id)
        .ok_or(ArchiveVectorError::MissingPackedVector)?;
    if backing.count != info.rows {
        return Err(ArchiveVectorError::RowCountMismatch);
    }
    Ok(())
}

fn ensure_unique(fragments: &[ArchiveFragmentId]) -> Result<(), ArchiveVectorError> {
    let mut seen = HashSet::with_capacity(fragments.len());
    if fragments.iter().any(|fragment| !seen.insert(*fragment)) {
        return Err(ArchiveVectorError::DuplicateFragment);
    }
    Ok(())
}

pub struct ArchiveVectorOpenState {
    records: Vec<ArchiveVectorReplayRecord>,
    format_seen: bool,
}

pub struct ArchiveVectorReplayRecord {
    pub info: ArchiveVectorInfo,
    pub chunk: ObjectRef,
    pub fragment_ids: Vec<ArchiveFragmentId>,
}

impl ArchiveVectorOpenState {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            format_seen: false,
        }
    }

    pub fn ingest(&mut self, chunk: ObjectRef, payload: &[u8]) -> Result<(), ArchiveVectorError> {
        if decode_archive_format(payload)? {
            if self.format_seen {
                return Err(ArchiveVectorError::ConflictingFormat);
            }
            self.format_seen = true;
            return Ok(());
        }
        let Some(decoded) = decode_archive_object(payload)? else {
            return Ok(());
        };
        if crate::archive_vector_id(decoded.packed_vector_id, &decoded.fragment_ids) != decoded.id {
            return Err(ArchiveVectorError::HashCollision);
        }
        self.records.push(ArchiveVectorReplayRecord {
            info: ArchiveVectorInfo {
                id: decoded.id,
                packed_vector_id: decoded.packed_vector_id,
                rows: u64::try_from(decoded.fragment_ids.len())
                    .map_err(|_| ArchiveVectorError::SizeOverflow)?,
                max_fragment_archive_version: decoded.max_fragment_archive_version,
            },
            chunk,
            fragment_ids: decoded.fragment_ids,
        });
        Ok(())
    }

    pub fn finish(self) -> Result<Vec<ArchiveVectorReplayRecord>, ArchiveVectorError> {
        if !self.format_seen {
            return Err(ArchiveVectorError::MissingFormat);
        }
        Ok(self.records)
    }
}

impl Default for ArchiveVectorOpenState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn archive_vector_object_packed_id(
    payload: &[u8],
) -> Result<Option<(ArchiveVectorId, PackedVectorId)>, ArchiveVectorError> {
    Ok(decode_archive_object(payload)?.map(|decoded| (decoded.id, decoded.packed_vector_id)))
}
