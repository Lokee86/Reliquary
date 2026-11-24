use super::*;
use reliquary_application::observation_drafts::{
    ObservationDraftRoutes, ObservationWorkflowError, PreparedObservation, ProposalEvaluation,
    ScopedObservationRead, evaluate_proposal, evaluate_review,
};
use reliquary_application::perception_workflow::{
    ContributionFailure, ContributionWork, ListResult, LookupError, audit_lookups, audit_owner_cut,
    capture_contribution, capture_discovery, capture_duplicate_contribution, discover,
    evaluate_contribution, retain_current_ego_evidence,
};
use reliquary_application::{SwitchboardEmbeddingEndpoint, verify_compatibility_profile};
use reliquary_owner_session::observation::{
    ObservationAuditProgress, ObservationBatch, ObservationChange, ObservationOperation,
    ObservationPendingReason, ObservationPolicy, ObservationStanding,
};
use reliquary_owner_session::{
    PerceptionAuditAdvance, PerceptionAuditProgress, PerceptionSourceCut,
};
use sha2::{Digest, Sha256};

const REVIEW_SCAN_LIMIT: usize = 16;
const MALFORMED_NATIVE_DRAFT: &str = "Perception native verification response malformed";

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
                live_contribution(owner, state, previous, progress)
            }
        },
        TickInput::Review(id) => {
            let result = (|| {
                let profile = resolve_memory_profile(owner, state)?;
                let review = match evaluate_review(owner, &draft_routes(owner.routes, profile), id)
                {
                    Err(error) if error.malformed_native_reply() => {
                        if let Some(cut) = state.review_cut {
                            state.malformed_reviews.insert(id, cut);
                        }
                        return Ok(());
                    }
                    result => result.map_err(draft_error)?,
                };
                if let Some(review) = review {
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
    if !state.ego_candidates.is_empty() && state.ego_cut.as_ref() != Some(&cut) {
        retain_current_ego_evidence(rel, &mut state.ego_candidates)?;
        state.ego_cut = Some(cut.clone());
    }
    let changes = (
        cut.memory,
        cut.graph,
        cut.entity,
        rel.perception_review_version(),
    );
    state
        .malformed_reviews
        .retain(|_, snapshot| *snapshot == changes);
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
            if !state.malformed_reviews.contains_key(&head.revision.id)
                && state
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
        let profile = if page.sources.iter().any(|source| source.sampled) {
            resolve_memory_profile(owner, state)?
        } else {
            None
        };
        let work = owner.read(|rel| capture_contribution(rel, page, true, profile))??;
        let Some(operations) = process_contribution(owner, state, &work)? else {
            return Ok(false);
        };
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
    let batch_key = (capture.key, page.batch);
    let creation_held = state.malformed_creation == Some(batch_key);
    if creation_held && page.ego_accepted {
        return Ok(false);
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
    if !page.perception_accepted && !creation_held {
        match discovery.perception {
            ListResult::Inactive => {
                page.perception_accepted = true;
                changed = true;
            }
            ListResult::Accepted { candidates, .. } => {
                let profile = if candidates.is_empty() {
                    None
                } else {
                    resolve_memory_profile(owner, state)?
                };
                match proposals(owner, state, &page.cut, profile, candidates) {
                    Ok((prepared, contribution)) => {
                        drafts = prepared;
                        operations = contribution;
                        state.malformed_creation = None;
                        page.perception_accepted = true;
                        changed = true;
                    }
                    Err(GatewayError::RetrievalUnavailable(MALFORMED_NATIVE_DRAFT)) => {
                        state.malformed_creation = Some(batch_key);
                        if !changed {
                            return Ok(false);
                        }
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
    state: &mut WorkerState,
    previous: [u8; 32],
    mut progress: PerceptionAuditProgress,
) -> Result<bool, GatewayError> {
    let page = progress
        .live_contribution
        .as_mut()
        .ok_or(OwnerSessionError::ManagementStale)?;
    let operations = if usize::from(page.contribution_cursor) < page.sources.len() {
        let profile = resolve_memory_profile(owner, state)?;
        let work = owner.read(|rel| capture_contribution(rel, page, false, profile))??;
        let Some(operations) = process_contribution(owner, state, &work)? else {
            return Ok(false);
        };
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
    state: &mut WorkerState,
    cut: &PerceptionSourceCut,
    memory_profile: Option<CompatibilityProfileId>,
    candidates: Vec<reliquary_application::perception_workflow::ProposedClaim>,
) -> Result<(Vec<PreparedObservation>, Vec<ObservationOperation>), GatewayError> {
    let routes = draft_routes(owner.routes, memory_profile);
    let mut drafts = Vec::new();
    let mut operations = Vec::new();
    let mut seen = BTreeSet::new();
    let mut duplicate_targets = BTreeMap::<_, BTreeSet<_>>::new();
    for candidate in candidates {
        let evaluated = match evaluate_proposal(owner, &routes, audit_owner_cut(cut), &candidate) {
            Ok(value) => value,
            Err(
                ObservationWorkflowError::InvalidDraft | ObservationWorkflowError::EvidenceLimit,
            ) => continue,
            Err(error) => return Err(draft_error(error)),
        };
        match evaluated {
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
        let Some(decisions) = process_contribution(owner, state, &work)? else {
            return Err(workflow_wait(
                "Perception contribution waits for changed evidence",
            ));
        };
        operations.extend(decisions);
    }
    Ok((drafts, operations))
}

fn process_contribution(
    owner: &OwnerWork<'_>,
    state: &mut WorkerState,
    work: &ContributionWork,
) -> Result<Option<Vec<ObservationOperation>>, GatewayError> {
    let key = work.decision_key();
    if key.is_some() && state.malformed_contribution == key {
        return Ok(None);
    }
    match evaluate_contribution(work, owner.routes, &mut state.contribution_vectors) {
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

fn draft_routes(
    routes: &PerceptionRoutes,
    memory_vector_profile: Option<CompatibilityProfileId>,
) -> ObservationDraftRoutes<'_> {
    ObservationDraftRoutes {
        general: routes.general.as_ref(),
        verification: routes.verification.as_ref(),
        embedding: routes.embedding.as_ref(),
        route_identity: routes.identity,
        embedding_profile: routes.embedding_profile(),
        memory_vector_profile,
    }
}

/// Reuse only existing owner vectors whose recorded probe profile the
/// configured route has verified. The cache lives with the route and mount.
fn resolve_memory_profile(
    owner: &OwnerWork<'_>,
    state: &mut WorkerState,
) -> Result<Option<CompatibilityProfileId>, GatewayError> {
    if !owner.routes.embedding.supports_bounded_embedding() {
        return Ok(None);
    }
    if let Some(id) = state.memory_profile {
        if owner.read(|rel| rel.active_compatibility_profile(id).is_some())? {
            return Ok(Some(id));
        }
        state.memory_profile = None;
    }
    let candidate = owner.read(|rel| {
        rel.memory_vector_infos()
            .into_iter()
            .map(|info| info.compatibility_profile_id)
            .filter(|id| !state.probed_profiles.contains(id))
            .filter_map(|id| rel.active_compatibility_profile(id))
            .find(|profile| profile.dimensions == owner.routes.embedding.dimensions())
            .cloned()
    })?;
    if let Some(profile) = candidate {
        state.probed_profiles.insert(profile.id);
        let endpoint = SwitchboardEmbeddingEndpoint::new(owner.routes.embedding.clone());
        if verify_compatibility_profile(&profile, &endpoint).is_ok() {
            state.memory_profile = Some(profile.id);
        }
    }
    Ok(state.memory_profile)
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

fn draft_error(error: ObservationWorkflowError) -> GatewayError {
    if error.malformed_native_reply() {
        workflow_wait(MALFORMED_NATIVE_DRAFT)
    } else {
        workflow_wait("Perception draft remains pending")
    }
}

fn workflow_wait(reason: &'static str) -> GatewayError {
    GatewayError::RetrievalUnavailable(reason)
}
