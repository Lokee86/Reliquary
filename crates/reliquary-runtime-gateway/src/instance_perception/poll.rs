use super::*;
use reliquary_application::observation_drafts::{
    ObservationDraftRoutes, ObservationWorkflowError, PreparedObservation, ProposalEvaluation,
    ScopedObservationRead, evaluate_proposal, evaluate_review,
};
use reliquary_application::perception_workflow::{
    EgoCandidate, ListResult, LookupError, audit_lookups, audit_owner_cut, capture_contribution,
    capture_discovery, capture_duplicate_contribution, discover, evaluate_contribution,
    retain_current_ego_evidence,
};
use reliquary_owner_session::observation::{
    ObservationAuditProgress, ObservationBatch, ObservationChange, ObservationOperation,
    ObservationPendingReason, ObservationPolicy, ObservationStanding,
};
use reliquary_owner_session::{
    PerceptionAuditAdvance, PerceptionAuditPage, PerceptionAuditProgress, PerceptionSourceCut,
};
use sha2::{Digest, Sha256};

const REVIEW_SCAN_LIMIT: usize = 16;

impl ScopedObservationRead for OwnerWork<'_> {
    fn read<T>(
        &self,
        capture: impl FnOnce(&RelSession) -> Result<T, ObservationWorkflowError>,
    ) -> Result<T, ObservationWorkflowError> {
        OwnerWork::read(self, capture)
            .map_err(|_| ObservationWorkflowError::Owner(OwnerSessionError::ManagementStale))?
    }
}

enum TickInput {
    Initialize(PerceptionAuditProgress),
    Audit {
        previous: [u8; 32],
        progress: PerceptionAuditProgress,
        advance: PerceptionAuditAdvance,
    },
    Review(ObservationId),
    Sweep,
}

pub(super) fn tick(owner: &OwnerWork<'_>, state: &mut WorkerState) -> Result<bool, GatewayError> {
    let input = owner.read(|rel| prepare(rel, state))??;
    match input {
        TickInput::Initialize(progress) => {
            publish(owner, None, Some(progress), [], Vec::new(), |_| Ok(()))?;
            Ok(true)
        }
        TickInput::Audit {
            previous,
            progress,
            advance,
        } => match advance {
            PerceptionAuditAdvance::Idle => Ok(false),
            PerceptionAuditAdvance::Updated => {
                publish(
                    owner,
                    Some(previous),
                    Some(progress),
                    [],
                    Vec::new(),
                    |_| Ok(()),
                )?;
                Ok(true)
            }
            PerceptionAuditAdvance::Ready => audit_page(owner, state, previous, progress),
            PerceptionAuditAdvance::LiveContributionReady => {
                live_contribution(owner, previous, progress)
            }
        },
        TickInput::Review(id) => {
            let result = (|| {
                if let Some(review) =
                    evaluate_review(owner, &draft_routes(owner.routes), id).map_err(draft_error)?
                {
                    let previous =
                        owner.read(|rel| rel.observation_progress().map(|row| row.fingerprint))?;
                    publish(owner, previous, None, [review], Vec::new(), |_| Ok(()))?;
                }
                Ok::<(), GatewayError>(())
            })();
            if result.is_err() {
                state
                    .review_deferred
                    .insert(id, Instant::now() + RETRY_INTERVALS[1]);
            } else {
                state.review_deferred.remove(&id);
            }
            Ok(true)
        }
        TickInput::Sweep => Ok(true),
    }
}

fn prepare(rel: &RelSession, state: &mut WorkerState) -> Result<TickInput, OwnerSessionError> {
    let Some(row) = rel.observation_progress() else {
        return Ok(TickInput::Initialize(PerceptionAuditProgress::new(
            rel.perception_source_cut(),
        )));
    };
    let cut = rel.perception_source_cut();
    let changes = (cut.memory, cut.entity, rel.perception_review_version());
    let now = Instant::now();
    state.review_deferred.retain(|id, _| {
        rel.observation(*id).is_some_and(|head| {
            matches!(
                head.standing,
                ObservationStanding::Pending(
                    ObservationPendingReason::NewEvidence
                        | ObservationPendingReason::ChangedSupport
                )
            )
        })
    });
    if state.review_cut != Some(changes)
        || (state.review_exhausted && state.review_deferred.values().any(|retry| *retry <= now))
    {
        state.review_cut = Some(changes);
        state.review_after = None;
        state.review_exhausted = false;
    }
    if !state.review_exhausted {
        let inspections = rel.observations_after(state.review_after, REVIEW_SCAN_LIMIT);
        for head in &inspections {
            state.review_after = Some(head.revision.id);
            if state
                .review_deferred
                .get(&head.revision.id)
                .is_none_or(|retry| *retry <= now)
                && matches!(
                    head.standing,
                    ObservationStanding::Pending(
                        ObservationPendingReason::NewEvidence
                            | ObservationPendingReason::ChangedSupport
                    )
                )
            {
                return Ok(TickInput::Review(head.revision.id));
            }
        }
        if inspections.len() == REVIEW_SCAN_LIMIT {
            return Ok(TickInput::Sweep);
        }
        state.review_exhausted = true;
    }
    let mut progress = PerceptionAuditProgress::decode(&row.bytes)?;
    let advance = progress.advance(rel)?;
    Ok(TickInput::Audit {
        previous: row.fingerprint,
        progress,
        advance,
    })
}

