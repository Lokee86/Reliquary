use super::support::owner_hex;
use super::*;
use crate::model::RelationKind;

pub(super) fn retire(
    common: &mut CommonState,
    container: &mut Container,
    request: MemoryRetirementRequest,
) -> Result<TerminalTombstone, OwnerSessionError> {
    validate_retirement(common, container.owner_id(), &request)?;
    Ok(common
        .memories
        .tombstone(container, request.memory_id, request.memory_revision)?)
}

pub(in crate::session) fn validate_retirement(
    common: &CommonState,
    owner: [u8; 16],
    request: &MemoryRetirementRequest,
) -> Result<(), OwnerSessionError> {
    if request.owner_id != owner {
        return Err(OwnerSessionError::Record(
            "Memory retirement owner mismatch".into(),
        ));
    }
    let target = common.memories.current(request.memory_id)?;
    if target.record.revision != request.memory_revision || target.record.body_id != request.body_id
    {
        return Err(OwnerSessionError::ManagementStale);
    }
    if !target.record.archived
        || target.record.authority_kind != "direct"
        || target.record.source_ref.as_ref().is_none_or(|source| {
            source.owner_id.trim().is_empty() || source.source_node_id.trim().is_empty()
        })
        || target.record.superseded_by != Some(request.superseded_by)
        || !matches!(target.payload, PayloadState::Available(_))
    {
        return Err(OwnerSessionError::Record(
            "Memory retirement requires an available, accepted archived supersession".into(),
        ));
    }
    let successor = common.memories.current(request.superseded_by)?;
    if successor.record.revision != request.successor_revision
        || successor.record.body_id != request.successor_body_id
    {
        return Err(OwnerSessionError::ManagementStale);
    }
    if successor.record.archived || successor.record.superseded_by.is_some() {
        return Err(OwnerSessionError::Record(
            "Memory retirement successor is unavailable or changed".into(),
        ));
    }
    // Dream supersession records its edge from the surviving successor to the retired source.
    let expected_edge = |relation: &Relation| {
        relation.kind == RelationKind::Supersedes
            && relation.source == NodeRef::Memory(request.superseded_by.0)
            && relation.target == NodeRef::Memory(request.memory_id.0)
    };
    if common
        .perception
        .mentions_for_memory(request.memory_id.0)
        .any(|mention| {
            !matches!(
                mention.status,
                MemoryEntityResolutionStatus::Unresolved { .. }
            )
        })
        || common.perception.relationships().any(|relationship| {
            relationship.evidence.iter().any(|evidence| {
                evidence.owner_id == owner_hex(&request.owner_id)
                    && evidence.memory_id == request.memory_id.0
            })
        })
        || common.graph.active_relations().any(|relation| {
            [relation.source, relation.target]
                .into_iter()
                .any(|node| matches!(node, NodeRef::Memory(id) if id == request.memory_id.0))
                && !expected_edge(relation)
        })
    {
        return Err(OwnerSessionError::Record(
            "Memory retirement blocked by active references".into(),
        ));
    }
    Ok(())
}
