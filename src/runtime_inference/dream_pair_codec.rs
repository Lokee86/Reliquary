//! Physical references local to one Dream pair stream, never semantic IDs.
use crate::{MemoryError, MemoryId};

pub(super) const MEMORY_MAGIC: &[u8; 8] = b"CVADRM01";
pub(super) const PAIR_MAGIC: &[u8; 8] = b"CVADRP02";

pub(super) enum Record {
    Memory(u32, MemoryId),
    Pair(u32, u32),
}

pub(super) fn encode_memory(reference: u32, id: MemoryId) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(MEMORY_MAGIC);
    bytes.extend_from_slice(&reference.to_le_bytes());
    bytes.extend_from_slice(&id.0);
    bytes
}

pub(super) fn encode_pair(left: u32, right: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(PAIR_MAGIC);
    bytes.extend_from_slice(&left.to_le_bytes());
    bytes.extend_from_slice(&right.to_le_bytes());
    bytes
}

pub(super) fn decode(bytes: &[u8]) -> Result<Option<Record>, MemoryError> {
    if bytes.starts_with(MEMORY_MAGIC) {
        if bytes.len() != 44 {
            return Err(MemoryError::CorruptRecord(
                "invalid Dream pair Memory mapping length",
            ));
        }
        let reference = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if reference == 0 {
            return Err(MemoryError::CorruptRecord(
                "zero Dream pair Memory reference",
            ));
        }
        return Ok(Some(Record::Memory(
            reference,
            MemoryId(bytes[12..44].try_into().unwrap()),
        )));
    }
    if bytes.starts_with(PAIR_MAGIC) {
        if bytes.len() != 16 {
            return Err(MemoryError::CorruptRecord(
                "invalid compact Dream pair length",
            ));
        }
        let left = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let right = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        if left == 0 || right == 0 || left == right {
            return Err(MemoryError::CorruptRecord(
                "invalid compact Dream pair references",
            ));
        }
        return Ok(Some(Record::Pair(left, right)));
    }
    if bytes.starts_with(b"CVADRM") || bytes.starts_with(b"CVADRP") {
        return Err(MemoryError::CorruptRecord(
            "unsupported Dream pair encoding",
        ));
    }
    Ok(None)
}
