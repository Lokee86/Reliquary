//! Current semantic-state closure and fresh-baseline emission for rewrites.
//!
//! Observation is not a separately persisted owner store in the current format.
//! Relationship evidence is its durable local-memory reference surface, so this
//! module closes those references before deciding which semantic records survive.

use super::RewriteError;
use crate::{
    Cva, Entity, EntityDraft, EntityId, GraphRelationOrigin, MemoryId, Phylactery, Relationship,
    RelationshipDraft, SemanticGraphRelation, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
};
use std::collections::BTreeSet;

pub(crate) use crate::maintenance::rewrite::closure::SemanticClosure;
use crate::maintenance::rewrite::closure::semantic_closure;

pub(crate) fn semantic_closure_rel(
    source: &Cva,
    retained: &BTreeSet<MemoryId>,
) -> Result<SemanticClosure, RewriteError> {
    semantic_closure(
        source.owner_id().as_deref(),
        retained,
        source.memories.current_ids().into_iter().collect(),
        source.entities(),
        source.relationships(),
        source.graph.active_semantic_relations(),
    )
}

pub(crate) fn semantic_closure_phy(
    source: &Phylactery,
    retained: &BTreeSet<MemoryId>,
) -> Result<SemanticClosure, RewriteError> {
    semantic_closure(
        source.owner_id().as_deref(),
        retained,
        source.memories.current_ids().into_iter().collect(),
        source.entities(),
        source.relationships(),
        source
            .graph
            .active_semantic_relations()
            .into_iter()
            .filter(|relation| relation.kind != SemanticGraphRelationKind::PrincipalAssociation)
            .collect(),
    )
}

pub(crate) fn copy_semantic_state_rel(
    source: &Cva,
    destination: &mut Cva,
    closure: &SemanticClosure,
    include_rel_ego_synthesis: bool,
) -> Result<(), RewriteError> {
    let owner_id = source.owner_id();
    let mut output = crate::maintenance::rewrite::semantic_emit::SemanticRewriteDestination {
        container: &mut destination.owner.container,
        memories: &destination.owner.memories,
        entities: &mut destination.owner.entities,
        relationships: &mut destination.owner.relationships,
        graph: &mut destination.owner.graph,
        ego: &mut destination.owner.ego,
    };
    crate::maintenance::rewrite::semantic_emit::copy_semantic_state_rel(
        owner_id.as_deref(),
        &source.ego,
        &mut output,
        closure,
        include_rel_ego_synthesis,
    )
}
pub(crate) fn copy_semantic_state_phy(
    source: &Phylactery,
    destination: &mut Phylactery,
    closure: &SemanticClosure,
) -> Result<(), RewriteError> {
    let owner_id = source.owner_id();
    let mut output = crate::maintenance::rewrite::semantic_emit::SemanticRewriteDestination {
        container: &mut destination.owner.container,
        memories: &destination.owner.memories,
        entities: &mut destination.owner.entities,
        relationships: &mut destination.owner.relationships,
        graph: &mut destination.owner.graph,
        ego: &mut destination.owner.ego,
    };
    crate::maintenance::rewrite::semantic_emit::copy_semantic_state_phy(
        owner_id.as_deref(),
        &source.ego,
        &mut output,
        closure,
    )
}

#[cfg(test)]
#[path = "semantic_state_tests.rs"]
mod tests;

#[cfg(test)]
fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
