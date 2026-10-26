//! Read opaque prune-proof metadata from compact Entity carriers.
//!
//! This only extracts the proof's carrier identity and opaque digests. The
//! proof does not authorize admission until file-level manifest validation.
use crate::entity_compact_contract::EntityDescriptionProof;
use crate::{EntityError, EntityId};

pub fn compact_description_proof(
    bytes: &[u8],
) -> Result<Option<(EntityId, u64, EntityDescriptionProof)>, EntityError> {
    use crate::entity_compact_codec::publication::{self, EntityPublication};
    let envelope = reliquary_container::publication_envelope::decode(bytes)?;
    let (body, global) = envelope
        .map(|e| (e.body, e.global_version))
        .unwrap_or((bytes, 1));
    if !body.starts_with(b"CVAENC") && !body.starts_with(b"CVAEND") {
        return Ok(None);
    }
    // Bare current bodies remain unpublished. Probe coordinates only decode
    // opaque proof metadata; file-level validation still owns authorization.
    Ok(match publication::decode(body, global)? {
        Some(EntityPublication::Creation(value)) => value
            .description_proof
            .map(|proof| (value.record.id, value.record.revision, proof)),
        Some(EntityPublication::Delta(value)) => value
            .description_proof
            .map(|proof| (value.id, value.revision, proof)),
        _ => None,
    })
}

#[cfg(test)]
#[path = "entity_description_proof_tests.rs"]
mod tests;