fn audit_page(
    owner: &OwnerWork<'_>,
    state: &mut WorkerState,
    previous: [u8; 32],
    mut progress: PerceptionAuditProgress,
) -> Result<bool, GatewayError> {
    let page = progress
        .page
        .as_mut()
        .ok_or(OwnerSessionError::ManagementStale)?;
    if usize::from(page.contribution_cursor) < page.sources.len() {
        let work = owner.read(|rel| capture_contribution(rel, page, true))??;
        let operations = evaluate_contribution(&work, owner.routes)?;
        page.contribution_cursor = work.next_source;
        page.contribution_target_after = work.next_target;
        publish(
            owner,
            Some(previous),
            Some(progress),
            [],
            operations,
            |_| Ok(()),
        )?;
        return Ok(true);
    }
    if usize::from(page.discovery.source) == page.sources.len() + page.derived.len() {
        progress.finish_page();
        publish(
            owner,
            Some(previous),
            Some(progress),
            [],
            Vec::new(),
            |_| Ok(()),
        )?;
        return Ok(true);
    }

    let capture = owner.read(|rel| capture_discovery(rel, page, owner.routes.identity))??;
    let page_before = page.clone();
    page.bind_capture(capture.key);
    if page.batch as usize >= capture.batch_count() {
        page.finish_capture(capture.next)?;
        publish(
            owner,
            Some(previous),
            Some(progress),
            [],
            Vec::new(),
            |_| Ok(()),
        )?;
        return Ok(true);
    }
    let mut lookup = |feature, query: &str| {
        owner
            .read(|rel| audit_lookups(rel, &page.cut, feature, query))
            .map_err(|_| LookupError::Invalidated)?
            .map_err(|_| LookupError::Unavailable)
    };
    let discovery = discover(
        &capture,
        page.batch as usize,
        owner.routes.general.as_ref(),
        &mut lookup,
    )
    .map_err(|_| workflow_wait("Perception discovery response unavailable"))?;
    let mut ego = None;
    let mut changed = false;
    if !page.ego_accepted {
        match discovery.ego {
            ListResult::Inactive => {
                page.ego_accepted = true;
                changed = true;
            }
            ListResult::Accepted { candidates, .. } => {
                ego = Some(candidates);
                page.ego_accepted = true;
                changed = true;
            }
            ListResult::Incomplete(_) => {}
        }
    }
    let mut drafts = Vec::new();
    let mut operations = Vec::new();
    if !page.perception_accepted {
        match discovery.perception {
            ListResult::Inactive => {
                page.perception_accepted = true;
                changed = true;
            }
            ListResult::Accepted { candidates, .. } => {
                match proposals(owner, &page.cut, candidates) {
                    Ok((prepared, contribution)) => {
                        drafts = prepared;
                        operations = contribution;
                        page.perception_accepted = true;
                        changed = true;
                    }
                    Err(error) if !changed => return Err(error),
                    Err(_) => {}
                }
            }
            ListResult::Incomplete(_) => {}
        }
    }
    if !changed {
        return Err(workflow_wait("Perception discovery list remains pending"));
    }
    if page.ego_accepted && page.perception_accepted {
        page.finish_batch()?;
    }
    publish(
        owner,
        Some(previous),
        Some(progress),
        drafts,
        operations,
        |rel| {
            let now = capture_discovery(rel, &page_before, owner.routes.identity)?;
            if now.key != capture.key {
                return Err(OwnerSessionError::ManagementStale.into());
            }
            if let Some(candidates) = &mut ego {
                retain_current_ego_evidence(rel, candidates)?;
            }
            Ok(())
        },
    )?;
    if let Some(ego) = ego {
        state.ego_candidates = ego;
    }
    Ok(true)
}

fn live_contribution(
    owner: &OwnerWork<'_>,
    previous: [u8; 32],
    mut progress: PerceptionAuditProgress,
) -> Result<bool, GatewayError> {
    let page = progress
        .live_contribution
        .as_mut()
        .ok_or(OwnerSessionError::ManagementStale)?;
    let operations = if usize::from(page.contribution_cursor) < page.sources.len() {
        let work = owner.read(|rel| capture_contribution(rel, page, false))??;
        let operations = evaluate_contribution(&work, owner.routes)?;
        page.contribution_cursor = work.next_source;
        page.contribution_target_after = work.next_target;
        operations
    } else {
        progress.live_contribution = None;
        Vec::new()
    };
    publish(
        owner,
        Some(previous),
        Some(progress),
        [],
        operations,
        |_| Ok(()),
    )?;
    Ok(true)
}

