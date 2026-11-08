//! Owner-local Memory history for REL/PHY files.
//!
//! This slice owns immutable body identity, deterministic Memory identity,
//! provenance, metadata revisions, terminal tombstones, and historical replay.
//! Physical framing and writer leases remain in `reliquary-container`.

mod codec;
mod error;
mod model;
mod store;

pub use error::MemoryError;
pub use model::*;
pub use store::{
    GroupedManagementEffect, GroupedMemoryInitialPromotion, GroupedMemoryLifecycle,
    GroupedMemoryReplay, GroupedMemorySupersession, MemoryStore, PreparedMemoryCorrection,
};

/// Encode a Memory creation for embedding in a grouped owner carrier.
/// Grouped Insomnia births are revision-one records; other revisions reject.
pub fn encode_grouped_creation(record: &MemoryRecord) -> Result<Vec<u8>, MemoryError> {
    if record.revision != 1 {
        return Err(MemoryError::Corrupt("grouped creation is not revision one"));
    }
    codec::encode_creation(record)
}

/// Decode and validate a Memory creation embedded in an owner carrier.
pub fn decode_grouped_creation(bytes: &[u8]) -> Result<MemoryRecord, MemoryError> {
    if bytes.get(..8) != Some(codec::CREATION.as_slice()) {
        return Err(MemoryError::Corrupt(
            "invalid grouped Memory creation marker",
        ));
    }
    let record = codec::decode_creation(bytes)?;
    if record.revision != 1 {
        return Err(MemoryError::Corrupt("grouped creation is not revision one"));
    }
    Ok(record)
}

/// Read the owner-local Memory clock from a revision or tombstone frame.
/// Full semantic validation remains the responsibility of Memory replay.
pub fn history_change_in_frame(bytes: &[u8]) -> Result<Option<(MemoryId, u64)>, MemoryError> {
    if bytes.starts_with(codec::DELTA) {
        let delta = codec::decode_delta(bytes)?;
        Ok(Some((delta.id, delta.record.memory_version)))
    } else if bytes.starts_with(codec::TOMBSTONE) {
        let (id, _, version) = codec::decode_tombstone(bytes)?;
        Ok(Some((id, version)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;

/// Check draft fields before an owner records a recoverable publication intent.
pub fn validate_memory_draft(draft: &MemoryDraft) -> Result<(), MemoryError> {
    codec::validate_draft(draft)
}
