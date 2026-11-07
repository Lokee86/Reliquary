//! Current retained Resolution records have their own source clock and typed evidence.
use super::*;

const REVISION: &[u8; 8] = b"RELERH01";
const INERT: &[u8; 8] = b"RELERL01";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedUnresolvedEvidence {
    pub candidate_entity_ids: Vec<EntityId>,
    pub reason: EntityResolutionReason,
    pub candidate_set_fingerprint: [u8; 32],
    pub context_fingerprint: [u8; 32],
    pub first_seen_at_ns: i64,
    pub last_attempt_at_ns: i64,
    pub attempt_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedResolutionDecision {
    Tombstoned,
    Resolved {
        entity_id: EntityId,
        reason: EntityResolutionReason,
    },
    Rejected {
        reason: EntityResolutionReason,
    },
    Pending(RetainedUnresolvedEvidence),
    Dormant {
        evidence: RetainedUnresolvedEvidence,
        dormant_at_ns: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedResolutionRevision {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub updated_at_ns: i64,
    pub decision: RetainedResolutionDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedResolutionValue {
    Revision(RetainedResolutionRevision),
    Inert(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedResolutionRecord {
    pub source_global_version: u64,
    pub source_ordinal: u64,
    pub value: RetainedResolutionValue,
}

#[derive(Default)]
pub(super) struct History {
    pub(super) records: Vec<RetainedResolutionRecord>,
    current: BTreeMap<MemoryEntityMentionKey, usize>,
}

impl History {
    pub(super) fn current(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Option<&RetainedResolutionRevision> {
        match &self.records[*self.current.get(&key)?].value {
            RetainedResolutionValue::Revision(value) => Some(value),
            RetainedResolutionValue::Inert(_) => unreachable!("only revisions are indexed"),
        }
    }

    pub(super) fn accept(
        &mut self,
        row: RetainedResolutionRecord,
        global_head: u64,
    ) -> Result<(), PerceptionError> {
        if row.source_global_version > global_head
            || self.records.last().is_some_and(|last| {
                row.source_global_version < last.source_global_version
                    || row.source_ordinal <= last.source_ordinal
            })
        {
            return Err(PerceptionError::CorruptRecord(
                "retained Resolution source order",
            ));
        }
        if let RetainedResolutionValue::Revision(value) = &row.value {
            let actual = self.current(value.key).map_or(0, |prior| prior.revision);
            let expected = actual
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?;
            if value.revision != expected {
                return Err(PerceptionError::RevisionConflict {
                    expected,
                    actual: value.revision,
                });
            }
            if value.decision == RetainedResolutionDecision::Tombstoned {
                self.current.remove(&value.key);
            } else {
                self.current.insert(value.key, self.records.len());
            }
        }
        self.records.push(row);
        Ok(())
    }
}

type DecodedResolution = ([u8; 16], RetainedResolutionRecord);

pub(super) fn decode(bytes: &[u8]) -> Result<Option<DecodedResolution>, PerceptionError> {
    let inert = match bytes.get(..8) {
        Some(magic) if magic == REVISION => false,
        Some(magic) if magic == INERT => true,
        _ => return Ok(None),
    };
    let mut at = 8;
    let owner = get_array(bytes, &mut at)?;
    let source_global_version = get_u64(bytes, &mut at)?;
    let source_ordinal = get_u64(bytes, &mut at)?;
    let value = if inert {
        let size = get_u32(bytes, &mut at)? as usize;
        let end = at
            .checked_add(size)
            .ok_or(PerceptionError::CorruptRecord("inert Resolution length"))?;
        let payload = bytes
            .get(at..end)
            .ok_or(PerceptionError::CorruptRecord("inert Resolution extent"))?;
        if !(8..=8 * 1024 * 1024).contains(&size)
            || !payload.starts_with(b"CVAERS02")
            || bytes.get(end..) != Some(Sha256::digest(&bytes[..end]).as_slice())
        {
            return Err(PerceptionError::CorruptRecord(
                "inert Resolution literal checksum",
            ));
        }
        at = bytes.len();
        RetainedResolutionValue::Inert(payload.to_vec())
    } else {
        let key = MemoryEntityMentionKey {
            memory_id: get_array(bytes, &mut at)?,
            field: MemoryTextField::from_tag(get_u8(bytes, &mut at)?)?,
            start_byte: get_u32(bytes, &mut at)?,
            end_byte: get_u32(bytes, &mut at)?,
        };
        validate_key(key)?;
        let revision = get_u32(bytes, &mut at)?;
        let updated_at_ns = get_i64(bytes, &mut at)?;
        let decision = match get_u8(bytes, &mut at)? {
            0 => RetainedResolutionDecision::Tombstoned,
            1 => RetainedResolutionDecision::Resolved {
                entity_id: EntityId(get_array(bytes, &mut at)?),
                reason: reason(bytes, &mut at)?,
            },
            2 => RetainedResolutionDecision::Rejected {
                reason: reason(bytes, &mut at)?,
            },
            3 => RetainedResolutionDecision::Pending(unresolved(bytes, &mut at)?),
            4 => {
                let evidence = unresolved(bytes, &mut at)?;
                let dormant_at_ns = get_i64(bytes, &mut at)?;
                if dormant_at_ns < evidence.last_attempt_at_ns {
                    return Err(PerceptionError::CorruptRecord(
                        "retained Dormant chronology",
                    ));
                }
                RetainedResolutionDecision::Dormant {
                    evidence,
                    dormant_at_ns,
                }
            }
            _ => {
                return Err(PerceptionError::CorruptRecord(
                    "unknown retained Resolution decision",
                ));
            }
        };
        RetainedResolutionValue::Revision(RetainedResolutionRevision {
            key,
            revision,
            updated_at_ns,
            decision,
        })
    };
    if at != bytes.len() {
        return Err(PerceptionError::CorruptRecord(
            "retained Resolution trailing bytes",
        ));
    }
    Ok(Some((
        owner,
        RetainedResolutionRecord {
            source_global_version,
            source_ordinal,
            value,
        },
    )))
}

fn reason(bytes: &[u8], at: &mut usize) -> Result<EntityResolutionReason, PerceptionError> {
    let tag = get_u8(bytes, at)?;
    if tag > 15 {
        return Err(PerceptionError::CorruptRecord(
            "unknown retained Resolution reason",
        ));
    }
    EntityResolutionReason::from_tag(tag)
}

fn unresolved(bytes: &[u8], at: &mut usize) -> Result<RetainedUnresolvedEvidence, PerceptionError> {
    let count = get_u8(bytes, at)? as usize;
    if count > MAX_MENTION_CANDIDATES {
        return Err(PerceptionError::CorruptRecord(
            "too many retained Resolution candidates",
        ));
    }
    let mut candidates = Vec::with_capacity(count);
    for _ in 0..count {
        candidates.push(EntityId(get_array(bytes, at)?));
    }
    if candidates.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PerceptionError::CorruptRecord(
            "unordered retained Resolution candidates",
        ));
    }
    let value = RetainedUnresolvedEvidence {
        candidate_entity_ids: candidates,
        reason: reason(bytes, at)?,
        candidate_set_fingerprint: get_array(bytes, at)?,
        context_fingerprint: get_array(bytes, at)?,
        first_seen_at_ns: get_i64(bytes, at)?,
        last_attempt_at_ns: get_i64(bytes, at)?,
        attempt_count: u16::from_le_bytes(get_array(bytes, at)?),
    };
    if value.attempt_count == 0 || value.last_attempt_at_ns < value.first_seen_at_ns {
        return Err(PerceptionError::CorruptRecord(
            "retained Resolution attempt chronology",
        ));
    }
    Ok(value)
}
