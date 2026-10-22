use crate::{
    GraphError, GraphRelationChange, GraphRelationOrigin, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
};

pub(super) fn validate_memory_change(
    change: GraphRelationChange,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    if change.source == change.target {
        return Err(GraphError::SelfRelation);
    }
    if origin == GraphRelationOrigin::Perception {
        return Err(GraphError::InvalidRelationOrigin);
    }
    Ok(())
}

pub(super) fn validate_change(
    change: SemanticGraphRelationChange,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    validate_relation_contract(change.source, change.target, change.kind, origin)
}

pub(super) fn validate_stored_change(
    relation: crate::SemanticGraphRelation,
) -> Result<(), GraphError> {
    validate_relation_contract(
        relation.source,
        relation.target,
        relation.kind,
        relation.origin,
    )?;
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
