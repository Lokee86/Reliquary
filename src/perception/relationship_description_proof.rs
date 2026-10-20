//! Read opaque prune-proof metadata from compact Relationship carriers.
//!
//! This only extracts the proof's carrier identity and opaque digests. The
//! proof does not authorize admission until file-level manifest validation.
use crate::relationship_compact_contract::RelationshipDescriptionProof;
use crate::{RelationshipError, RelationshipId};

const CREATION_FAMILY: &[u8; 6] = b"CVARNC";
const DELTA_FAMILY: &[u8; 6] = b"CVARND";

pub(crate) fn compact_description_proof(
    bytes: &[u8],
) -> Result<Option<(RelationshipId, u64, RelationshipDescriptionProof)>, RelationshipError> {
    if let Some(envelope) = crate::container::publication_envelope::decode(bytes)? {
        use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
        return Ok(
            match publication::decode(envelope.body, envelope.global_version)? {
                Some(RelationshipPublication::Creation(value)) => value
                    .description_proof
                    .map(|proof| (value.record.id, value.record.revision, proof)),
                Some(RelationshipPublication::Delta(value)) => value
                    .description_proof
                    .map(|proof| (value.id, value.revision, proof)),
                _ => None,
            },
        );
    }
    if bytes.starts_with(CREATION_FAMILY) {
        let creation = crate::relationship_compact_codec::decode_creation(bytes)?;
        return Ok(creation
            .description_proof
            .map(|proof| (creation.record.id, creation.record.revision, proof)));
    }
    if bytes.starts_with(DELTA_FAMILY) {
        let delta = crate::relationship_compact_codec::decode_delta(bytes)?;
        return Ok(delta
            .description_proof
            .map(|proof| (delta.id, delta.revision, proof)));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "relationship_description_proof_tests.rs"]
mod tests;
