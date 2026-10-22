use super::phylactery_error::OwnerPhylacteryError;
use crate::{
    GraphError, MemoryError, SemanticGraphRelation, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
};
use crate::{
    dream_pair_history::DreamPairStore, entity_store::EntityStore, graph_store::GraphStore,
    memory_store::MemoryStore, relationship_store::RelationshipStore,
};

/// Validates persisted Dream-pair references after replay without mutating either owner.
pub(crate) fn validate_dream_pairs(
    pairs: &DreamPairStore,
    memories: &MemoryStore,
) -> Result<(), MemoryError> {
    if pairs
        .referenced_memories()
        .all(|id| memories.contains_historical_memory(id))
    {
        Ok(())
    } else {
        Err(MemoryError::CorruptRecord(
            "Dream pair references a missing Memory",
        ))
    }
}

/// Validates current PHY/REL relationship references after replay.
pub(crate) fn validate_current_relationship_references(
    owner_id: Option<&str>,
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &[crate::Relationship],
) -> Result<(), crate::RelationshipError> {
    if relationships.is_empty() {
        return Ok(());
    }
    let owner_id = owner_id.ok_or(crate::RelationshipError::MissingOwnerIdentity)?;
    for relationship in relationships {
        validate_relationship_references(
            owner_id,
            memories,
            entities,
            &relationship.participants,
            &relationship.evidence,
            true,
        )?;
    }
    Ok(())
}

pub(crate) fn validate_relationship_entities(
    owner_id: &str,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    historical: bool,
) -> Result<(), crate::RelationshipError> {
    for participant in participants {
        if participant.entity.owner_id != owner_id {
            continue;
        }
        let exists = if historical {
            entities.contains_historical(participant.entity.entity_id)
        } else {
            entities.contains(participant.entity.entity_id)
        };
        if !exists {
            return Err(crate::RelationshipError::MissingLocalEntity(
                participant.entity.entity_id,
            ));
        }
    }
    Ok(())
}

/// Validate historical EntityResolution references after the store's local validation pass.
pub(crate) fn validate_entity_resolution_references(
    memories: &MemoryStore,
    entities: &EntityStore,
    resolutions: &[crate::MemoryEntityResolution],
) -> Result<(), MemoryError> {
    for resolution in resolutions {
        memories.validate_historical_entity_mention_key(resolution.key)?;
        crate::entity_resolution_store::validate_status(&resolution.status)?;
        validate_historical_entity_resolution_status(&resolution.status, entities)?;
    }
    Ok(())
}

fn validate_historical_entity_resolution_status(
    status: &crate::MemoryEntityResolutionStatus,
    entities: &EntityStore,
) -> Result<(), MemoryError> {
    let contains = |id| entities.contains_historical(id);
    let valid = match status {
        crate::MemoryEntityResolutionStatus::Resolved { entity_id, .. } => contains(*entity_id),
        crate::MemoryEntityResolutionStatus::Pending(value) => {
            value.candidate_entity_ids.iter().copied().all(contains)
        }
        crate::MemoryEntityResolutionStatus::Dormant(value) => {
            value.candidate_entity_ids.iter().copied().all(contains)
        }
        crate::MemoryEntityResolutionStatus::Rejected { .. } => true,
    };
    if valid {
        Ok(())
    } else {
        Err(MemoryError::InvalidField(
            "Entity resolution Entity reference",
        ))
    }
}

fn validate_relationship_references(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    participants: &[crate::RelationshipParticipant],
    evidence: &[crate::MemoryRef],
    historical: bool,
) -> Result<(), crate::RelationshipError> {
    validate_relationship_entities(owner_id, entities, participants, historical)?;
    for evidence in evidence {
        let exists = if historical {
            memories.contains_historical_memory(evidence.memory_id)
        } else {
            memories.contains_memory(evidence.memory_id)
        };
        if evidence.owner_id == owner_id && !exists {
            return Err(crate::RelationshipError::MissingLocalMemory(
                evidence.memory_id,
            ));
        }
    }
    Ok(())
}

