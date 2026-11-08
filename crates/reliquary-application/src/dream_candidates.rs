//! Rank owner-backed Dream candidates without mutating owner state.
use crate::{
    EntityOwnerSession,
    dream_workflow::{DreamWorkflowError, MAX_DREAM_CANDIDATES},
};
use reliquary_dream::{
    DreamCandidateConfig, DreamCandidateFrontier, RankedCandidate, candidate_cosine, lexical_score,
    validate_candidate_config,
};
use reliquary_owner_session::model::{
    CompatibilityProfileId, MemoryBody, MemoryId, MemoryRecord, NodeRef, PayloadState, RelationKind,
};

/// Select current owner-backed candidates and prepare the Dream proposal.
pub fn draft_ranked_dream_organization<
    C: reliquary_switchboard::GeneralEndpoint,
    V: reliquary_switchboard::GeneralEndpoint,
>(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    profile: CompatibilityProfileId,
    config: DreamCandidateConfig,
    classifier: &reliquary_dream::DreamClassifier<C>,
    verifier: Option<&reliquary_dream::DreamVerifier<V>>,
    verification_policy: reliquary_dream::DreamVerificationPolicy,
) -> Result<crate::DreamOrganizationDraft, DreamWorkflowError> {
    let candidates =
        ranked_owner_dream_candidates(owner, authorized_owner_id, source_id, profile, config)?;
    crate::draft_dream_organization(
        owner,
        authorized_owner_id,
        source_id,
        &candidates,
        classifier,
        verifier,
        verification_policy,
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DreamCandidateReadReport {
    pub index_entries_examined: usize,
    pub identities_examined: usize,
    pub body_reads: usize,
    pub vector_rows: usize,
}
pub fn ranked_owner_dream_candidates(
    owner: &EntityOwnerSession<'_>,
    authorized: [u8; 16],
    source: MemoryId,
    profile: CompatibilityProfileId,
    config: DreamCandidateConfig,
) -> Result<Vec<MemoryId>, DreamWorkflowError> {
    Ok(ranked_owner_dream_candidates_with_report(owner, authorized, source, profile, config)?.0)
}
pub fn ranked_owner_dream_candidates_with_report(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    profile: CompatibilityProfileId,
    config: DreamCandidateConfig,
) -> Result<(Vec<MemoryId>, DreamCandidateReadReport), DreamWorkflowError> {
    ranked_owner_dream_candidates_filtered(
        owner,
        authorized_owner_id,
        source_id,
        profile,
        config,
        |_| true,
    )
}

pub(crate) fn ranked_owner_dream_candidates_filtered(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    profile: CompatibilityProfileId,
    config: DreamCandidateConfig,
    eligible: impl Fn(MemoryId) -> bool,
) -> Result<(Vec<MemoryId>, DreamCandidateReadReport), DreamWorkflowError> {
    if owner.owner_id() != authorized_owner_id {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    if validate_candidate_config(config).is_err() || config.limit > MAX_DREAM_CANDIDATES {
        return Err(DreamWorkflowError::CandidateLimit);
    }
    let (source, source_body) = current_memory(owner, source_id)?;
    let source_vector = owner
        .memory_vector_row(profile, source.body_id)
        .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?
        .ok_or(DreamWorkflowError::MissingSourceVector)?;
    let source_time = temporal(&source, &source_body)?;
    let mut frontier = DreamCandidateFrontier::new(source.source_time_ns, config);
    candidate_cosine(&source_vector, &source_vector)
        .map_err(|e| DreamWorkflowError::InvalidVector(e.into()))?;
    let pool = owner
        .dream_candidate_pool(source_id, profile, &source_vector)
        .map_err(|e| DreamWorkflowError::OwnerRead(e.to_string()))?;
    let mut report = DreamCandidateReadReport {
        index_entries_examined: pool.index_entries_examined,
        identities_examined: pool.ids.len(),
        body_reads: 1,
        vector_rows: 1,
    };
    for id in pool.ids {
        if id == source_id
            || !eligible(id)
            || owner.dream_pair_evaluated(source_id, id)
            || already_related(owner, source_id, id)
        {
            continue;
        }
        let (record, body) = current_memory(owner, id)?;
        report.body_reads += 1;
        if record.archived {
            continue;
        }
        let vector = owner
            .memory_vector_row(profile, record.body_id)
            .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?;
        if vector.is_some() {
            report.vector_rows += 1;
        }
        let semantic_score = vector
            .as_ref()
            .map(|vector| {
                candidate_cosine(&source_vector, vector)
                    .map_err(|error| DreamWorkflowError::InvalidVector(error.into()))
            })
            .transpose()?;
        let (_, temporal_score) =
            reliquary_chronos::temporal_matches(&source_time, &temporal(&record, &body)?);
        frontier.push(RankedCandidate {
            id,
            source_timestamp_ns: record.source_time_ns,
            semantic_score,
            lexical_score: lexical_score(&source, &source_body, &record, &body),
            temporal_score,
            semantic_rank: None,
            prior_rank: None,
            lexical_rank: None,
            temporal_rank: None,
            fused_score: 0.0,
        });
    }
    Ok((
        frontier
            .finish()
            .into_iter()
            .map(|candidate| candidate.id)
            .collect(),
        report,
    ))
}

fn current_memory(
    owner: &EntityOwnerSession<'_>,
    id: MemoryId,
) -> Result<(MemoryRecord, MemoryBody), DreamWorkflowError> {
    let revision = owner.current_memory(id).map_err(|error| match error {
        reliquary_owner_session::OwnerSessionError::Memory(
            reliquary_owner_session::model::MemoryError::TerminalTombstone,
        ) => DreamWorkflowError::MemoryTombstoned(id),
        _ => DreamWorkflowError::MemoryUnavailable(id),
    })?;
    let body = match &revision.payload {
        PayloadState::Available(body) => body.clone(),
        PayloadState::Absent => return Err(DreamWorkflowError::MemoryBodyUnavailable(id)),
    };
    Ok((revision.record.clone(), body))
}

fn temporal(
    record: &MemoryRecord,
    body: &MemoryBody,
) -> Result<reliquary_chronos::TemporalAnalysis, DreamWorkflowError> {
    crate::dream_temporal::analyze(record, body)
}

fn already_related(owner: &EntityOwnerSession<'_>, source: MemoryId, other: MemoryId) -> bool {
    [
        RelationKind::Topical,
        RelationKind::Factual,
        RelationKind::Causal,
        RelationKind::Recurrent,
        RelationKind::DuplicateOf,
        RelationKind::Supersedes,
    ]
    .into_iter()
    .any(|kind| {
        [(source, other), (other, source)]
            .into_iter()
            .any(|(a, b)| {
                owner
                    .graph_relation(NodeRef::Memory(a.0), NodeRef::Memory(b.0), kind)
                    .is_some_and(|relation| relation.active)
            })
    })
}
