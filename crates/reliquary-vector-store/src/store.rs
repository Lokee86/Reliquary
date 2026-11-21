use crate::codec::{encode_format, encode_object};
use crate::{
    ArchiveFragmentId, ArchiveVectorId, CompatibilityProfileId, MemoryBodyId, MemoryVectorId,
    PackedVectorError, PackedVectorId, PackedVectorInfo, PackedVectorStats,
};
use lodestone_packed::{PackedVectors, VectorSchema};
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

/// Identify a packed payload or retained pruned marker in a physical frame.
pub fn packed_id_in_frame(payload: &[u8]) -> Result<Option<PackedVectorId>, PackedVectorError> {
    if let Some(marker) = crate::codec::decode_pruned_marker(payload)? {
        return Ok(Some(marker.info.id));
    }
    Ok(crate::codec::decode_object(payload)?.map(|record| record.id))
}

#[derive(Default)]
pub struct PackedVectorStore {
    objects: HashMap<PackedVectorId, PackedVectorEntry>,
}

#[derive(Clone)]
struct PackedVectorEntry {
    info: PackedVectorInfo,
    chunk: Option<ObjectRef>,
    packed: Option<Arc<PackedVectors>>,
    marker_chunk: Option<ObjectRef>,
    manifest_digest: Option<[u8; 32]>,
}

/// A shared reference to an immutable, digest-validated owner matrix.
pub struct PackedVectorView(Arc<PackedVectors>);

impl PackedVectorView {
    pub fn search_cosine(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<lodestone_core::PackedCosineHit>, PackedVectorError> {
        lodestone_core::search_packed_cosine(&self.0, query, limit)
            .map_err(PackedVectorError::Search)
    }
}

impl PackedVectorStore {
    /// Encode a bounded caller-validated matrix using the store's F32 format.
    pub fn put_f32_rows(
        &mut self,
        container: &mut Container,
        rows: &[Vec<f32>],
    ) -> Result<PackedVectorId, PackedVectorError> {
        let dimensions = rows
            .first()
            .ok_or(PackedVectorError::CorruptRecord("empty matrix"))?
            .len();
        if dimensions == 0
            || rows
                .iter()
                .any(|row| row.len() != dimensions || row.iter().any(|v| !v.is_finite()))
        {
            return Err(PackedVectorError::CorruptRecord("invalid F32 matrix"));
        }
        let dimensions = u32::try_from(dimensions).map_err(|_| PackedVectorError::SizeOverflow)?;
        let bytes = rows
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let schema = VectorSchema::new(dimensions, lodestone_packed::ScalarType::F32)
            .map_err(|_| PackedVectorError::CorruptRecord("matrix dimensions"))?;
        let packed = PackedVectors::from_bytes(schema, bytes)
            .map_err(|_| PackedVectorError::CorruptRecord("matrix bytes"))?;
        self.put(container, packed)
    }
    /// Read one retained F32 row without exposing the packed store or writer.
    pub fn read_f32_row(
        &self,
        id: PackedVectorId,
        row: u64,
    ) -> Result<Vec<f32>, PackedVectorError> {
        let packed = self.matrix(id)?;
        if packed.schema().scalar != lodestone_packed::ScalarType::F32 {
            return Err(PackedVectorError::CorruptRecord(
                "packed-vector row backing",
            ));
        }
        let index = usize::try_from(row)
            .map_err(|_| PackedVectorError::CorruptRecord("packed-vector row index"))?;
        let bytes = packed
            .row(index)
            .ok_or(PackedVectorError::CorruptRecord("packed-vector row index"))?;
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(f32::from_le_bytes)
            .collect())
    }

    pub fn view(&self, id: PackedVectorId) -> Result<PackedVectorView, PackedVectorError> {
        Ok(PackedVectorView(Arc::clone(self.matrix(id)?)))
    }

    fn matrix(&self, id: PackedVectorId) -> Result<&Arc<PackedVectors>, PackedVectorError> {
        self.objects
            .get(&id)
            .ok_or(PackedVectorError::MissingObject)?
            .packed
            .as_ref()
            .ok_or(PackedVectorError::PayloadUnavailable(
                crate::PayloadUnavailable::Pruned,
            ))
    }

