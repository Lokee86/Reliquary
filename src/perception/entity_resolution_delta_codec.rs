//! Exact unresolved-evidence deltas; keys, decisions and local revisions stay literal.
use super::*;
pub(crate) const MAGIC: &[u8; 8] = b"CVAERS03";
pub(crate) fn is_current(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

#[derive(Clone, Copy)]
struct Unresolved<'a> {
    candidates: &'a [EntityId],
    reason: EntityResolutionReason,
    candidate_fp: &'a [u8; 32],
    context_fp: &'a [u8; 32],
    first: i64,
    last: i64,
    attempt: u16,
    dormant: Option<i64>,
}
fn unresolved(status: &MemoryEntityResolutionStatus) -> Option<Unresolved<'_>> {
    match status {
        MemoryEntityResolutionStatus::Pending(v) => Some(Unresolved {
            candidates: &v.candidate_entity_ids,
            reason: v.reason,
            candidate_fp: &v.candidate_set_fingerprint,
            context_fp: &v.context_fingerprint,
            first: v.first_seen_at_ns,
            last: v.last_attempt_at_ns,
            attempt: v.attempt_count,
            dormant: None,
        }),
        MemoryEntityResolutionStatus::Dormant(v) => Some(Unresolved {
            candidates: &v.candidate_entity_ids,
            reason: v.reason,
            candidate_fp: &v.candidate_set_fingerprint,
            context_fp: &v.context_fingerprint,
            first: v.first_seen_at_ns,
            last: v.last_attempt_at_ns,
            attempt: v.attempt_count,
            dormant: Some(v.dormant_at_ns),
        }),
        _ => None,
    }
}

