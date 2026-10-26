//! Read opaque prune-proof metadata from compact Relationship carriers.
//!
//! This only extracts the proof's carrier identity and opaque digests. The
//! proof does not authorize admission until file-level manifest validation.
use crate::relationship_compact_contract::RelationshipDescriptionProof;
use crate::{RelationshipError, RelationshipId};

pub fn compact_description_proof(
    bytes: &[u8],
) -> Result<Option<(RelationshipId, u64, RelationshipDescriptionProof)>, RelationshipError> {
    use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
    let envelope = reliquary_container::publication_envelope::decode(bytes)?;
    let (body, global) = envelope
        .map(|e| (e.body, e.global_version))
        .unwrap_or((bytes, 1));
    if !body.starts_with(b"CVARNC") && !body.starts_with(b"CVARND") {
        return Ok(None);
    }
    // Bare current bodies remain unpublished. Probe coordinates only decode
    // opaque proof metadata; file-level validation still owns authorization.
    Ok(match publication::decode(body, global)? {
        Some(RelationshipPublication::Creation(value)) => value
            .description_proof
            .map(|proof| (value.record.id, value.record.revision, proof)),
        Some(RelationshipPublication::Delta(value)) => value
            .description_proof
            .map(|proof| (value.id, value.revision, proof)),
        _ => None,
    })
}

#[cfg(test)]
#[path = "relationship_description_proof_tests.rs"]
mod tests;
