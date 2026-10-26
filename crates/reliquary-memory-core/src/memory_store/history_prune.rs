//! Memory-owned unavailable markers for destructively pruned body chunks.
use crate::MemoryError;
use crate::memory_model::MemoryBodyId;
use reliquary_container::ObjectRef;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const MARKER_MAGIC: &[u8; 8] = b"CVAMPRN1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrunedMemoryBody {
    pub id: MemoryBodyId,
    pub byte_length: u64,
    pub manifest_digest: [u8; 32],
    pub chunk: ObjectRef,
}

impl PrunedMemoryBody {
    pub fn encode(id: MemoryBodyId, byte_length: u64, manifest_digest: [u8; 32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(112);
        bytes.extend_from_slice(MARKER_MAGIC);
        bytes.extend_from_slice(&id.0);
        bytes.extend_from_slice(&byte_length.to_le_bytes());
        bytes.extend_from_slice(&manifest_digest);
        bytes.extend_from_slice(&Sha256::digest(&bytes));
        bytes
    }

    fn decode(chunk: ObjectRef, bytes: &[u8]) -> Result<Self, MemoryError> {
        if bytes.len() != 112 || Sha256::digest(&bytes[..80]).as_slice() != &bytes[80..] {
            return Err(MemoryError::CorruptRecord("pruned body marker"));
        }
        let marker = Self {
            id: MemoryBodyId(bytes[8..40].try_into().unwrap()),
            byte_length: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            manifest_digest: bytes[48..80].try_into().unwrap(),
            chunk,
        };
        if marker.manifest_digest == [0; 32] {
            return Err(MemoryError::CorruptRecord("pruned body marker digest"));
        }
        Ok(marker)
    }
}

#[derive(Clone, Default)]
pub(crate) struct MemoryPruneState {
    markers: HashMap<MemoryBodyId, PrunedMemoryBody>,
}

impl MemoryPruneState {
    pub(crate) fn ingest(&mut self, chunk: ObjectRef, bytes: &[u8]) -> Result<bool, MemoryError> {
        if !bytes.starts_with(MARKER_MAGIC) {
            return Ok(false);
        }
        let marker = PrunedMemoryBody::decode(chunk, bytes)?;
        if self.markers.contains_key(&marker.id) {
            return Err(MemoryError::CorruptRecord("duplicate pruned body marker"));
        }
        self.markers.insert(marker.id, marker);
        Ok(true)
    }

    pub(crate) fn get(&self, id: MemoryBodyId) -> Option<&PrunedMemoryBody> {
        self.markers.get(&id)
    }

    pub(crate) fn markers(&self) -> impl Iterator<Item = &PrunedMemoryBody> {
        self.markers.values()
    }
}
