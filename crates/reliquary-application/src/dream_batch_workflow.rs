//! Complete bounded preparation and one-frame publication of a Dream frontier.
use crate::dream_workflow::{
    DreamMemoryObservation, PreparedDreamOrganization, prepare_dream_organization,
};
use crate::*;
use reliquary_dream::{
    DreamClassifier, DreamRelationKind, DreamVerificationPolicy, DreamVerificationVerdict,
    DreamVerifier, MemoryFact,
};
use reliquary_owner_session::model::{MemoryId, RelationKind};
use reliquary_owner_session::{
    DreamBatchCut, DreamLifecyclePublication, DreamMemoryCut, DreamPairPublication,
};
use reliquary_switchboard::GeneralEndpoint;
use std::collections::BTreeMap;
pub fn draft_dream_batch_organization<C: GeneralEndpoint, V: GeneralEndpoint>(
    owner: &EntityOwnerSession<'_>,
    authorized: [u8; 16],
    source: MemoryId,
    candidates: &[MemoryId],
    classifier: &DreamClassifier<C>,
    verifier: Option<&DreamVerifier<V>>,
    policy: DreamVerificationPolicy,
) -> Result<DreamOrganizationDraft, DreamWorkflowError> {
    prepare_dream_batch(owner, authorized, source, candidates)?
        .evaluate(classifier, verifier, policy)
}

pub(crate) struct PreparedDreamBatch {
    base: PreparedDreamOrganization,
    source: MemoryId,
    records: Vec<reliquary_owner_session::model::MemoryRecord>,
    duplicate_relations: Vec<reliquary_owner_session::model::Relation>,
}

pub(crate) fn prepare_dream_batch(
    owner: &EntityOwnerSession<'_>,
    authorized: [u8; 16],
    source: MemoryId,
    candidates: &[MemoryId],
) -> Result<PreparedDreamBatch, DreamWorkflowError> {
    let base = prepare_dream_organization(owner, authorized, source, candidates)?;
    let seeds: Vec<_> = std::iter::once(source)
        .chain(candidates.iter().copied())
        .collect();
    let (records, duplicate_relations) = owner
        .dream_duplicate_context(&seeds)
        .map_err(|e| DreamWorkflowError::OwnerRead(e.to_string()))?;
    Ok(PreparedDreamBatch {
        base,
        source,
        records,
        duplicate_relations,
    })
}

impl PreparedDreamBatch {
    pub(crate) fn evaluate<C: GeneralEndpoint, V: GeneralEndpoint>(
        self,
        classifier: &DreamClassifier<C>,
        verifier: Option<&DreamVerifier<V>>,
        policy: DreamVerificationPolicy,
    ) -> Result<DreamOrganizationDraft, DreamWorkflowError> {
        let Self {
            base,
            source,
            records,
            duplicate_relations,
        } = self;
        let mut draft = base.evaluate(classifier, verifier, policy)?;
        let mut relations: BTreeMap<_, _> = draft
            .relations
            .iter()
            .copied()
            .map(|r| ((r.source, r.target, r.kind), r))
            .collect();
        for r in duplicate_relations {
            relations.insert((r.source, r.target, r.kind), r);
        }
        let mut facts = BTreeMap::new();
        for r in records {
            if !draft.observed_memories.iter().any(|o| o.memory_id == r.id) {
                draft.observed_memories.push(DreamMemoryObservation {
                    memory_id: r.id,
                    revision: r.revision,
                    body_id: r.body_id,
                });
            }
            facts.insert(
                r.id,
                MemoryFact {
                    id: r.id,
                    lifecycle_state: r.lifecycle_state,
                    archived: r.archived,
                    superseded_by: r.superseded_by,
                    source_time_ns: r.source_time_ns,
                    authority_kind: r.authority_kind,
                    category: r.category,
                    memory_type: r.memory_type,
                },
            );
        }
        draft.facts = facts.into_values().collect();
        draft.relations = relations.into_values().collect();
        let mut blocked = Vec::new();
        draft.accepted_decisions.retain(|d| {
            let kind = match d.relation {
                reliquary_dream::PairRelation::None => return true,
                reliquary_dream::PairRelation::Topical => RelationKind::Topical,
                reliquary_dream::PairRelation::Factual => RelationKind::Factual,
                reliquary_dream::PairRelation::Causal => RelationKind::Causal,
                reliquary_dream::PairRelation::Recurrent => RelationKind::Recurrent,
                reliquary_dream::PairRelation::DuplicateOf => RelationKind::DuplicateOf,
                reliquary_dream::PairRelation::Supersedes => RelationKind::Supersedes,
            };
            let denied = draft.relations.iter().any(|r| {
                r.origin == reliquary_owner_session::model::RelationOrigin::User
                    && !r.active
                    && r.kind == kind
                    && ((r.source == reliquary_owner_session::model::NodeRef::Memory(d.a.0)
                        && r.target == reliquary_owner_session::model::NodeRef::Memory(d.b.0))
                        || (r.source == reliquary_owner_session::model::NodeRef::Memory(d.b.0)
                            && r.target == reliquary_owner_session::model::NodeRef::Memory(d.a.0)))
            });
            if denied {
                blocked.push(if d.a == source { d.b } else { d.a });
            }
            !denied
        });
        draft.skipped.extend(blocked);
        draft.plan = reliquary_dream::plan_organization(
            source,
            &draft.accepted_decisions,
            &draft.facts,
            &draft.relations,
        )
        .map_err(|e| DreamWorkflowError::OwnerRead(format!("Dream organization: {e:?}")))?;
        draft.batch_ready = true;
        Ok(draft)
    }
}

