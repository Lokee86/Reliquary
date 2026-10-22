use crate::relationship_model::RelationshipRecord;
use crate::{
    EntityRef, MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES,
    MAX_RELATIONSHIP_OWNER_ID_BYTES, MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES,
    MAX_RELATIONSHIP_SUMMARY_BYTES, MemoryRef, RelationshipDraft, RelationshipError,
};

pub(crate) fn normalize_draft(draft: &mut RelationshipDraft) -> Result<(), RelationshipError> {
    draft.kind = draft.kind.trim().to_owned();
    draft.summary = draft.summary.trim().to_owned();
    draft.mutation_id = draft.mutation_id.trim().to_owned();
    for participant in &mut draft.participants {
        participant.entity.owner_id = participant.entity.owner_id.trim().to_owned();
        participant.role = participant.role.take().and_then(|role| {
            let role = role.trim().to_owned();
            (!role.is_empty()).then_some(role)
        });
    }
    for evidence in &mut draft.evidence {
        evidence.owner_id = evidence.owner_id.trim().to_owned();
    }
    draft.participants.sort();
    draft.participants.dedup();
    draft.evidence.sort_by(compare_memory_ref);
    draft.evidence.dedup();
    validate_fields(
        &draft.kind,
        &draft.participants,
        &draft.evidence,
        &draft.summary,
        &draft.mutation_id,
        draft.created_at_ns,
        draft.updated_at_ns,
    )
}

pub(crate) fn validate_record(record: &RelationshipRecord) -> Result<(), RelationshipError> {
    validate_fields(
        &record.kind,
        &record.participants,
        &record.evidence,
        &record.summary,
        &record.mutation_id,
        record.created_at_ns,
        record.updated_at_ns,
    )
}

fn validate_fields(
    kind: &str,
    participants: &[crate::RelationshipParticipant],
    evidence: &[MemoryRef],
    summary: &str,
    mutation_id: &str,
    created_at_ns: i64,
    updated_at_ns: i64,
) -> Result<(), RelationshipError> {
    if invalid_text(kind, MAX_RELATIONSHIP_KIND_BYTES, true) {
        return Err(RelationshipError::InvalidField("Relationship kind"));
    }
    if participants.is_empty()
        || participants.len() > MAX_RELATIONSHIP_PARTICIPANTS
        || participants.windows(2).any(|pair| pair[0] >= pair[1])
        || participants.iter().any(invalid_participant)
    {
        return Err(RelationshipError::InvalidField("Relationship participants"));
    }
    if evidence.len() > MAX_RELATIONSHIP_EVIDENCE
        || evidence
            .windows(2)
            .any(|pair| compare_memory_ref(&pair[0], &pair[1]).is_ge())
        || evidence.iter().any(|value| invalid_owner(&value.owner_id))
    {
        return Err(RelationshipError::InvalidField("Relationship evidence"));
    }
    if invalid_text(summary, MAX_RELATIONSHIP_SUMMARY_BYTES, false) {
        return Err(RelationshipError::InvalidField("Relationship summary"));
    }
    if mutation_id.is_empty() || mutation_id.chars().any(char::is_control) {
        return Err(RelationshipError::InvalidField("Relationship mutation ID"));
    }
    if updated_at_ns < created_at_ns {
        return Err(RelationshipError::InvalidField("Relationship timestamps"));
    }
    Ok(())
}

fn invalid_participant(value: &crate::RelationshipParticipant) -> bool {
    invalid_entity_ref(&value.entity)
        || value
            .role
            .as_deref()
            .is_some_and(|role| invalid_text(role, MAX_RELATIONSHIP_ROLE_BYTES, true))
}

fn invalid_entity_ref(value: &EntityRef) -> bool {
    invalid_owner(&value.owner_id)
}

fn invalid_owner(value: &str) -> bool {
    invalid_text(value, MAX_RELATIONSHIP_OWNER_ID_BYTES, true)
}

fn invalid_text(value: &str, max_bytes: usize, require_nonempty: bool) -> bool {
    (require_nonempty && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
}

fn compare_memory_ref(left: &MemoryRef, right: &MemoryRef) -> std::cmp::Ordering {
    left.owner_id
        .cmp(&right.owner_id)
        .then_with(|| left.memory_id.0.cmp(&right.memory_id.0))
}
