//! Read opaque prune-proof metadata from compact Entity carriers.
//!
//! This only extracts the proof's carrier identity and opaque digests. The
//! proof does not authorize admission until file-level manifest validation.
use crate::entity_compact_contract::EntityDescriptionProof;
use crate::{EntityError, EntityId};

const CREATION_FAMILY: &[u8; 6] = b"CVAENC";
const DELTA_FAMILY: &[u8; 6] = b"CVAEND";

pub(crate) fn compact_description_proof(
    bytes: &[u8],
) -> Result<Option<(EntityId, u64, EntityDescriptionProof)>, EntityError> {
    if let Some(envelope) = crate::container::publication_envelope::decode(bytes)? {
        use crate::entity_compact_codec::publication::{self, EntityPublication};
        return Ok(
            match publication::decode(envelope.body, envelope.global_version)? {
                Some(EntityPublication::Creation(creation)) => creation
                    .description_proof
                    .map(|proof| (creation.record.id, creation.record.revision, proof)),
                Some(EntityPublication::Delta(delta)) => delta
                    .description_proof
                    .map(|proof| (delta.id, delta.revision, proof)),
                _ => None,
            },
        );
    }
    if bytes.starts_with(CREATION_FAMILY) {
        let creation = crate::entity_compact_codec::decode_creation(bytes)?;
        return Ok(creation
            .description_proof
            .map(|proof| (creation.record.id, creation.record.revision, proof)));
    }
    if bytes.starts_with(DELTA_FAMILY) {
        let delta = crate::entity_compact_codec::decode_delta(bytes)?;
        return Ok(delta
            .description_proof
            .map(|proof| (delta.id, delta.revision, proof)));
    }
    Ok(None)
}

#[cfg(test)]
#[path = "entity_description_proof_tests.rs"]
mod tests;
