use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_owner_duplicate::publish_duplicate_pair;
use crate::graph_owner::GraphOwner;
use crate::memory_store::MemoryStore;
use crate::{
    DreamPairClassification, DreamPairVerification, DreamPublicationError, DreamPublicationOutcome,
    DreamRelationKind, DreamVerificationPolicy, GraphRelationChange, GraphRelationOrigin, Memory,
};
use reliquary_container::Container;
use std::collections::HashSet;

use reliquary_dream::{
    PublicationPolicyError, desired_relations, validate_classification, verification_gate,
};

fn policy_error(error: PublicationPolicyError) -> DreamPublicationError {
    match error {
        PublicationPolicyError::InvalidClassification => {
            DreamPublicationError::InvalidClassification
        }
        PublicationPolicyError::VerificationMismatch => DreamPublicationError::VerificationMismatch,
        PublicationPolicyError::MissingVerification => DreamPublicationError::MissingVerification,
    }
}

pub(crate) fn publish_dream_pair<R>(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &mut GraphOwner,
    duplicate_index: &mut DuplicateIndex,
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
    expected_graph_version: u64,
    source_time: &R,
) -> Result<DreamPublicationOutcome, DreamPublicationError>
where
    R: Fn(&Memory) -> Option<i64>,
{
    validate_classification(classification).map_err(policy_error)?;
    if let Some(outcome) =
        verification_gate(classification, verification, policy).map_err(policy_error)?
    {
        return Ok(DreamPublicationOutcome::Withheld(outcome));
    }
    if classification.relation == DreamRelationKind::DuplicateOf {
        return publish_duplicate_pair(
            container,
            memories,
            graph,
            duplicate_index,
            classification.a,
            classification.b,
            expected_graph_version,
            source_time,
        );
    }
    let current_graph_version = graph.graph_version();
    if expected_graph_version != current_graph_version {
        return Err(crate::GraphError::RevisionConflict {
            expected: expected_graph_version,
            actual: current_graph_version,
        }
        .into());
    }

    let desired = desired_relations(classification).map_err(policy_error)?;
    let existing: HashSet<_> = graph
        .active_relations()
        .into_iter()
        .map(|relation| (relation.source, relation.target, relation.kind))
        .collect();

    let mut changes = Vec::new();
    for &(source, target, kind) in desired.difference(&existing) {
        let change = GraphRelationChange {
            source,
            target,
            kind,
            active: true,
        };
        if dream_may_change_relation(graph, change) {
            changes.push(change);
        }
    }
    if changes.is_empty() {
        return Ok(DreamPublicationOutcome::NoChange);
    }
    let published = graph.set_relations(container, memories, &changes, expected_graph_version)?;
    if published.is_empty() {
        Ok(DreamPublicationOutcome::NoChange)
    } else {
        Ok(DreamPublicationOutcome::Published(published))
    }
}

pub(crate) fn dream_may_change_relation(graph: &GraphOwner, change: GraphRelationChange) -> bool {
    !graph
        .relation_state(change.source, change.target, change.kind)
        .is_some_and(|state| {
            state.origin == GraphRelationOrigin::User && state.active != change.active
        })
}