    pub fn open(container: &Container) -> Result<Self, PackedVectorError> {
        let mut state = crate::PackedVectorOpenState::new();
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let mut payload = Vec::new();
            std::io::Read::read_to_end(&mut frame, &mut payload)
                .map_err(|_| PackedVectorError::CorruptRecord("packed-vector frame read"))?;
            state.ingest(object, &payload)?;
        }
        state.finish()
    }

    pub fn initialize(&self, container: &mut Container) -> Result<(), PackedVectorError> {
        let format = encode_format();
        if crate::has_format(container, &format)? {
            return Err(PackedVectorError::ConflictingFormat);
        }
        container.append(&format)?;
        Ok(())
    }

    pub fn put(
        &mut self,
        container: &mut Container,
        packed: PackedVectors,
    ) -> Result<PackedVectorId, PackedVectorError> {
        let id = packed_vector_id(packed.schema(), packed.bytes());
        if let Some(existing) = self.objects.get(&id) {
            let Some(existing_packed) = &existing.packed else {
                return Err(PackedVectorError::PayloadUnavailable(
                    crate::PayloadUnavailable::Pruned,
                ));
            };
            if existing.info.schema != packed.schema() || existing_packed.bytes() != packed.bytes()
            {
                return Err(PackedVectorError::HashCollision);
            }
            return Ok(id);
        }
        let payload = encode_object(id, &packed)?;
        let chunk = container.append(&payload)?;
        let info = info(id, packed.schema(), packed.count(), packed.bytes().len())?;
        self.objects.insert(
            id,
            PackedVectorEntry {
                info,
                chunk: Some(chunk),
                packed: Some(Arc::new(packed)),
                marker_chunk: None,
                manifest_digest: None,
            },
        );
        Ok(id)
    }

    pub fn get(
        &self,
        _container: &mut Container,
        id: PackedVectorId,
    ) -> Result<PackedVectors, PackedVectorError> {
        let packed = self.matrix(id)?;
        if packed_vector_id(packed.schema(), packed.bytes()) != id {
            return Err(PackedVectorError::HashCollision);
        }
        Ok(packed.as_ref().clone())
    }

    pub fn insert_rebuilt(
        &mut self,
        packed_info: PackedVectorInfo,
        chunk: ObjectRef,
        bytes: Vec<u8>,
    ) -> Result<(), PackedVectorError> {
        let packed = PackedVectors::from_bytes(packed_info.schema, bytes)
            .map_err(|_| PackedVectorError::CorruptRecord("packed-vector matrix"))?;
        let expected = info(
            packed_info.id,
            packed_info.schema,
            packed.count(),
            packed.bytes().len(),
        )?;
        if expected != packed_info {
            return Err(PackedVectorError::CorruptRecord(
                "packed-vector backing metadata",
            ));
        }
        if packed_vector_id(packed_info.schema, packed.bytes()) != packed_info.id {
            return Err(PackedVectorError::HashCollision);
        }
        if let Some(existing) = self.objects.get(&packed_info.id) {
            if existing.info != packed_info || existing.packed.as_deref() != Some(&packed) {
                return Err(PackedVectorError::HashCollision);
            }
            return Ok(());
        }
        self.objects.insert(
            packed_info.id,
            PackedVectorEntry {
                info: packed_info,
                chunk: Some(chunk),
                packed: Some(Arc::new(packed)),
                marker_chunk: None,
                manifest_digest: None,
            },
        );
        Ok(())
    }

    pub fn insert_pruned(
        &mut self,
        info: PackedVectorInfo,
        marker_chunk: ObjectRef,
        manifest_digest: [u8; 32],
    ) -> Result<(), PackedVectorError> {
        if manifest_digest == [0; 32] {
            return Err(PackedVectorError::CorruptRecord("pruned payload marker"));
        }
        if let Some(existing) = self.objects.get(&info.id) {
            return if existing.info == info && existing.manifest_digest == Some(manifest_digest) {
                Ok(())
            } else {
                Err(PackedVectorError::HashCollision)
            };
        }
        self.objects.insert(
            info.id,
            PackedVectorEntry {
                info,
                chunk: None,
                packed: None,
                marker_chunk: Some(marker_chunk),
                manifest_digest: Some(manifest_digest),
            },
        );
        Ok(())
    }

    pub fn is_pruned(&self, id: PackedVectorId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|entry| entry.chunk.is_none())
    }

    pub fn prune_manifest_digest(&self, id: PackedVectorId) -> Option<[u8; 32]> {
        self.objects
            .get(&id)
            .and_then(|entry| entry.manifest_digest)
    }

    pub fn marker_chunk(&self, id: PackedVectorId) -> Option<ObjectRef> {
        self.objects.get(&id).and_then(|entry| entry.marker_chunk)
    }

    pub fn marker_bytes(
        &self,
        id: PackedVectorId,
        manifest_digest: [u8; 32],
    ) -> Result<Vec<u8>, PackedVectorError> {
        let entry = self
            .objects
            .get(&id)
            .ok_or(PackedVectorError::MissingObject)?;
        crate::codec::encode_pruned_marker(entry.info, manifest_digest)
    }

    pub fn marker_matches(&self, id: PackedVectorId, manifest_digest: [u8; 32]) -> bool {
        self.objects.get(&id).is_some_and(|entry| {
            self.is_pruned(id) && entry.manifest_digest == Some(manifest_digest)
        })
    }

    pub fn pruned_ids(&self) -> Vec<PackedVectorId> {
        let mut ids: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(id, entry)| entry.chunk.is_none().then_some(*id))
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn info(&self, id: PackedVectorId) -> Option<PackedVectorInfo> {
        self.objects.get(&id).map(|entry| entry.info)
    }

    pub fn infos(&self) -> Vec<PackedVectorInfo> {
        let mut infos: Vec<_> = self.objects.values().map(|entry| entry.info).collect();
        infos.sort_by_key(|info| info.id.0);
        infos
    }

    pub fn stats(&self) -> PackedVectorStats {
        PackedVectorStats {
            objects: self.objects.len(),
            rows: self.objects.values().map(|entry| entry.info.count).sum(),
            matrix_bytes: self
                .objects
                .values()
                .filter(|entry| entry.chunk.is_some())
                .map(|entry| entry.info.byte_len)
                .sum(),
        }
    }
}

