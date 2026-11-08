use super::support::{contains_normalized, owner_hex, source_memory};
use super::*;

pub(super) fn retire(
    common: &mut CommonState,
    container: &mut Container,
    request: EntityRetirementRequest,
) -> Result<EntityTombstone, OwnerSessionError> {
    if request.owner_id != container.owner_id() {
        return Err(OwnerSessionError::Record(
            "Entity retirement owner mismatch".into(),
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
    let entity = common.perception.entity(request.entity_id).ok_or_else(|| {
        OwnerSessionError::Record("Entity retirement target is not active".into())
    })?;
    if entity.revision != request.entity_revision {
        return Err(OwnerSessionError::ManagementStale);
    }
    if !(contains_normalized(&request.source_quote, &entity.canonical_name)
        || entity
            .aliases
            .iter()
            .any(|alias| contains_normalized(&request.source_quote, alias)))
    {
        return Err(OwnerSessionError::ManagementEvidence);
    }
    let active_mention = common
        .perception
        .mentions()
        .any(|mention| match &mention.status {
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                *entity_id == request.entity_id
            }
            MemoryEntityResolutionStatus::Ambiguous { candidates } => {
                candidates.contains(&request.entity_id)
            }
            MemoryEntityResolutionStatus::Unresolved { .. } => false,
        });
    let active_relationship = common.perception.relationships().any(|relationship| {
        relationship.participants.iter().any(|participant| {
            participant.entity.owner_id == owner_hex(&request.owner_id)
                && participant.entity.entity_id == request.entity_id
        })
    });
    let active_graph_relation = common.graph.active_relations().any(|relation| {
        [relation.source, relation.target]
            .into_iter()
            .any(|node| matches!(node, NodeRef::Entity(id) if id == request.entity_id.0))
    });
    if active_mention || active_relationship || active_graph_relation {
        return Err(OwnerSessionError::Record(
            "Entity retirement blocked by active references".into(),
        ));
    }
    Ok(common
        .perception
        .tombstone_entity(container, request.entity_id, request.entity_revision)?)
}