pub fn commit_dream_batch(
    writer: &mut DreamOwnerWriter<'_>,
    authorized: [u8; 16],
    publication: [u8; 32],
    draft: &DreamOrganizationDraft,
    updated_at_ns: i64,
) -> Result<bool, DreamWorkflowError> {
    if writer.owner_id() != authorized {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    let cut = prepare_dream_batch_cut(authorized, publication, draft, updated_at_ns)?;
    let result = match writer {
        DreamOwnerWriter::Rel(w) => w.commit_dream_batch(cut),
        DreamOwnerWriter::Phy(w) => w.commit_dream_batch(cut),
    };
    result.map_err(map_dream_owner_write)
}

pub(crate) fn prepare_dream_batch_cut(
    authorized: [u8; 16],
    publication: [u8; 32],
    draft: &DreamOrganizationDraft,
    updated_at_ns: i64,
) -> Result<DreamBatchCut, DreamWorkflowError> {
    if draft.owner_id != authorized {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    if !draft.pending.is_empty() {
        return Err(DreamWorkflowError::IncompleteFrontier);
    }
    if !draft.batch_ready
        || draft.classifications.len() > 16
        || (draft.classifications.is_empty()
            && draft.skipped.is_empty()
            && draft.plan.relation_changes.is_empty()
            && draft.plan.lifecycle_intents.is_empty())
    {
        return Err(DreamWorkflowError::UnsupportedPublication);
    }
    let source = draft
        .observed_memories
        .first()
        .ok_or(DreamWorkflowError::UnsupportedPublication)?
        .memory_id;
    let mut pairs = Vec::new();
    for c in &draft.classifications {
        let candidate = if c.a == source {
            c.b
        } else if c.b == source {
            c.a
        } else {
            return Err(DreamWorkflowError::UnsupportedPublication);
        };
        let rejected = draft.skipped.contains(&candidate)
            || draft
                .verifications
                .iter()
                .any(|v| v.classification == *c && v.verdict == DreamVerificationVerdict::Reject);
        let kind = if rejected {
            None
        } else {
            match c.relation {
                DreamRelationKind::None => None,
                DreamRelationKind::Topical => Some(RelationKind::Topical),
                DreamRelationKind::Factual => Some(RelationKind::Factual),
                DreamRelationKind::Causal => Some(RelationKind::Causal),
                DreamRelationKind::Recurrent => Some(RelationKind::Recurrent),
                DreamRelationKind::DuplicateOf => Some(RelationKind::DuplicateOf),
                DreamRelationKind::Supersedes => Some(RelationKind::Supersedes),
            }
        };
        pairs.push(DreamPairPublication {
            source,
            candidate,
            kind,
            rejected,
        });
    }
    for candidate in &draft.skipped {
        if pairs.iter().any(|p| p.candidate == *candidate) {
            continue;
        }
        pairs.push(DreamPairPublication {
            source,
            candidate: *candidate,
            kind: None,
            rejected: true,
        });
    }
    Ok(DreamBatchCut {
        owner: authorized,
        publication_id: publication,
        graph_version: draft.graph_version,
        observed: draft
            .observed_memories
            .iter()
            .map(|o| DreamMemoryCut {
                id: o.memory_id,
                revision: o.revision,
                body_id: o.body_id,
            })
            .collect(),
        pairs,
        changes: draft.plan.relation_changes.clone(),
        lifecycles: draft
            .plan
            .lifecycle_intents
            .iter()
            .map(|i| DreamLifecyclePublication {
                memory_id: i.memory_id,
                lifecycle_state: i.lifecycle_state.clone(),
                archived: i.archived,
                superseded_by: i.superseded_by,
            })
            .collect(),
        updated_at_ns,
    })
}

pub(crate) fn map_dream_owner_write(
    e: reliquary_owner_session::OwnerSessionError,
) -> DreamWorkflowError {
    match e {
        reliquary_owner_session::OwnerSessionError::StaleDreamCut => {
            DreamWorkflowError::StaleSnapshot
        }
        _ => DreamWorkflowError::OwnerWrite(e.to_string()),
    }
}

#[cfg(test)]
#[path = "dream_batch_workflow/tests.rs"]
mod tests;
