//! Canonical retained-structure schema used by Entity prune descriptors.
//!
//! The R1/R2 discriminators below are immutable schema tags for the historical
//! descriptor hash input. This encoding is never persisted as a carrier and
//! intentionally has no dependency on the Entity disk codec.
use crate::EntityError;
use crate::entity_model::EntityRecord;

const RECORD_WITHOUT_GLOBAL_ID: &[u8; 8] = b"CVAENTR1";
const RECORD_WITH_GLOBAL_ID: &[u8; 8] = b"CVAENTR2";

pub fn encode_record_structure(record: &EntityRecord) -> Result<Vec<u8>, EntityError> {
    let mut out = Vec::with_capacity(160);
    match record.global_id {
        Some(global_id) => {
            out.extend_from_slice(RECORD_WITH_GLOBAL_ID);
            out.extend_from_slice(&record.id.0);
            out.extend_from_slice(&global_id.0);
        }
        None => {
            out.extend_from_slice(RECORD_WITHOUT_GLOBAL_ID);
            out.extend_from_slice(&record.id.0);
        }
    }
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, "[pruned]")?;
    out.extend_from_slice(&0_u16.to_le_bytes()); // pruned records retain no aliases
    write_string(&mut out, &record.kind)?;
    write_string(&mut out, "")?;
    write_string(&mut out, &record.mutation_id)?;
    Ok(out)
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), EntityError> {
    let length = u32::try_from(value.len()).map_err(|_| EntityError::FieldTooLarge)?;
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

#[cfg(test)]
#[path = "entity_retained_structure_tests.rs"]
mod tests;
