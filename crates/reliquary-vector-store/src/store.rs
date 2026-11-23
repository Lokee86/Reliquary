use crate::codec::{encode_format, encode_object};
use crate::{
    ArchiveFragmentId, ArchiveVectorId, CompatibilityProfileId, MemoryBodyId, MemoryVectorId,
    PackedVectorError, PackedVectorId, PackedVectorInfo, PackedVectorStats,
};
use lodestone_packed::{PackedVectors, VectorSchema};
use reliquary_container::{Container, LeasedFrameReader, ObjectRef};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

/// Identify a packed payload or retained pruned marker in a physical frame.
pub fn packed_id_in_frame(payload: &[u8]) -> Result<Option<PackedVectorId>, PackedVectorError> {
    if let Some(marker) = crate::codec::decode_pruned_marker(payload)? {
        return Ok(Some(marker.info.id));
    }
    Ok(crate::codec::decode_object(payload)?.map(|record| record.id))
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct PackedVectorStore {
    objects: HashMap<PackedVectorId, PackedVectorEntry>,
    #[serde(skip)]
    reader: Option<LeasedFrameReader>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PackedVectorEntry {
    info: PackedVectorInfo,
    chunk: Option<ObjectRef>,
    #[serde(skip)]
    packed: Option<Arc<PackedVectors>>,
    backing: Option<PackedVectorBacking>,
    marker_chunk: Option<ObjectRef>,
    manifest_digest: Option<[u8; 32]>,
}

const MATRIX_PAGE_BYTES: usize = 64 * 1024;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PackedVectorBacking {
    frame_digest: [u8; 32],
    pages: Vec<[u8; 32]>,
}

impl PackedVectorBacking {
    fn from_object(payload: &[u8]) -> Self {
        Self {
            frame_digest: Sha256::digest(payload).into(),
            pages: payload[crate::codec::OBJECT_HEADER_LEN..]
                .chunks(MATRIX_PAGE_BYTES)
                .map(|page| Sha256::digest(page).into())
                .collect(),
        }
    }

    fn read_row(
        &self,
        reader: &LeasedFrameReader,
        object: ObjectRef,
        start: usize,
        end: usize,
        matrix_len: usize,
    ) -> Result<Vec<u8>, PackedVectorError> {
        let mut row = Vec::with_capacity(end - start);
        for page_index in start / MATRIX_PAGE_BYTES..=(end - 1) / MATRIX_PAGE_BYTES {
            let page_start = page_index * MATRIX_PAGE_BYTES;
            let page_end = matrix_len.min(page_start + MATRIX_PAGE_BYTES);
            let digest = *self
                .pages
                .get(page_index)
                .ok_or(PackedVectorError::CorruptRecord(
                    "packed-vector page missing",
                ))?;
            let bytes = reader.read_frame_range(
                object,
                (crate::codec::OBJECT_HEADER_LEN + page_start) as u64,
                (page_end - page_start) as u64,
                digest,
            )?;
            row.extend_from_slice(
                &bytes[start.max(page_start) - page_start..end.min(page_end) - page_start],
            );
        }
        Ok(row)
    }
}

/// A shared reference to an immutable, digest-validated owner matrix.
pub struct PackedVectorView(Arc<PackedVectors>);

impl PackedVectorView {
    /// Borrow one encoded row without materializing float values.
    pub fn row(&self, ordinal: usize) -> Option<&[u8]> {
        self.0.row(ordinal)
    }

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
        let entry = self.entry(id)?;
        if entry.info.schema.scalar != lodestone_packed::ScalarType::F32 {
            return Err(PackedVectorError::CorruptRecord(
                "packed-vector row backing",
            ));
        }
        let index = usize::try_from(row)
            .map_err(|_| PackedVectorError::CorruptRecord("packed-vector row index"))?;
        let floats = |bytes: &[u8]| {
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .copied()
                .map(f32::from_le_bytes)
                .collect()
        };
        if let Some(packed) = &entry.packed {
            return packed
                .row(index)
                .map(floats)
                .ok_or(PackedVectorError::CorruptRecord("packed-vector row index"));
        }
        let row_len = entry
            .info
            .schema
            .row_bytes()
            .map_err(|_| PackedVectorError::SizeOverflow)?;
        let matrix_len =
            usize::try_from(entry.info.byte_len).map_err(|_| PackedVectorError::SizeOverflow)?;
        let start = index
            .checked_mul(row_len)
            .ok_or(PackedVectorError::SizeOverflow)?;
        let end = start
            .checked_add(row_len)
            .filter(|end| *end <= matrix_len)
            .ok_or(PackedVectorError::CorruptRecord("packed-vector row index"))?;
        let (reader, object, backing) = self.backing(entry)?;
        let bytes = backing
            .read_row(reader, object, start, end, matrix_len)
            .inspect_err(|_| reader.fence_writes())?;
        Ok(floats(&bytes))
    }

    pub fn view(&self, id: PackedVectorId) -> Result<PackedVectorView, PackedVectorError> {
        Ok(PackedVectorView(self.matrix(id)?))
    }

    fn entry(&self, id: PackedVectorId) -> Result<&PackedVectorEntry, PackedVectorError> {
        self.objects
            .get(&id)
            .ok_or(PackedVectorError::MissingObject)
    }

    fn backing<'a>(
        &'a self,
        entry: &'a PackedVectorEntry,
    ) -> Result<(&'a LeasedFrameReader, ObjectRef, &'a PackedVectorBacking), PackedVectorError>
    {
        let object = entry.chunk.ok_or(PackedVectorError::PayloadUnavailable(
            crate::PayloadUnavailable::Pruned,
        ))?;
        let backing = entry
            .backing
            .as_ref()
            .ok_or(PackedVectorError::CorruptRecord("packed-vector locator"))?;
        let reader = self
            .reader
            .as_ref()
            .ok_or(PackedVectorError::CorruptRecord(
                "packed-vector owner lease",
            ))?;
        Ok((reader, object, backing))
    }

    fn matrix(&self, id: PackedVectorId) -> Result<Arc<PackedVectors>, PackedVectorError> {
        let entry = self.entry(id)?;
        if let Some(packed) = &entry.packed {
            return Ok(Arc::clone(packed));
        }
        let (reader, object, backing) = self.backing(entry)?;
        let payload = reader.read_frame(object, backing.frame_digest)?;
        let decode = || -> Result<Arc<PackedVectors>, PackedVectorError> {
            let value = crate::codec::decode_object(&payload)?
                .filter(|value| {
                    value.id == id
                        && value.schema == entry.info.schema
                        && value.count == entry.info.count
                })
                .ok_or(PackedVectorError::CorruptRecord(
                    "packed-vector locator identity",
                ))?;
            let rows = PackedVectors::from_bytes(value.schema, value.bytes.to_vec())
                .map_err(|_| PackedVectorError::CorruptRecord("packed-vector matrix"))?;
            Ok(Arc::new(rows))
        };
        decode().inspect_err(|_| reader.fence_writes())
    }

    /// Keep the admitted coordinates and defer reading matrices through this lease.
    pub fn bind_read_lease(&mut self, container: &Container) -> Result<(), PackedVectorError> {
        self.reader = Some(container.leased_frame_reader()?);
        for entry in self
            .objects
            .values_mut()
            .filter(|entry| entry.chunk.is_some())
        {
            if entry.backing.is_none() {
                return Err(PackedVectorError::CorruptRecord("packed-vector locator"));
            }
            entry.packed = None;
        }
        Ok(())
    }

    pub fn open(container: &Container) -> Result<Self, PackedVectorError> {
        let mut state = crate::PackedVectorOpenState::new();
        let mut frames = container.scan_frames()?;
        while let Some(frame) = frames.next_frame()? {
            state.ingest(frame.object, frame.payload)?;
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
            let existing_packed = self.matrix(id)?;
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
                packed: self.reader.is_none().then(|| Arc::new(packed)),
                backing: Some(PackedVectorBacking::from_object(&payload)),
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
                backing: None,
                marker_chunk: None,
                manifest_digest: None,
            },
        );
        Ok(())
    }

    pub(crate) fn retain_locator(&mut self, id: PackedVectorId, payload: &[u8]) {
        let entry = self
            .objects
            .get_mut(&id)
            .expect("admitted packed-vector backing");
        if entry.backing.is_none() {
            entry.backing = Some(PackedVectorBacking::from_object(payload));
        }
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
                backing: None,
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
