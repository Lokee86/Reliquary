use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_owner_duplicate::{
    DuplicateIndexSequence, DuplicateKeySequence, finish_duplicate_publication,
};
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{
    DreamEvidenceSide, DreamPairClassification, DreamPairVerification, DreamPublicationError,
    DreamPublicationOutcome, DreamRelationDirection, DreamRelationKind, DreamVerificationPolicy,
    DreamVerificationVerdict, GraphRelationChange, GraphRelationKind, GraphRelationOrigin,
    MemoryId,
};
use std::collections::HashSet;

#[path = "dream_owner_lifecycle.rs"]
mod lifecycle;
pub(crate) use lifecycle::reconcile_phy_lifecycle;

type RelationIdentity = (MemoryId, MemoryId, GraphRelationKind);

pub(crate) fn prepare_dream_pair(
    memories: &MemoryReadView<'_>,
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
    expected_graph_version: u64,
) -> Result<DreamPublicationPlan, DreamPublicationError> {
    validate_classification(classification)?;
    if let Some(outcome) = verification_gate(classification, verification, policy)? {
        return Ok(DreamPublicationPlan::Complete(outcome));
    }
    if classification.relation == DreamRelationKind::DuplicateOf {
        return Ok(DreamPublicationPlan::Duplicate(
            DuplicateKeySequence::start(
                graph,
                duplicate_index,
                classification.a,
                classification.b,
                expected_graph_version,
            )?,
        ));
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
        return Ok(DreamPublicationPlan::Complete(
            DreamPublicationOutcome::NoChange,
        ));
    }
    validate_dream_memory_changes(memories, &changes)?;
    Ok(DreamPublicationPlan::Changes {
        changes,
        duplicate: None,
    })
}

pub(crate) enum DreamPublicationPlan {
    Complete(DreamPublicationOutcome),
    Duplicate(DuplicateKeySequence),
    Changes {
        changes: Vec<GraphRelationChange>,
        duplicate: Option<DuplicatePublication>,
    },
}

pub(crate) use crate::dream_owner_duplicate::DuplicatePublication;

pub(crate) enum DreamPublicationFinish {
    Complete(DreamPublicationOutcome),
    Rebuild {
        sequence: DuplicateIndexSequence,
        outcome: DreamPublicationOutcome,
    },
}

pub(crate) fn finish_dream_pair(
    graph: &GraphStore,
    duplicate_index: &mut DuplicateIndex,
    duplicate: Option<DuplicatePublication>,
    published: Vec<crate::GraphRelation>,
) -> Result<DreamPublicationFinish, DreamPublicationError> {
    if let Some(duplicate) = duplicate {
        let (sequence, outcome) =
            finish_duplicate_publication(graph, duplicate_index, duplicate, published);
        Ok(match sequence {
            Some(sequence) => DreamPublicationFinish::Rebuild { sequence, outcome },
            None => DreamPublicationFinish::Complete(outcome),
        })
    } else if published.is_empty() {
        Ok(DreamPublicationFinish::Complete(
            DreamPublicationOutcome::NoChange,
        ))
    } else {
        Ok(DreamPublicationFinish::Complete(
            DreamPublicationOutcome::Published(published),
        ))
    }
}

pub(crate) fn dream_may_change_relation(graph: &GraphStore, change: GraphRelationChange) -> bool {
    !graph
        .relation_state(change.source, change.target, change.kind)
        .is_some_and(|state| {
            state.origin == GraphRelationOrigin::User && state.active != change.active
        })
}

pub(crate) fn validate_dream_memory_changes(
    memories: &MemoryReadView<'_>,
    changes: &[GraphRelationChange],
) -> Result<(), DreamPublicationError> {
    for change in changes {
        if change.source == change.target {
            return Err(crate::GraphError::SelfRelation.into());
        }
        for memory_id in [change.source, change.target] {
            if !memories.contains_memory(memory_id) {
                return Err(crate::GraphError::MissingMemory(memory_id).into());
            }
        }
    }
    Ok(())
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
