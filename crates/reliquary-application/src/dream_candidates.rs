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

pub fn ranked_owner_dream_candidates(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    source_id: MemoryId,
    profile: CompatibilityProfileId,
    config: DreamCandidateConfig,
) -> Result<Vec<MemoryId>, DreamWorkflowError> {
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
    let mut cursor = None;
    let mut cut = None;
    loop {
        let (version, ids) = owner
            .memory_ids_page(cursor, 128)
            .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?;
        if cut.is_some_and(|previous| previous != version) {
            return Err(DreamWorkflowError::StaleSnapshot);
        }
        cut = Some(version);
        if ids.is_empty() {
            break;
        }
        cursor = ids.last().copied();
        for id in ids {
            if id == source_id
                || owner.dream_pair_evaluated(source_id, id)
                || already_related(owner, source_id, id)
            {
                continue;
            }
            let history = owner
                .memory_history(id)
                .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?
                .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
            if history.tombstone.is_some() {
                continue;
            }
            let revision = history
                .revisions
                .last()
                .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
            if revision.record.archived {
                continue;
            }
            let body = match &revision.payload {
                PayloadState::Available(body) => body,
                PayloadState::Absent => return Err(DreamWorkflowError::MemoryBodyUnavailable(id)),
            };
            let vector = owner
                .memory_vector_row(profile, revision.record.body_id)
                .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?;
            let semantic_score = vector
                .as_ref()
                .map(|vector| {
                    candidate_cosine(&source_vector, vector)
                        .map_err(|error| DreamWorkflowError::InvalidVector(error.into()))
                })
                .transpose()?;
            let (_, temporal_score) = reliquary_chronos::temporal_matches(
                &source_time,
                &temporal(&revision.record, body)?,
            );
            frontier.push(RankedCandidate {
                id,
                source_timestamp_ns: revision.record.source_time_ns,
                semantic_score,
                lexical_score: lexical_score(&source, &source_body, &revision.record, body),
                temporal_score,
                semantic_rank: None,
                prior_rank: None,
                lexical_rank: None,
                temporal_rank: None,
                fused_score: 0.0,
            });
        }
    }
    Ok(frontier
        .finish()
        .into_iter()
        .map(|candidate| candidate.id)
        .collect())
}

fn current_memory(
    owner: &EntityOwnerSession<'_>,
    id: MemoryId,
) -> Result<(MemoryRecord, MemoryBody), DreamWorkflowError> {
    let history = owner
        .memory_history(id)
        .map_err(|error| DreamWorkflowError::OwnerRead(error.to_string()))?
        .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
    if history.tombstone.is_some() {
        return Err(DreamWorkflowError::MemoryTombstoned(id));
    }
    let revision = history
        .revisions
        .last()
        .ok_or(DreamWorkflowError::MemoryUnavailable(id))?;
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
    if record.temporal_inference.is_some() {
        return Err(DreamWorkflowError::TemporalInferenceUnavailable);
    }
    Ok(reliquary_chronos::analyze(
        &format!("{}\n{}", body.title, body.content),
        record.source_time_ns,
    ))
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
