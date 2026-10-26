use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_owner_duplicate::publish_duplicate_pair;
use crate::graph_owner::GraphOwner;
use crate::memory_store::MemoryStore;
use crate::{
    DreamEvidenceSide, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamRelationDirection, DreamRelationKind, DreamVerificationPolicy,
    DreamVerificationVerdict, GraphRelationChange, GraphRelationKind, GraphRelationOrigin, Memory,
    MemoryId,
};
use reliquary_container::Container;
use std::collections::HashSet;

type RelationIdentity = (MemoryId, MemoryId, GraphRelationKind);

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
    validate_classification(classification)?;
    if let Some(outcome) = verification_gate(classification, verification, policy)? {
        return Ok(outcome);
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

    let desired = desired_relations(classification)?;
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

fn verification_gate(
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
) -> Result<Option<DreamPublicationOutcome>, DreamPublicationError> {
    if let Some(verification) = verification {
        if verification.a != classification.a
            || verification.b != classification.b
            || verification.classification != *classification
        {
            return Err(DreamPublicationError::VerificationMismatch);
        }
        return Ok(match verification.verdict {
            DreamVerificationVerdict::Accept => None,
            verdict => Some(DreamPublicationOutcome::Withheld(verdict)),
        });
    }
    if policy.should_verify(classification.relation) {
        Err(DreamPublicationError::MissingVerification)
    } else {
        Ok(None)
    }
}

fn validate_classification(
    classification: &DreamPairClassification,
) -> Result<(), DreamPublicationError> {
    if classification.a == classification.b {
        return Err(DreamPublicationError::InvalidClassification);
    }
    let evidence_valid = if classification.relation == DreamRelationKind::None {
        classification.evidence.is_empty()
    } else {
        classification.evidence.len() == 2
            && classification
                .evidence
                .iter()
                .filter(|item| item.side == DreamEvidenceSide::A)
                .count()
                == 1
            && classification
                .evidence
                .iter()
                .filter(|item| item.side == DreamEvidenceSide::B)
                .count()
                == 1
    };
    let valid = evidence_valid
        && match classification.relation {
            DreamRelationKind::None => classification.direction == DreamRelationDirection::None,
            DreamRelationKind::Topical
            | DreamRelationKind::Recurrent
            | DreamRelationKind::DuplicateOf => {
                classification.direction == DreamRelationDirection::Undirected
            }
            DreamRelationKind::Factual
            | DreamRelationKind::Causal
            | DreamRelationKind::Supersedes => matches!(
                classification.direction,
                DreamRelationDirection::AToB | DreamRelationDirection::BToA
            ),
        };
    valid
        .then_some(())
        .ok_or(DreamPublicationError::InvalidClassification)
}

fn desired_relations(
    classification: &DreamPairClassification,
) -> Result<HashSet<RelationIdentity>, DreamPublicationError> {
    let mut desired = HashSet::new();
    let kind = match classification.relation {
        DreamRelationKind::None => return Ok(desired),
        DreamRelationKind::Topical => GraphRelationKind::Topical,
        DreamRelationKind::Factual => GraphRelationKind::Factual,
        DreamRelationKind::Causal => GraphRelationKind::Causal,
        DreamRelationKind::Recurrent => GraphRelationKind::Recurrent,
        DreamRelationKind::Supersedes => GraphRelationKind::Supersedes,
        DreamRelationKind::DuplicateOf => return Err(DreamPublicationError::InvalidClassification),
    };
    match classification.direction {
        DreamRelationDirection::Undirected => {
            desired.insert((classification.a, classification.b, kind));
            desired.insert((classification.b, classification.a, kind));
        }
        DreamRelationDirection::AToB => {
            desired.insert((classification.a, classification.b, kind));
        }
        DreamRelationDirection::BToA => {
            desired.insert((classification.b, classification.a, kind));
        }
        DreamRelationDirection::None => return Err(DreamPublicationError::InvalidClassification),
    }
    Ok(desired)
}
