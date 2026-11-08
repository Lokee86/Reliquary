use super::support::{contains_normalized, owner_hex, source_memory};
use super::*;

pub(super) fn create(
    common: &mut CommonState,
    container: &mut Container,
    request: CreateRelationshipRequest,
) -> Result<Relationship, OwnerSessionError> {
    if request.owner_id != container.owner_id() {
        return Err(OwnerSessionError::Record(
            "Relationship owner mismatch".into(),
        ));
    }
    if request.kind.trim().is_empty()
        || request.mutation_id.trim().is_empty()
        || request.participants.is_empty()
    {
        return Err(OwnerSessionError::Record(
            "Relationship request is incomplete".into(),
        ));
    }
    let memory = source_memory(
        common,
        request.source_memory_id,
        request.source_memory_revision,
        request.source_body_id,
        request.source_field,
        request.start_byte,
        request.end_byte,
        &request.source_quote,
    )?;
    let mut participants = Vec::with_capacity(request.participants.len());
    for input in request.participants {
        let entity = common
            .perception
            .entity(input.entity_id)
            .filter(|entity| entity.revision == input.entity_revision)
            .ok_or(OwnerSessionError::ManagementStale)?;
        if !contains_normalized(&request.source_quote, &entity.canonical_name) {
            return Err(OwnerSessionError::ManagementEvidence);
        }
        participants.push(RelationshipParticipant {
            entity: RelationshipEntityRef {
                owner_id: owner_hex(&request.owner_id),
                entity_id: entity.id,
            },
            role: input.role,
        });
    }
    participants.sort_by(|a, b| {
        (&a.entity.owner_id, a.entity.entity_id, &a.role).cmp(&(
            &b.entity.owner_id,
            b.entity.entity_id,
            &b.role,
        ))
    });
    if participants.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(OwnerSessionError::Record(
            "duplicate Relationship participant".into(),
        ));
    }
    let memory = memory.record;
    common
        .perception
        .publish_relationship(
            container,
            RelationshipDraft {
                kind: request.kind,
                participants,
                evidence: vec![RelationshipMemoryRef {
                    owner_id: owner_hex(&request.owner_id),
                    memory_id: request.source_memory_id.0,
                }],
                summary: request.source_quote,
                mutation_id: request.mutation_id,
                created_at_ns: memory.created_at_ns,
                updated_at_ns: request.updated_at_ns.max(memory.updated_at_ns),
            },
        )
        .map_err(|error| match error {
            PerceptionError::InvalidField("conflicting Relationship mutation") => {
                OwnerSessionError::ManagementConflict
            }
            other => other.into(),
        })
}

pub(super) fn retire(
    common: &mut CommonState,
    container: &mut Container,
    request: RelationshipRetirementRequest,
) -> Result<RelationshipTombstone, OwnerSessionError> {
    if request.owner_id != container.owner_id() {
        return Err(OwnerSessionError::Record(
            "Relationship retirement owner mismatch".into(),
        ));
    }
    source_memory(
        common,
        request.source_memory_id,
        request.source_memory_revision,
        request.source_body_id,
        request.source_field,
        request.start_byte,
        request.end_byte,
        &request.source_quote,
    )?;
    let relationship = common
        .perception
        .relationship(request.relationship_id)
        .ok_or_else(|| {
            OwnerSessionError::Record("Relationship retirement target is not active".into())
        })?;
    if relationship.revision != request.relationship_revision {
        return Err(OwnerSessionError::ManagementStale);
    }
    if relationship.summary != request.source_quote
        || !relationship.evidence.iter().any(|evidence| {
            evidence.owner_id == owner_hex(&request.owner_id)
                && evidence.memory_id == request.source_memory_id.0
        })
    {
        return Err(OwnerSessionError::ManagementEvidence);
    }
    Ok(common.perception.tombstone_relationship(
        container,
        request.relationship_id,
        request.relationship_revision,
    )?)
}
