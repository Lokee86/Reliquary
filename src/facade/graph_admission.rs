use crate::{GraphError, GraphRelationOrigin, SemanticGraphRelationKind, SemanticNodeRef};
pub(crate) fn validate_relation_contract(
    source: SemanticNodeRef,
    target: SemanticNodeRef,
    kind: SemanticGraphRelationKind,
    origin: GraphRelationOrigin,
) -> Result<(), GraphError> {
    reliquary_graph::validate_relation_contract(source.into(), target.into(), kind, origin)
        .map_err(Into::into)
}
