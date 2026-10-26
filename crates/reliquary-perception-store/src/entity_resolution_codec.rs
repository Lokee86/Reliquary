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

pub fn decode_resolution(bytes: &[u8]) -> Result<Option<DecodedEntityResolution>, ResolutionError> {
    if bytes.len() < 8 || bytes[..8] != MAGIC {
        return Ok(None);
    }
    let mut cursor = 8;
    let memory_id = read_array(bytes, &mut cursor)?;
    let field = MemoryTextField::from_tag(read_u8(bytes, &mut cursor)?).ok_or(
        ResolutionError::CorruptRecord("invalid Entity resolution mention field"),
    )?;
    let key = MemoryEntityMentionKey {
        memory_id,
        field,
        start_byte: read_u32(bytes, &mut cursor)?,
        end_byte: read_u32(bytes, &mut cursor)?,
    };
    let revision = read_u32(bytes, &mut cursor)?;
    if revision == 0 {
        return Err(ResolutionError::CorruptRecord(
            "invalid Entity resolution revision",
        ));
    }
    let updated_at_ns = read_i64(bytes, &mut cursor)?;
    let status = match read_u8(bytes, &mut cursor)? {
        0 => None,
        1 => Some(MemoryEntityResolutionStatus::Resolved {
            entity_id: EntityId(read_array(bytes, &mut cursor)?),
            reason: read_reason(bytes, &mut cursor)?,
        }),
        2 => Some(MemoryEntityResolutionStatus::Rejected {
            reason: read_reason(bytes, &mut cursor)?,
        }),
        3 => Some(MemoryEntityResolutionStatus::Pending(read_pending(
            bytes,
            &mut cursor,
        )?)),
        4 => Some(MemoryEntityResolutionStatus::Dormant(read_dormant(
            bytes,
            &mut cursor,
        )?)),
        _ => {
            return Err(ResolutionError::CorruptRecord(
                "invalid Entity resolution status",
            ));
        }
    };
    if cursor != bytes.len() {
        return Err(ResolutionError::CorruptRecord(
            "Entity resolution trailing bytes",
        ));
    }
    Ok(Some(DecodedEntityResolution {
        key,
        revision,
        updated_at_ns,
        status,
    }))
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

fn read_pending(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<EntityResolutionPending, ResolutionError> {
    Ok(EntityResolutionPending {
        candidate_entity_ids: read_candidates(bytes, cursor)?,
        reason: read_reason(bytes, cursor)?,
        candidate_set_fingerprint: read_array(bytes, cursor)?,
        context_fingerprint: read_array(bytes, cursor)?,
        first_seen_at_ns: read_i64(bytes, cursor)?,
        last_attempt_at_ns: read_i64(bytes, cursor)?,
        attempt_count: read_u16(bytes, cursor)?,
    })
}

fn read_dormant(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<EntityResolutionDormant, ResolutionError> {
    let pending = read_pending(bytes, cursor)?;
    Ok(EntityResolutionDormant {
        candidate_entity_ids: pending.candidate_entity_ids,
        reason: pending.reason,
        candidate_set_fingerprint: pending.candidate_set_fingerprint,
        context_fingerprint: pending.context_fingerprint,
        first_seen_at_ns: pending.first_seen_at_ns,
        last_attempt_at_ns: pending.last_attempt_at_ns,
        attempt_count: pending.attempt_count,
        dormant_at_ns: read_i64(bytes, cursor)?,
    })
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
