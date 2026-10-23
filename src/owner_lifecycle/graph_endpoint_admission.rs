use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{GraphError, GraphRelationChange, GraphRelationOrigin, SemanticGraphRelationChange};

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
        crate::owner_lifecycle::validate_current_graph_node(memories, entities, change.source)?;
        crate::owner_lifecycle::validate_current_graph_node(memories, entities, change.target)?;
        crate::owner_lifecycle::validate_principal_graph_kind(entities, *change)?;
    }
    Ok(())
}