pub(crate) fn encode(
    value: &DecodedEntityResolution,
    previous: Option<&MemoryEntityResolutionStatus>,
) -> Result<Vec<u8>, MemoryError> {
    if value.revision == 0 {
        return Err(MemoryError::InvalidVersion);
    }
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&value.key.memory_id.0);
    bytes.push(value.key.field.tag());
    bytes.extend_from_slice(&value.key.start_byte.to_le_bytes());
    bytes.extend_from_slice(&value.key.end_byte.to_le_bytes());
    bytes.extend_from_slice(&value.revision.to_le_bytes());
    bytes.extend_from_slice(&value.updated_at_ns.to_le_bytes());
    match value.status.as_ref() {
        None => bytes.push(0),
        Some(MemoryEntityResolutionStatus::Resolved { entity_id, reason }) => {
            bytes.push(1);
            bytes.extend_from_slice(&entity_id.0);
            bytes.push(reason.tag());
        }
        Some(MemoryEntityResolutionStatus::Rejected { reason }) => {
            bytes.push(2);
            bytes.push(reason.tag());
        }
        Some(status) => {
            let next = unresolved(status).expect("remaining statuses are unresolved");
            let old = previous.and_then(unresolved);
            bytes.push(if next.dormant.is_some() { 4 } else { 3 });
            let mut mask = 0u8;
            macro_rules! changed {
                ($bit:literal,$field:ident) => {
                    if old.is_none_or(|old| next.$field != old.$field) {
                        mask |= 1 << $bit;
                    }
                };
            }
            changed!(0, candidates);
            changed!(1, reason);
            changed!(2, candidate_fp);
            changed!(3, context_fp);
            changed!(4, first);
            changed!(5, last);
            changed!(6, attempt);
            if next.dormant.is_some() && next.dormant != old.and_then(|old| old.dormant) {
                mask |= 128;
            }
            bytes.push(mask);
            if mask & 1 != 0 {
                write_candidates(&mut bytes, next.candidates)?;
            }
            if mask & 2 != 0 {
                bytes.push(next.reason.tag());
            }
            if mask & 4 != 0 {
                bytes.extend_from_slice(next.candidate_fp);
            }
            if mask & 8 != 0 {
                bytes.extend_from_slice(next.context_fp);
            }
            if mask & 16 != 0 {
                bytes.extend_from_slice(&next.first.to_le_bytes());
            }
            if mask & 32 != 0 {
                bytes.extend_from_slice(&next.last.to_le_bytes());
            }
            if mask & 64 != 0 {
                bytes.extend_from_slice(&next.attempt.to_le_bytes());
            }
            if mask & 128 != 0 {
                bytes.extend_from_slice(&next.dormant.expect("Dormant literal").to_le_bytes());
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn decode<'a>(
    bytes: &[u8],
    previous: impl FnOnce(MemoryEntityMentionKey) -> Option<&'a MemoryEntityResolutionStatus>,
) -> Result<Option<DecodedEntityResolution>, MemoryError> {
    if !is_current(bytes) {
        if bytes.starts_with(b"CVAERS") && !bytes.starts_with(b"CVAERS01") {
            return Err(corrupt("unknown or truncated Entity resolution version"));
        }
        return Ok(None);
    }
    let mut cursor = 8;
    let memory_id = MemoryId(read_array(bytes, &mut cursor)?);
    let field = MemoryTextField::from_tag(read_u8(bytes, &mut cursor)?)
        .ok_or(corrupt("invalid Entity resolution mention field"))?;
    let key = MemoryEntityMentionKey {
        memory_id,
        field,
        start_byte: read_u32(bytes, &mut cursor)?,
        end_byte: read_u32(bytes, &mut cursor)?,
    };
    let revision = read_u32(bytes, &mut cursor)?;
    if revision == 0 {
        return Err(corrupt("invalid Entity resolution revision"));
    }
    let updated_at_ns = read_i64(bytes, &mut cursor)?;
    let tag = read_u8(bytes, &mut cursor)?;
    let status = match tag {
        0 => None,
        1 => Some(MemoryEntityResolutionStatus::Resolved {
            entity_id: EntityId(read_array(bytes, &mut cursor)?),
            reason: read_reason(bytes, &mut cursor)?,
        }),
        2 => Some(MemoryEntityResolutionStatus::Rejected {
            reason: read_reason(bytes, &mut cursor)?,
        }),
        3 | 4 => {
            let mask = read_u8(bytes, &mut cursor)?;
            if tag == 3 && mask & 128 != 0 {
                return Err(corrupt("Pending resolution has Dormant field"));
            }
            let old = previous(key).and_then(unresolved);
            macro_rules! value {
                ($bit:literal,$literal:expr,$prior:expr) => {
                    if mask & (1 << $bit) != 0 {
                        $literal?
                    } else {
                        $prior.ok_or(corrupt(
                            "Entity resolution delta has no matching evidence base",
                        ))?
                    }
                };
            }
            let candidate_entity_ids = value!(
                0,
                read_candidates(bytes, &mut cursor),
                old.map(|v| v.candidates.to_vec())
            );
            let reason = value!(1, read_reason(bytes, &mut cursor), old.map(|v| v.reason));
            let candidate_set_fingerprint = value!(
                2,
                read_array(bytes, &mut cursor),
                old.map(|v| *v.candidate_fp)
            );
            let context_fingerprint = value!(
                3,
                read_array(bytes, &mut cursor),
                old.map(|v| *v.context_fp)
            );
            let first_seen_at_ns = value!(4, read_i64(bytes, &mut cursor), old.map(|v| v.first));
            let last_attempt_at_ns = value!(5, read_i64(bytes, &mut cursor), old.map(|v| v.last));
            let attempt_count = value!(6, read_u16(bytes, &mut cursor), old.map(|v| v.attempt));
            Some(if tag == 3 {
                MemoryEntityResolutionStatus::Pending(EntityResolutionPending {
                    candidate_entity_ids,
                    reason,
                    candidate_set_fingerprint,
                    context_fingerprint,
                    first_seen_at_ns,
                    last_attempt_at_ns,
                    attempt_count,
                })
            } else {
                let dormant_at_ns =
                    value!(7, read_i64(bytes, &mut cursor), old.and_then(|v| v.dormant));
                MemoryEntityResolutionStatus::Dormant(EntityResolutionDormant {
                    candidate_entity_ids,
                    reason,
                    candidate_set_fingerprint,
                    context_fingerprint,
                    first_seen_at_ns,
                    last_attempt_at_ns,
                    attempt_count,
                    dormant_at_ns,
                })
            })
        }
        _ => return Err(corrupt("invalid Entity resolution status")),
    };
    if cursor != bytes.len() {
        return Err(corrupt("Entity resolution trailing bytes"));
    }
    Ok(Some(DecodedEntityResolution {
        key,
        revision,
        updated_at_ns,
        status,
    }))
}
fn corrupt(message: &'static str) -> MemoryError {
    MemoryError::CorruptRecord(message)
}
