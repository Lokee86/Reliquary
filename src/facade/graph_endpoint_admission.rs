use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{
    GraphError, GraphRelationChange, GraphRelationOrigin, SemanticGraphRelation,
    SemanticGraphRelationChange, SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
};

pub(crate) fn validate_memory_changes(
    memories: &MemoryStore,
    changes: &[GraphRelationChange],
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    for change in changes {
        if change.source == change.target {
            return Err(GraphError::SelfRelation);
        }
        for id in [change.source, change.target] {
            if !memories.contains_memory(id) {
                return Err(GraphError::MissingMemory(id));
            }
        }
        if origin == GraphRelationOrigin::Perception {
            return Err(GraphError::InvalidRelationOrigin);
        }
        GraphStore::validate_memory_change_shape(*change, origin)?;
    }
    Ok(())
}

pub(crate) fn validate_semantic_changes(
    memories: &MemoryStore,
    entities: &EntityStore,
    changes: &[SemanticGraphRelationChange],
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    for change in changes {
        GraphStore::validate_semantic_change_shape(*change, origin)?;
        validate_current_node(memories, entities, change.source)?;
        validate_current_node(memories, entities, change.target)?;
        validate_principal_kind(entities, *change)?;
    }
    Ok(())
}

pub(crate) fn validate_rebuilt_graph(
    graph: &mut GraphStore,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<(), GraphError> {
    for node in graph.semantic_nodes() {
        validate_historical_node(memories, entities, node)?;
    }
    for relation in graph.active_semantic_relations() {
        validate_historical_relation(memories, entities, relation)?;
    }
    let excluded = graph
        .semantic_nodes()
        .into_iter()
        .filter(|node| validate_current_node(memories, entities, *node).is_err())
        .collect::<Vec<_>>();
    graph.exclude_from_current_many(&excluded)
}

fn validate_current_node(
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

fn validate_historical_node(
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
        SemanticNodeKind::Observation => validate_current_node(memories, entities, node),
    }
}

fn validate_historical_relation(
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
    validate_historical_node(memories, entities, relation.source)?;
    validate_historical_node(memories, entities, relation.target)?;
    validate_principal_kind(entities, change)
}

fn validate_principal_kind(
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
