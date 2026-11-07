//! Owner-local Memory history for REL/PHY files.
//!
//! This slice owns immutable body identity, deterministic Memory identity,
//! provenance, metadata revisions, terminal tombstones, and historical replay.
//! Physical framing and writer leases remain in `reliquary-container`.

mod codec;
mod error;
mod model;
mod routing;
mod store;

pub use routing::{
    MemoryRoutingField, MemoryRoutingMention, MemoryRoutingMetadata, decode_routing_metadata,
    encode_routing_metadata,
};

pub use error::MemoryError;
pub use model::*;
pub use store::{
    GroupedMemoryInitialPromotion, GroupedMemoryLifecycle, GroupedMemoryReplay,
    GroupedMemorySupersession, MemoryStore,
};

/// Encode an immutable retained Memory body after verifying its stable ID.
pub fn encode_retained_memory_body(
    id: MemoryBodyId,
    body: &MemoryBody,
) -> Result<Vec<u8>, MemoryError> {
    if memory_body_id(&body.title, &body.content) != id {
        return Err(MemoryError::Corrupt(
            "retained Memory body identity differs",
        ));
    }
    codec::encode_body(id, body)
}

/// Encode an already accepted ungrouped Memory revision using native record semantics.
pub fn encode_retained_memory_revision(
    previous: Option<&MemoryRecord>,
    accepted: &MemoryRecord,
    body: &MemoryBody,
) -> Result<Vec<u8>, MemoryError> {
    if memory_body_id(&body.title, &body.content) != accepted.body_id
        || accepted.memory_version == 0
    {
        return Err(MemoryError::Corrupt("retained Memory identity differs"));
    }
    codec::validate_replayed_record(accepted, Some(body))?;
    if let Some(previous) = previous {
        return codec::encode_delta(previous, accepted);
    }
    if accepted.revision != 1 || memory_id(&accepted.mutation_id) != accepted.id {
        return Err(MemoryError::Corrupt(
            "retained first Memory identity differs",
        ));
    }
    codec::encode_creation(accepted)
}

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
