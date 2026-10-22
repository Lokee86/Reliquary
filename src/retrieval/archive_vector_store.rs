use crate::archive_vector_codec::{decode_object, encode_format, encode_object};
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    Archive, ArchiveVectorError, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorSet,
    ArchiveVectorStats, Container, FragmentId, ObjectRef, PackedVectorId,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct ArchiveVectorStore {
    objects: HashMap<ArchiveVectorId, ArchiveVectorEntry>,
}

#[derive(Clone)]
struct ArchiveVectorEntry {
    info: ArchiveVectorInfo,
    chunk: ObjectRef,
    fragment_ids: Vec<FragmentId>,
}

impl ArchiveVectorStore {
    pub(crate) fn initialize(&self, container: &mut Container) -> Result<(), ArchiveVectorError> {
        container.append(&encode_format())?;
        Ok(())
    }

    pub(crate) fn put(
        &mut self,
        container: &mut Container,
        archive: &Archive,
        packed_vectors: &PackedVectorStore,
        packed_vector_id: PackedVectorId,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<ArchiveVectorId, ArchiveVectorError> {
        let max_fragment_archive_version =
            validate_mapping(archive, packed_vectors, packed_vector_id, &fragment_ids)?;
        if packed_vectors.is_pruned(packed_vector_id) {
            return Err(crate::PackedVectorError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned,
            )
            .into());
        }
        let id = archive_vector_id(packed_vector_id, &fragment_ids);
        if let Some(existing) = self.objects.get(&id) {
            let payload = container.read(existing.chunk)?;
            let decoded = decode_object(&payload)?.ok_or(ArchiveVectorError::MissingObject)?;
            if decoded.id != id
                || decoded.packed_vector_id != packed_vector_id
                || decoded.fragment_ids != fragment_ids
                || existing.fragment_ids != fragment_ids
            {
                return Err(ArchiveVectorError::HashCollision);
            }
            return Ok(id);
        }
        let payload = encode_object(id, packed_vector_id, &fragment_ids)?;
        let chunk = container.append(&payload)?;
        let info = ArchiveVectorInfo {
            id,
            packed_vector_id,
            rows: u64::try_from(fragment_ids.len())
                .map_err(|_| ArchiveVectorError::SizeOverflow)?,
            max_fragment_archive_version,
        };
        self.objects.insert(
            id,
            ArchiveVectorEntry {
                info,
                chunk,
                fragment_ids,
            },
        );
        Ok(id)
    }

    pub(crate) fn get(
        &self,
        container: &mut Container,
        id: ArchiveVectorId,
    ) -> Result<ArchiveVectorSet, ArchiveVectorError> {
        let entry = self
            .objects
            .get(&id)
            .ok_or(ArchiveVectorError::MissingObject)?;
        let payload = container.read(entry.chunk)?;
        let decoded = decode_object(&payload)?.ok_or(ArchiveVectorError::MissingObject)?;
        if decoded.id != id
            || archive_vector_id(decoded.packed_vector_id, &decoded.fragment_ids) != id
            || decoded.fragment_ids != entry.fragment_ids
        {
            return Err(ArchiveVectorError::HashCollision);
        }
        Ok(ArchiveVectorSet {
            id,
            packed_vector_id: decoded.packed_vector_id,
            fragment_ids: decoded.fragment_ids,
        })
    }

    pub(crate) fn insert_rebuilt(
        &mut self,
        info: ArchiveVectorInfo,
        chunk: ObjectRef,
        fragment_ids: Vec<FragmentId>,
    ) -> Result<(), ArchiveVectorError> {
        if info.rows != fragment_ids.len() as u64
            || archive_vector_id(info.packed_vector_id, &fragment_ids) != info.id
        {
            return Err(ArchiveVectorError::HashCollision);
        }
        if let Some(existing) = self.objects.get(&info.id) {
            if existing.info != info || existing.fragment_ids != fragment_ids {
                return Err(ArchiveVectorError::HashCollision);
            }
            return Ok(());
        }
        self.objects.insert(
            info.id,
            ArchiveVectorEntry {
                info,
                chunk,
                fragment_ids,
            },
        );
        Ok(())
    }

    pub(crate) fn info(&self, id: ArchiveVectorId) -> Option<ArchiveVectorInfo> {
        self.objects.get(&id).map(|entry| entry.info)
    }

    pub(crate) fn has_exact_fragment_coverage(
        &self,
        id: ArchiveVectorId,
        expected: &[FragmentId],
    ) -> bool {
        let Some(entry) = self.objects.get(&id) else {
            return false;
        };
        if entry.fragment_ids.len() != expected.len() {
            return false;
        }
        let expected: HashSet<_> = expected.iter().copied().collect();
        expected.len() == entry.fragment_ids.len()
            && entry
                .fragment_ids
                .iter()
                .all(|fragment| expected.contains(fragment))
    }

    pub(crate) fn infos(&self) -> Vec<ArchiveVectorInfo> {
        let mut infos: Vec<_> = self.objects.values().map(|entry| entry.info).collect();
        infos.sort_by_key(|info| info.id.0);
        infos
    }

    pub(crate) fn stats(&self) -> ArchiveVectorStats {
        ArchiveVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|entry| entry.info.rows).sum(),
        }
    }
}

pub(crate) fn validate_mapping(
    archive: &Archive,
    packed_vectors: &PackedVectorStore,
    packed_vector_id: PackedVectorId,
    fragment_ids: &[FragmentId],
) -> Result<u64, ArchiveVectorError> {
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
            .fragments
            .archive_version(*fragment_id)
            .ok_or(ArchiveVectorError::MissingFragment)?;
        max_archive_version = max_archive_version.max(archive_version);
        if !seen.insert(*fragment_id) {
            return Err(ArchiveVectorError::DuplicateFragment);
        }
    }
    Ok(max_archive_version)
}

pub(crate) fn archive_vector_id(
    packed_vector_id: PackedVectorId,
    fragment_ids: &[FragmentId],
) -> ArchiveVectorId {
    let mut hash = Sha256::new();
    hash.update(b"CVA-ARCHIVE-VECTORS-V1\0");
    hash.update(packed_vector_id.0);
    for fragment_id in fragment_ids {
        hash.update(fragment_id.0);
    }
    ArchiveVectorId(hash.finalize().into())
}
