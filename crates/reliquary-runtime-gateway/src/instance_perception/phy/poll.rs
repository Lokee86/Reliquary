use super::{
    PhyWork,
    progress::{Page, Progress},
};
use crate::{GatewayError, RelSession};
use reliquary_application::{
    observation_drafts::{
        ObservationRead, ObservationWorkflowError, PreparedObservation, ProposalEvaluation,
        ScopedObservationRead, evaluate_proposal, evaluate_review,
    },
    perception_workflow::{
        BatchLimits, ContributionEmbeddingCache, ContributionFailure, ContributionWork,
        Eligibility, ListResult, LookupError, LookupRequest, PerceptionRoutes, audit_owner_cut,
        capture_phy_contribution, capture_phy_duplicate_contribution,
        capture_phy_perception_discovery, discover_phy_for_features, evaluate_contribution,
        phy_audit_lookups,
    },
};
use reliquary_owner_session::{
    OwnerSessionError, PerceptionSourceCut, PhyObservationBatch,
    observation::{
        ObservationAuditProgress, ObservationBatch, ObservationId, ObservationOperation,
        ObservationPendingReason, ObservationPolicy, ObservationStanding,
    },
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use super::super::poll::{claim_vectors::ClaimVectorIndex, draft_error, draft_routes};

const REVIEW_SCAN_LIMIT: usize = 16;

#[derive(Default)]
pub(super) struct State {
    review_cut: Option<PerceptionSourceCut>,
    review_after: Option<ObservationId>,
    review_exhausted: bool,
    contribution_vectors: ContributionEmbeddingCache,
    malformed_contribution: Option<[u8; 32]>,
    malformed_creation: Option<([u8; 32], u32)>,
    malformed_reviews: BTreeSet<ObservationId>,
    claim_vectors: ClaimVectorIndex,
}

impl ScopedObservationRead for PhyWork<'_> {
    fn read<T>(
        &self,
        _: impl FnOnce(&RelSession) -> Result<T, ObservationWorkflowError>,
    ) -> Result<T, ObservationWorkflowError> {
        Err(ObservationWorkflowError::Owner(
            OwnerSessionError::ManagementStale,
        ))
    }

    fn read_owner<T>(
        &self,
        capture: impl FnOnce(ObservationRead<'_>) -> Result<T, ObservationWorkflowError>,
    ) -> Result<T, ObservationWorkflowError> {
        PhyWork::read(self, |writer| capture(ObservationRead::Phy(writer)))
            .map_err(|_| ObservationWorkflowError::Owner(OwnerSessionError::ManagementStale))?
    }
}

pub(super) fn tick(owner: &PhyWork<'_>, state: &mut State) -> Result<bool, GatewayError> {
    let progressed = tick_work(owner, state)?;
    let indexed = state.claim_vectors.tick_phy(owner, &owner.fenced_routes());
    Ok(progressed || indexed)
}

fn tick_work(owner: &PhyWork<'_>, state: &mut State) -> Result<bool, GatewayError> {
    let (previous, mut progress) = owner.read(|phy| {
        if let Some(row) = phy.observation_progress() {
            Ok((
                Some(row.fingerprint),
                Progress::decode(&row.bytes, phy.phy_observation_cut())?,
            ))
        } else {
            Ok::<_, OwnerSessionError>((None, Progress::new(phy)))
        }
    })??;
    if previous.is_none() {
        publish(owner, None, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    if run_review(owner, state, previous)? {
        return Ok(true);
    }
    let Some(page) = progress.page.take() else {
        if owner.read(|writer| progress.schedule(writer))?? {
            publish(owner, previous, Some(progress), [], Vec::new())?;
            return Ok(true);
        }
        return Ok(false);
    };
    if page.kind.discovery() {
        discovery(owner, state, previous, progress, page)
    } else {
        contribution(owner, state, previous, progress, page)
    }
}

fn run_review(
    owner: &PhyWork<'_>,
    state: &mut State,
    previous: Option<[u8; 32]>,
) -> Result<bool, GatewayError> {
    let Some(target) = owner.read(|writer| {
        let cut = writer.perception_source_cut();
        if state.review_cut.as_ref() != Some(&cut) {
            state.review_cut = Some(cut);
            state.review_after = None;
            state.review_exhausted = false;
            state.malformed_reviews.clear();
        }
        if state.review_exhausted {
            return None;
        }
        let page = writer.observations_after(state.review_after, REVIEW_SCAN_LIMIT);
        for head in &page {
            let id = head.revision.id;
            if !state.malformed_reviews.contains(&id)
                && matches!(
                    head.standing,
                    ObservationStanding::Pending(
                        ObservationPendingReason::NewEvidence
                            | ObservationPendingReason::ChangedSupport
                    )
                )
            {
                return Some(Some(id));
            }
            state.review_after = Some(id);
        }
        state.review_exhausted = page.len() < REVIEW_SCAN_LIMIT;
        (!state.review_exhausted).then_some(None)
    })?
    else {
        return Ok(false);
    };
    let Some(id) = target else { return Ok(true) };
    let routes = owner.fenced_routes();
    let result = evaluate_review(owner, &draft_routes(&routes, None), id);
    match result {
        Ok(Some(prepared)) => publish(owner, previous, None, [prepared], Vec::new())?,
        Ok(None) => {}
        Err(error) if error.malformed_native_reply() => {
            state.malformed_reviews.insert(id);
        }
        Err(error) => return Err(draft_error(error)),
    }
    state.review_after = Some(id);
    Ok(true)
}

fn contribution(
    owner: &PhyWork<'_>,
    state: &mut State,
    previous: Option<[u8; 32]>,
    mut progress: Progress,
    mut page: Page,
) -> Result<bool, GatewayError> {
    if usize::from(page.contribution_cursor) >= page.sources.len() {
        publish(owner, previous, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    let routes = owner.fenced_routes();
    let profile = state.claim_vectors.phy_memory_profile(owner, &routes)?;
    let work = owner.read(|phy| {
        capture_phy_contribution(
            phy,
            page.cut,
            &page.sources,
            page.contribution_cursor,
            page.target_after,
            profile,
        )
    })??;
    let Some(operations) = evaluate(&routes, state, &work)? else {
        return Ok(false);
    };
    page.contribution_cursor = work.next_source;
    page.target_after = work.next_target;
    progress.page = Some(page);
    publish(owner, previous, Some(progress), [], operations)?;
    Ok(true)
}

fn discovery(
    owner: &PhyWork<'_>,
    state: &mut State,
    previous: Option<[u8; 32]>,
    mut progress: Progress,
    mut page: Page,
) -> Result<bool, GatewayError> {
    if usize::from(page.cursor.source) >= page.sources.len() + page.derived.len() {
        publish(owner, previous, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    let routes = owner.fenced_routes();
    let capture = owner.read(|phy| {
        capture_phy_perception_discovery(
            phy,
            page.cut,
            &page.sources,
            &page.derived,
            page.cursor,
            routes.identity,
            BatchLimits::default(),
            Eligibility::PERCEPTION,
        )
    })??;
    if page.capture != Some(capture.key) {
        page.batch = 0;
        page.proposals = None;
        page.capture = Some(capture.key);
    }
    if page.batch as usize >= capture.batch_count() {
        if capture.next <= page.cursor {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        page.cursor = capture.next;
        page.batch = 0;
        page.capture = None;
        progress.page = Some(page);
        publish(owner, previous, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    let key = (capture.key, page.batch);
    if state.malformed_creation == Some(key) {
        return Ok(false);
    }
    if let Some(proposals) = page.proposals.take() {
        let (drafts, operations) = evaluate_proposals(owner, state, &routes, &page, proposals)?;
        page.batch += 1;
        progress.page = Some(page);
        publish(owner, previous, Some(progress), drafts, operations)?;
        return Ok(true);
    }
    let mut lookup = |_: _, request: LookupRequest<'_>| {
        owner
            .read(|phy| phy_audit_lookups(phy, page.cut.audit, request))
            .map_err(|_| LookupError::Invalidated)?
            .map_err(|_| LookupError::Unavailable)
    };
    let result = discover_phy_for_features(
        &capture,
        page.batch as usize,
        Eligibility::PERCEPTION,
        routes.general.as_ref(),
        &mut lookup,
    )
    .map_err(|_| unavailable("PHY Perception discovery remains pending"))?;
    match result.perception {
        ListResult::Accepted { candidates, .. } => page.proposals = Some(candidates),
        ListResult::Inactive => page.batch += 1,
        ListResult::Incomplete(_) => {
            return Err(unavailable("PHY Perception discovery list remains pending"));
        }
    }
    progress.page = Some(page);
    publish(owner, previous, Some(progress), [], Vec::new())?;
    Ok(true)
}

fn evaluate_proposals(
    owner: &PhyWork<'_>,
    state: &mut State,
    routes: &PerceptionRoutes,
    page: &Page,
    proposals: Vec<reliquary_application::perception_workflow::ObservationProposal>,
) -> Result<(Vec<PreparedObservation>, Vec<ObservationOperation>), GatewayError> {
    let profile = state.claim_vectors.phy_memory_profile(owner, routes)?;
    let draft_routes = draft_routes(routes, profile);
    let mut drafts = Vec::new();
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeMap::<_, BTreeSet<_>>::new();
    let cut = audit_owner_cut(&page.cut.into());
    for proposal in proposals {
        match evaluate_proposal(owner, &draft_routes, cut, &proposal) {
            Ok(ProposalEvaluation::Ready(draft)) => {
                let identity = match &draft.operation {
                    ObservationOperation::Create { draft } => draft.claim_digest(),
                    _ => draft.input_fingerprint,
                };
                if seen.insert(identity) {
                    drafts.push(draft);
                }
            }
            Ok(ProposalEvaluation::Duplicate { target, evidence }) => {
                duplicates
                    .entry((ObservationId(target.id), target.revision))
                    .or_default()
                    .extend(evidence);
            }
            Ok(ProposalEvaluation::Abstained | ProposalEvaluation::Unresolved)
            | Err(
                ObservationWorkflowError::InvalidDraft | ObservationWorkflowError::EvidenceLimit,
            ) => {}
            Err(error) => {
                if error.malformed_native_reply() {
                    state.malformed_creation = page.capture.map(|key| (key, page.batch));
                }
                return Err(draft_error(error));
            }
        }
    }
    let mut operations = Vec::new();
    for ((id, revision), evidence) in duplicates {
        let selected = evidence.into_iter().collect::<Vec<_>>();
        let work = owner
            .read(|phy| capture_phy_duplicate_contribution(phy, id, revision, &selected))??;
        let Some(decisions) = evaluate(routes, state, &work)? else {
            return Err(unavailable(
                "PHY Perception contribution waits for changed evidence",
            ));
        };
        operations.extend(decisions);
    }
    Ok((drafts, operations))
}

fn evaluate(
    routes: &PerceptionRoutes,
    state: &mut State,
    work: &ContributionWork,
) -> Result<Option<Vec<ObservationOperation>>, GatewayError> {
    let key = work.decision_key();
    if key.is_some() && state.malformed_contribution == key {
        return Ok(None);
    }
    match evaluate_contribution(work, routes, &mut state.contribution_vectors) {
        Ok(operations) => {
            state.malformed_contribution = None;
            Ok(Some(operations))
        }
        Err(ContributionFailure::MalformedReply) => {
            state.malformed_contribution = key;
            Ok(None)
        }
        Err(ContributionFailure::Owner(error)) => Err(error.into()),
    }
}

fn publish(
    owner: &PhyWork<'_>,
    expected_progress: Option<[u8; 32]>,
    progress: Option<Progress>,
    prepared: impl IntoIterator<Item = PreparedObservation>,
    mut operations: Vec<ObservationOperation>,
) -> Result<(), GatewayError> {
    let progress = progress
        .map(|value| {
            let bytes = value.encode()?;
            Ok::<_, OwnerSessionError>(ObservationAuditProgress {
                previous: expected_progress,
                fingerprint: Sha256::digest(&bytes).into(),
                bytes,
            })
        })
        .transpose()?;
    owner.with_writer(|phy| {
        if phy.observation_progress().map(|row| row.fingerprint) != expected_progress {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        let cut = phy.phy_observation_cut();
        let route = owner.routes.identity;
        let mut hash = Sha256::new();
        hash.update(b"reliquary-phy-perception-publication-v1\0");
        hash.update(route);
        hash.update(expected_progress.unwrap_or([0; 32]));
        for observation in prepared {
            observation
                .revalidate_owner(ObservationRead::Phy(phy), route)
                .map_err(draft_error)?;
            hash.update(observation.input_fingerprint);
            operations.push(observation.operation);
        }
        if let Some(progress) = &progress {
            hash.update(progress.fingerprint);
        }
        hash.update(
            serde_json::to_vec(&operations).map_err(|_| OwnerSessionError::ManagementEvidence)?,
        );
        phy.publish_observation_batch(PhyObservationBatch {
            cut,
            batch: ObservationBatch {
                owner_id: cut.audit.owner_id,
                history_generation: cut.audit.history_generation,
                input_fingerprint: hash.finalize().into(),
                policy: ObservationPolicy {
                    version: 1,
                    general_route: route,
                    decision_route: route,
                },
                operations,
                progress,
            },
        })?;
        Ok(())
    })
}

fn unavailable(reason: &'static str) -> GatewayError {
    GatewayError::RetrievalUnavailable(reason)
}
