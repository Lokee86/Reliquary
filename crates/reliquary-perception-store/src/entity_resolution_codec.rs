#[cfg(test)]
use crate::MemoryEntityResolution;
use crate::{
    EntityId, EntityResolutionDormant, EntityResolutionPending, EntityResolutionReason,
    MAX_ENTITY_RESOLUTION_CANDIDATES, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
    MemoryTextField, ResolutionError,
};

const MAGIC: [u8; 8] = *b"CVAERS01";

#[path = "entity_resolution_delta_codec.rs"]
pub mod delta;
#[cfg(test)]
#[path = "entity_resolution_delta_tests.rs"]
mod delta_tests;
#[path = "entity_resolution_retained.rs"]
pub mod retained;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEntityResolution {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub updated_at_ns: i64,
    pub status: Option<MemoryEntityResolutionStatus>,
}

/// Established full-field comparison identity, never an ordinary storage write.
pub fn comparison_carrier(value: &DecodedEntityResolution) -> Result<Vec<u8>, ResolutionError> {
    encode_parts(
        value.key,
        value.revision,
        value.updated_at_ns,
        value.status.as_ref(),
    )
}

#[cfg(test)]
pub fn encode_resolution(value: &MemoryEntityResolution) -> Result<Vec<u8>, ResolutionError> {
    encode_parts(
        value.key,
        value.revision,
        value.updated_at_ns,
        Some(&value.status),
    )
}

#[cfg(test)]
pub fn encode_tombstone(
    key: MemoryEntityMentionKey,
    revision: u32,
    updated_at_ns: i64,
) -> Result<Vec<u8>, ResolutionError> {
    encode_parts(key, revision, updated_at_ns, None)
}

fn encode_parts(
    key: MemoryEntityMentionKey,
    revision: u32,
    updated_at_ns: i64,
    status: Option<&MemoryEntityResolutionStatus>,
) -> Result<Vec<u8>, ResolutionError> {
    if revision == 0 {
        return Err(ResolutionError::InvalidVersion);
    }
    let mut out = Vec::with_capacity(192);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&key.memory_id);
    out.push(key.field.tag());
    out.extend_from_slice(&key.start_byte.to_le_bytes());
    out.extend_from_slice(&key.end_byte.to_le_bytes());
    out.extend_from_slice(&revision.to_le_bytes());
    out.extend_from_slice(&updated_at_ns.to_le_bytes());
    match status {
        None => out.push(0),
        Some(MemoryEntityResolutionStatus::Resolved { entity_id, reason }) => {
            out.push(1);
            out.extend_from_slice(&entity_id.0);
            out.push(reason.tag());
        }
        Some(MemoryEntityResolutionStatus::Rejected { reason }) => {
            out.push(2);
            out.push(reason.tag());
        }
        Some(MemoryEntityResolutionStatus::Pending(value)) => {
            out.push(3);
            write_pending(&mut out, value)?;
        }
        Some(MemoryEntityResolutionStatus::Dormant(value)) => {
            out.push(4);
            write_dormant(&mut out, value)?;
        }
    }
    Ok(out)
}

fn write_pending(
    out: &mut Vec<u8>,
    value: &EntityResolutionPending,
) -> Result<(), ResolutionError> {
    write_candidates(out, &value.candidate_entity_ids)?;
    out.push(value.reason.tag());
    out.extend_from_slice(&value.candidate_set_fingerprint);
    out.extend_from_slice(&value.context_fingerprint);
    out.extend_from_slice(&value.first_seen_at_ns.to_le_bytes());
    out.extend_from_slice(&value.last_attempt_at_ns.to_le_bytes());
    out.extend_from_slice(&value.attempt_count.to_le_bytes());
    Ok(())
}

fn write_dormant(
    out: &mut Vec<u8>,
    value: &EntityResolutionDormant,
) -> Result<(), ResolutionError> {
    write_candidates(out, &value.candidate_entity_ids)?;
    out.push(value.reason.tag());
    out.extend_from_slice(&value.candidate_set_fingerprint);
    out.extend_from_slice(&value.context_fingerprint);
    out.extend_from_slice(&value.first_seen_at_ns.to_le_bytes());
    out.extend_from_slice(&value.last_attempt_at_ns.to_le_bytes());
    out.extend_from_slice(&value.attempt_count.to_le_bytes());
    out.extend_from_slice(&value.dormant_at_ns.to_le_bytes());
    Ok(())
}

fn write_candidates(out: &mut Vec<u8>, values: &[EntityId]) -> Result<(), ResolutionError> {
    if values.len() > MAX_ENTITY_RESOLUTION_CANDIDATES {
        return Err(ResolutionError::FieldTooLarge);
    }
    out.push(values.len() as u8);
    for value in values {
        out.extend_from_slice(&value.0);
    }
    Ok(())
}

fn read_candidates(bytes: &[u8], cursor: &mut usize) -> Result<Vec<EntityId>, ResolutionError> {
    let count = read_u8(bytes, cursor)? as usize;
    if count > MAX_ENTITY_RESOLUTION_CANDIDATES {
        return Err(ResolutionError::CorruptRecord(
            "too many Entity resolution candidates",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(EntityId(read_array(bytes, cursor)?));
    }
    Ok(values)
}

fn read_reason(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<EntityResolutionReason, ResolutionError> {
    EntityResolutionReason::from_tag(read_u8(bytes, cursor)?).ok_or(ResolutionError::CorruptRecord(
        "invalid Entity resolution reason",
    ))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], ResolutionError> {
    let end = cursor
        .checked_add(N)
        .ok_or(ResolutionError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(ResolutionError::CorruptRecord(
            "truncated Entity resolution record",
        ))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, ResolutionError> {
    Ok(read_array::<1>(bytes, cursor)?[0])
}
fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, ResolutionError> {
    Ok(u16::from_le_bytes(read_array(bytes, cursor)?))
}
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ResolutionError> {
    Ok(u32::from_le_bytes(read_array(bytes, cursor)?))
}
fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, ResolutionError> {
    Ok(i64::from_le_bytes(read_array(bytes, cursor)?))
}

#[cfg(test)]
#[path = "entity_resolution_retained_tests.rs"]
mod retained_tests;