/// Validates graph references against replayed owner records and rebuilds its current projection.
pub(crate) fn validate_rebuilt_graph(
    graph: &mut GraphStore,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<(), GraphError> {
    for node in graph.semantic_nodes() {
        validate_historical_graph_node(memories, entities, node)?;
    }
    for relation in graph.active_semantic_relations() {
        validate_historical_graph_relation(memories, entities, relation)?;
    }
    let excluded = graph
        .semantic_nodes()
        .into_iter()
        .filter(|node| validate_current_graph_node(memories, entities, *node).is_err())
        .collect::<Vec<_>>();
    graph.exclude_from_current_many(&excluded)
}

pub(crate) fn validate_current_graph_node(
    memories: &MemoryStore,
    entities: &EntityStore,
    node: SemanticNodeRef,
) -> Result<(), GraphError> {
    match node.kind {
        SemanticNodeKind::Memory => {
            let id = node.as_memory().expect("Memory node decodes");
            if memories.contains_memory(id) {
                Ok(())
            } else {
                Err(GraphError::MissingMemory(id))
            }
        }
        SemanticNodeKind::Entity => {
            let id = node.as_entity().expect("Entity node decodes");
            if entities.contains(id) {
                Ok(())
            } else {
                Err(GraphError::MissingEntity(id))
            }
        }
        SemanticNodeKind::Observation => Err(GraphError::UnsupportedSemanticNode(node)),
    }
}

fn validate_historical_graph_node(
    memories: &MemoryStore,
    entities: &EntityStore,
    node: SemanticNodeRef,
) -> Result<(), GraphError> {
    match node.kind {
        SemanticNodeKind::Memory => {
            let id = node.as_memory().expect("Memory node decodes");
            if memories.contains_historical_memory(id) {
                Ok(())
            } else {
                Err(GraphError::MissingMemory(id))
            }
        }
        SemanticNodeKind::Entity => {
            let id = node.as_entity().expect("Entity node decodes");
            if entities.contains_historical(id) {
                Ok(())
            } else {
                Err(GraphError::MissingEntity(id))
            }
        }
        SemanticNodeKind::Observation => validate_current_graph_node(memories, entities, node),
    }
}

fn validate_historical_graph_relation(
    memories: &MemoryStore,
    entities: &EntityStore,
    relation: SemanticGraphRelation,
) -> Result<(), GraphError> {
    let change = SemanticGraphRelationChange {
        source: relation.source,
        target: relation.target,
        kind: relation.kind,
        active: relation.active,
    };
    GraphStore::validate_semantic_change_shape(change, relation.origin)?;
    validate_historical_graph_node(memories, entities, relation.source)?;
    validate_historical_graph_node(memories, entities, relation.target)?;
    validate_principal_graph_kind(entities, change)
}

pub(crate) fn validate_principal_graph_kind(
    entities: &EntityStore,
    change: SemanticGraphRelationChange,
) -> Result<(), GraphError> {
    let Some(id) = change.target.as_entity() else {
        return Ok(());
    };
    let entity = entities
        .records()
        .iter()
        .rev()
        .find(|record| record.id == id)
        .ok_or(GraphError::MissingEntity(id))?;
    match change.kind {
        SemanticGraphRelationKind::EntityAssociation
            if entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND =>
        {
            Err(GraphError::InvalidRelationShape)
        }
        SemanticGraphRelationKind::PrincipalAssociation
            if entity.kind != crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND =>
        {
            Err(GraphError::InvalidRelationShape)
        }
        _ => Ok(()),
    }
}

// Kept here with the validated aggregate because these checks use only the
// replayed PHY owner constituents and preserve the existing open-time order.
pub(crate) fn validate_phylactery_global_versions(
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &crate::relationship_store::RelationshipStore,
    graph: &GraphStore,
) -> Result<(), OwnerPhylacteryError> {
    let mut versions: Vec<_> = memories
        .records()
        .iter()
        .map(|record| record.global_version)
        .chain(
            entities
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(
            relationships
                .records()
                .iter()
                .map(|record| record.global_version),
        )
        .chain(memories.tombstone_global_versions())
        .chain(entities.tombstone_global_versions())
        .chain(relationships.tombstone_global_versions())
        .chain(graph.transaction_global_versions().iter().copied())
        .collect();
    versions.sort_unstable();
    if let Some(version) = versions
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
    {
        return Err(OwnerPhylacteryError::SemanticGlobalVersionConflict(version));
    }
    Ok(())
}

pub(crate) fn validate_phylactery_provenance(memories: &MemoryStore) -> Result<(), MemoryError> {
    if memories.records().iter().any(|record| {
        record.source_episode_id.is_some()
            || record.source_node_id.is_some()
            || record.content_source_conversation_id.is_some()
            || record.content_source_node_id.is_some()
            || record.grounding_source_conversation_id.is_some()
            || record.grounding_source_node_id.is_some()
    }) {
        return Err(MemoryError::InvalidProvenance);
    }
    Ok(())
}

pub(crate) fn validate_phylactery_draft_provenance(
    draft: &crate::MemoryDraft,
) -> Result<(), MemoryError> {
    if draft.source_episode_id.is_some()
        || draft.source_node_id.is_some()
        || draft.content_source_conversation_id.is_some()
        || draft.content_source_node_id.is_some()
        || draft.grounding_source_conversation_id.is_some()
        || draft.grounding_source_node_id.is_some()
    {
        return Err(MemoryError::InvalidProvenance);
    }
    Ok(())
}