pub fn packed_vector_id(schema: VectorSchema, bytes: &[u8]) -> PackedVectorId {
    let mut hash = Sha256::new();
    hash.update([
        0x43, 0x56, 0x41, 0x2d, 0x50, 0x41, 0x43, 0x4b, 0x45, 0x44, 0x2d, 0x56, 0x45, 0x43, 0x54,
        0x4f, 0x52, 0x2d, 0x56, 0x31, 0x00,
    ]);
    hash.update(schema.dimensions.to_le_bytes());
    hash.update([schema.scalar.tag()]);
    hash.update(bytes);
    PackedVectorId(hash.finalize().into())
}

pub fn archive_vector_id(
    packed: PackedVectorId,
    fragments: &[ArchiveFragmentId],
) -> ArchiveVectorId {
    let mut hash = Sha256::new();
    hash.update([
        0x43, 0x56, 0x41, 0x2d, 0x41, 0x52, 0x43, 0x48, 0x49, 0x56, 0x45, 0x2d, 0x56, 0x45, 0x43,
        0x54, 0x4f, 0x52, 0x53, 0x2d, 0x56, 0x31, 0x00,
    ]);
    hash.update(packed.0);
    for fragment in fragments {
        hash.update(fragment.0);
    }
    ArchiveVectorId(hash.finalize().into())
}

pub fn memory_vector_id(
    profile: CompatibilityProfileId,
    packed: PackedVectorId,
    bodies: &[MemoryBodyId],
) -> MemoryVectorId {
    let mut hash = Sha256::new();
    hash.update([
        0x43, 0x56, 0x41, 0x2d, 0x4d, 0x45, 0x4d, 0x4f, 0x52, 0x59, 0x2d, 0x56, 0x45, 0x43, 0x54,
        0x4f, 0x52, 0x53, 0x2d, 0x56, 0x31, 0x00,
    ]);
    hash.update(profile.0);
    hash.update(packed.0);
    for body in bodies {
        hash.update(body.0);
    }
    MemoryVectorId(hash.finalize().into())
}

pub fn info(
    id: PackedVectorId,
    schema: VectorSchema,
    count: usize,
    byte_len: usize,
) -> Result<PackedVectorInfo, PackedVectorError> {
    Ok(PackedVectorInfo {
        id,
        schema,
        count: u64::try_from(count).map_err(|_| PackedVectorError::SizeOverflow)?,
        byte_len: u64::try_from(byte_len).map_err(|_| PackedVectorError::SizeOverflow)?,
    })
}
