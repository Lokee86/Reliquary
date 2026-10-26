use crate::{
    GraphAdmission, GraphError, GraphRelationChange, GraphRelationOrigin,
    SemanticGraphRelationChange, SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
};

pub(super) fn validate_memory_change(
    admission: &GraphAdmission,
    change: GraphRelationChange,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    if change.source == change.target {
        return Err(GraphError::SelfRelation);
    }
    for id in [change.source, change.target] {
        if !admission.current_memories.contains(&id) {
            return Err(GraphError::MissingMemory(id));
        }
    }
    if origin == GraphRelationOrigin::Perception {
        return Err(GraphError::InvalidRelationOrigin);
    }
    Ok(())
}

pub(super) fn validate_change(
    admission: &GraphAdmission,
    change: SemanticGraphRelationChange,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    validate_relation_contract(change.source, change.target, change.kind, origin)?;
    validate_node(admission, change.source)?;
    validate_node(admission, change.target)?;
    if let Some(id) = change.target.as_entity() {
        let principal = admission.principal_entities.contains(&id);
        if (change.kind == SemanticGraphRelationKind::PrincipalAssociation) != principal {
            return Err(GraphError::InvalidRelationShape);
        }
    }
    Ok(())
}

pub(super) fn validate_stored_change(
    admission: &GraphAdmission,
    relation: crate::SemanticGraphRelation,
) -> Result<(), GraphError> {
    validate_relation_contract(
        relation.source,
        relation.target,
        relation.kind,
        relation.origin,
    )?;
    validate_stored_node(admission, relation.source)?;
    validate_stored_node(admission, relation.target)?;
    if let Some(id) = relation.target.as_entity() {
        let principal = admission.principal_entities.contains(&id);
        if (relation.kind == SemanticGraphRelationKind::PrincipalAssociation) != principal {
            return Err(GraphError::InvalidRelationShape);
        }
    }
    Ok(())
}

pub(super) fn validate_relation_contract(
    source: SemanticNodeRef,
    target: SemanticNodeRef,
    kind: SemanticGraphRelationKind,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    if source == target {
        return Err(GraphError::SelfRelation);
    }
    match kind {
        SemanticGraphRelationKind::Memory(_) => {
            if source.kind != SemanticNodeKind::Memory || target.kind != SemanticNodeKind::Memory {
                return Err(GraphError::InvalidRelationShape);
            }
            if origin == GraphRelationOrigin::Perception {
                return Err(GraphError::InvalidRelationOrigin);
            }
        }
        SemanticGraphRelationKind::EntityAssociation
        | SemanticGraphRelationKind::PrincipalAssociation => {
            if source.kind != SemanticNodeKind::Memory || target.kind != SemanticNodeKind::Entity {
                return Err(GraphError::InvalidRelationShape);
            }
            if origin == GraphRelationOrigin::Dream {
                return Err(GraphError::InvalidRelationOrigin);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_node(
    admission: &GraphAdmission,
    node: SemanticNodeRef,
) -> Result<(), GraphError> {
    match node.kind {
        SemanticNodeKind::Memory => {
            let id = node.as_memory().expect("Memory node decodes");
            if admission.current_memories.contains(&id) {
                Ok(())
            } else {
                Err(GraphError::MissingMemory(id))
            }
        }
        SemanticNodeKind::Entity => {
            let id = node.as_entity().expect("Entity node decodes");
            if admission.current_entities.contains(&id) {
                Ok(())
            } else {
                Err(GraphError::MissingEntity(id))
            }
        }
        SemanticNodeKind::Observation => Err(GraphError::UnsupportedSemanticNode(node)),
    }
}

pub(super) fn validate_stored_node(
    admission: &GraphAdmission,
    node: SemanticNodeRef,
) -> Result<(), GraphError> {
    match node.kind {
        SemanticNodeKind::Entity => {
            let id = node.as_entity().expect("Entity node decodes");
            if admission.historical_entities.contains(&id) {
                Ok(())
            } else {
                Err(GraphError::MissingEntity(id))
            }
        }
        SemanticNodeKind::Memory => {
            let id = node.as_memory().expect("Memory node decodes");
            if admission.historical_memories.contains(&id) {
                Ok(())
            } else {
                Err(GraphError::MissingMemory(id))
            }
        }
        _ => validate_node(admission, node),
    }
}