fn proposals(
    owner: &OwnerWork<'_>,
    cut: &PerceptionSourceCut,
    candidates: Vec<reliquary_application::perception_workflow::ProposedClaim>,
) -> Result<(Vec<PreparedObservation>, Vec<ObservationOperation>), GatewayError> {
    let routes = draft_routes(owner.routes);
    let mut drafts = Vec::new();
    let mut operations = Vec::new();
    let mut seen = BTreeSet::new();
    let mut duplicate_targets = BTreeMap::<_, BTreeSet<_>>::new();
    for candidate in candidates {
        match evaluate_proposal(owner, &routes, audit_owner_cut(cut), &candidate)
            .map_err(draft_error)?
        {
            ProposalEvaluation::Abstained | ProposalEvaluation::Unresolved => {}
            ProposalEvaluation::Ready(draft) => {
                let identity = match &draft.operation {
                    ObservationOperation::Create { draft } => draft.claim_digest(),
                    _ => draft.input_fingerprint,
                };
                if seen.insert(identity) {
                    drafts.push(draft);
                }
            }
            ProposalEvaluation::Duplicate { target, evidence } => {
                duplicate_targets
                    .entry((ObservationId(target.id), target.revision))
                    .or_default()
                    .extend(evidence);
            }
        }
    }
    for ((target, revision), evidence) in duplicate_targets {
        let evidence = evidence.into_iter().collect::<Vec<_>>();
        let work = owner
            .read(|rel| capture_duplicate_contribution(rel, target, revision, &evidence))??;
        operations.extend(evaluate_contribution(&work, owner.routes)?);
    }
    Ok((drafts, operations))
}

fn draft_routes(routes: &PerceptionRoutes) -> ObservationDraftRoutes<'_> {
    ObservationDraftRoutes {
        general: routes.general.as_ref(),
        verification: routes.verification.as_ref(),
        embedding: routes.embedding.as_ref(),
        route_identity: routes.identity,
        embedding_profile: routes.embedding_profile(),
    }
}

fn publish(
    owner: &OwnerWork<'_>,
    expected_progress: Option<[u8; 32]>,
    progress: Option<PerceptionAuditProgress>,
    prepared: impl IntoIterator<Item = PreparedObservation>,
    operations: Vec<ObservationOperation>,
    fence: impl FnOnce(&RelSession) -> Result<(), GatewayError>,
) -> Result<(), GatewayError> {
    let progress = progress
        .map(|value| -> Result<_, OwnerSessionError> {
            let bytes = value.encode()?;
            Ok(ObservationAuditProgress {
                previous: expected_progress,
                fingerprint: Sha256::digest(&bytes).into(),
                bytes,
            })
        })
        .transpose()?;
    owner.with_writer(|writer| {
        let rel = writer.read_session();
        if rel.observation_progress().map(|row| row.fingerprint) != expected_progress {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        fence(rel)?;
        let mut operations = operations;
        let mut hash = Sha256::new();
        hash.update(b"reliquary-perception-work-publication-v1\0");
        hash.update(owner.routes.identity);
        hash.update(expected_progress.unwrap_or([0; 32]));
        for prepared in prepared {
            prepared
                .revalidate(rel, owner.routes.identity)
                .map_err(draft_error)?;
            hash.update(prepared.input_fingerprint);
            let mut operation = prepared.operation;
            credit_change(rel, &mut operation);
            operations.push(operation);
        }
        if let Some(progress) = &progress {
            hash.update(progress.fingerprint);
        }
        hash.update(
            serde_json::to_vec(&operations).map_err(|_| OwnerSessionError::ManagementEvidence)?,
        );
        let cut = rel.perception_source_cut();
        writer.publish_observation_batch(ObservationBatch {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            input_fingerprint: hash.finalize().into(),
            policy: ObservationPolicy {
                version: 1,
                general_route: owner.routes.identity,
                decision_route: owner.routes.identity,
            },
            operations,
            progress,
        })?;
        Ok(())
    })
}

fn credit_change(rel: &RelSession, operation: &mut ObservationOperation) {
    let ObservationOperation::Successor {
        id,
        draft,
        change,
        credited_activity,
        ..
    } = operation
    else {
        return;
    };
    if *change == ObservationChange::Equivalent {
        return;
    }
    *credited_activity = draft
        .citations
        .iter()
        .chain(&draft.reviewed)
        .filter_map(|cite| {
            let activity = rel.perception_source_activity(cite)?;
            let amount = rel.observation_semantic_match(*id, activity)?;
            Some((std::cmp::Reverse(amount), activity.source, activity))
        })
        .min()
        .map(|(_, _, activity)| activity);
}

fn draft_error(_error: ObservationWorkflowError) -> GatewayError {
    workflow_wait("Perception draft remains pending")
}

fn workflow_wait(reason: &'static str) -> GatewayError {
    GatewayError::RetrievalUnavailable(reason)
}
