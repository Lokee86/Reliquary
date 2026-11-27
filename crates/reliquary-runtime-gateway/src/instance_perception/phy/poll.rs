use super::{
    PhyWork,
    progress::{Page, PageKind, Progress},
};
use crate::{GatewayError, RelSession};
use reliquary_application::{
    ego_pending::EgoBatchKey,
    observation_drafts::{
        ObservationRead, ObservationWorkflowError, PreparedObservation, ScopedObservationRead,
        evaluate_review,
    },
    perception_workflow::{
        BatchLimits, ContributionEmbeddingCache, Eligibility, ListResult, LookupError,
        LookupRequest, PerceptionRoutes, audit_owner_cut, capture_phy_contribution,
        capture_phy_duplicate_contribution, capture_phy_perception_discovery,
        discover_phy_for_features, phy_audit_lookups,
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
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use super::super::poll::{
    claim_vectors::ClaimVectorIndex, draft_error, draft_routes, evaluate_contribution_once,
    prepare_candidates,
};

const REVIEW_SCAN_LIMIT: usize = 16;

#[derive(Default)]
pub(super) struct State {
    review_cut: Option<PerceptionSourceCut>,
    review_after: Option<ObservationId>,
    review_exhausted: bool,
    contribution_vectors: ContributionEmbeddingCache,
    malformed_contribution: Option<[u8; 32]>,
    malformed_reviews: BTreeSet<ObservationId>,
    review_deferred: BTreeMap<ObservationId, Instant>,
    claim_vectors: ClaimVectorIndex,
    incoming_first: bool,
    page_retry: [Option<Instant>; 2],
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
    let first = usize::from(state.incoming_first);
    for lane in [first, 1 - first] {
        if state.page_retry[lane].is_some_and(|retry| retry > Instant::now()) {
            continue;
        }
        let page = if lane == 1 {
            progress.incoming_page.take()
        } else {
            progress.page.take()
        };
        if let Some(page) = page {
            state.incoming_first = lane == 0;
            let result = if page.kind.discovery() {
                discovery(owner, state, previous, progress, page)
            } else {
                contribution(owner, state, previous, progress, page)
            };
            if !matches!(result, Ok(true)) {
                state.malformed_contribution = None;
                state.page_retry[lane] = Some(Instant::now() + Duration::from_secs(10));
            } else {
                state.page_retry[lane] = None;
            }
            return result.or(Ok(true));
        }
        let planned = if lane == 1 {
            owner.read(|writer| progress.plan_incoming(writer))??
        } else {
            let ego_route = owner.ego_target()?.map(|_| owner.routes.identity);
            owner.read(|writer| progress.schedule(writer, ego_route))??
        };
        if planned {
            state.incoming_first = lane == 0;
            publish(owner, previous, Some(progress), [], Vec::new())?;
            return Ok(true);
        }
    }
    Ok(false)
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
        state.review_deferred.retain(|id, _| {
            writer.observation(*id).is_some_and(|head| {
                matches!(
                    head.standing,
                    ObservationStanding::Pending(
                        ObservationPendingReason::NewEvidence
                            | ObservationPendingReason::ChangedSupport
                    )
                )
            })
        });
        let now = Instant::now();
        if state.review_exhausted {
            if !state.review_deferred.values().any(|retry| *retry <= now) {
                return None;
            }
            state.review_after = None;
            state.review_exhausted = false;
        }
        let page = writer.observations_after(state.review_after, REVIEW_SCAN_LIMIT);
        for head in &page {
            let id = head.revision.id;
            state.review_after = Some(id);
            if !state.malformed_reviews.contains(&id)
                && state
                    .review_deferred
                    .get(&id)
                    .is_none_or(|retry| *retry <= now)
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
        }
        state.review_exhausted = page.len() < REVIEW_SCAN_LIMIT;
        (!state.review_exhausted).then_some(None)
    })?
    else {
        return Ok(false);
    };
    let Some(id) = target else { return Ok(true) };
    let routes = owner.fenced_routes();
    let result = (|| {
        let profile = state.claim_vectors.phy_memory_profile(owner, &routes)?;
        match evaluate_review(owner, &draft_routes(&routes, profile), id) {
            Ok(Some(prepared)) => publish(owner, previous, None, [prepared], Vec::new()),
            Ok(None) => Ok(()),
            Err(error) if error.malformed_native_reply() => {
                state.malformed_reviews.insert(id);
                Ok(())
            }
            Err(error) => Err(draft_error(error)),
        }
    })();
    if result.is_err() {
        state
            .review_deferred
            .insert(id, Instant::now() + Duration::from_secs(10));
    } else {
        state.review_deferred.remove(&id);
    }
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
    let Some(operations) = evaluate_contribution_once(
        &routes,
        &mut state.contribution_vectors,
        &mut state.malformed_contribution,
        &work,
    )?
    else {
        return Ok(false);
    };
    page.contribution_cursor = work.next_source;
    page.target_after = work.next_target;
    if page.kind == PageKind::Incoming {
        progress.incoming_page = Some(page);
    } else {
        progress.page = Some(page);
    }
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
    let mut shared = None;
    if let Some(shared_page) = page.shared {
        if shared_page.route == owner.routes.identity
            && let Some(target) = owner.ego_target()?
        {
            let run = shared_page.window.run(shared_page.route);
            if progress.closed_ego(run) {
                shared = Some((target, run));
            } else {
                let available = target.with_pending(|pending| {
                    if pending
                        .checkpoint::<crate::ego_audit_handoff::SharedPhyAudit>(run)?
                        .is_none()
                    {
                        pending.save_checkpoint(run, &shared_page.window)?;
                    }
                    Ok(())
                })?;
                if available.is_some() {
                    shared = Some((target, run));
                }
            }
        }
        if shared.is_none() {
            progress.abandon_ego(shared_page);
            page.shared = None;
        }
    }
    if usize::from(page.cursor.source) >= page.sources.len() + page.derived.len() {
        progress.finish_page(page.shared);
        publish(owner, previous, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    let eligibility = if shared.is_some() {
        Eligibility::BOTH
    } else {
        Eligibility::PERCEPTION
    };
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
            eligibility,
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
    let batch = capture
        .batch_features(page.batch as usize)
        .ok_or(OwnerSessionError::ManagementStale)?;
    let batch_key = EgoBatchKey {
        capture: capture.key,
        ordinal: page.batch,
    };
    let ego_pending = if batch.ego
        && let Some((target, run)) = &shared
        && !progress.closed_ego(*run)
    {
        target
            .with_pending(|pending| pending.accepted_batch(*run, batch_key))?
            .flatten()
            .is_none()
    } else {
        false
    };
    let perception_pending = batch.perception && page.proposals.is_none();
    if !ego_pending
        && let Some((_, run)) = &shared
        && page.shared.is_some_and(|shared| shared.last)
        && usize::from(capture.next.source) == page.sources.len() + page.derived.len()
        && ((page.batch as usize + 1)..capture.batch_count()).all(|ordinal| {
            !capture
                .batch_features(ordinal)
                .is_some_and(|features| features.ego)
        })
        && !progress.closed_ego(*run)
    {
        progress.finish_page(page.shared);
        progress.page = Some(page);
        publish(owner, previous, Some(progress), [], Vec::new())?;
        return Ok(true);
    }
    if !ego_pending && !perception_pending {
        let (drafts, operations) = if let Some(proposals) = page.proposals.take() {
            evaluate_proposals(owner, state, &routes, &page, proposals)?
        } else {
            (Vec::new(), Vec::new())
        };
        page.batch += 1;
        progress.page = Some(page);
        publish(owner, previous, Some(progress), drafts, operations)?;
        return Ok(true);
    }
    let mut lookup = |request: LookupRequest<'_>| {
        owner
            .read(|phy| phy_audit_lookups(phy, page.cut.audit, request))
            .map_err(|_| LookupError::Invalidated)?
            .map_err(|_| LookupError::Unavailable)
    };
    let result = discover_phy_for_features(
        &capture,
        page.batch as usize,
        Eligibility {
            ego: ego_pending,
            perception: perception_pending,
        },
        routes.general.as_ref(),
        &mut lookup,
    )
    .map_err(|_| unavailable("PHY audit discovery remains pending"))?;
    let mut changed = false;
    if let ListResult::Accepted { candidates, .. } = result.perception {
        page.proposals = Some(candidates);
        changed = true;
    }
    let ego_ack = if let (Some((target, run)), ListResult::Accepted { candidates, .. }) =
        (&shared, result.ego)
    {
        target
            .with_pending(|pending| {
                let _routes = owner.live_routes().map_err(|_| {
                    reliquary_application::ego_pending::EgoPendingError::InvalidBinding
                })?;
                pending.accept_batch(*run, batch_key, &candidates)
            })
            .map(|accepted| accepted.is_some())
    } else {
        Ok(false)
    };
    if !changed {
        if ego_ack? {
            return Ok(true);
        }
        return Err(unavailable("PHY audit list remains pending"));
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
    let (drafts, duplicates) = prepare_candidates(
        owner,
        &draft_routes,
        audit_owner_cut(&page.cut.into()),
        proposals,
    )
    .map_err(draft_error)?;
    let mut operations = Vec::new();
    for ((id, revision), evidence) in duplicates {
        let selected = evidence.into_iter().collect::<Vec<_>>();
        let work = owner
            .read(|phy| capture_phy_duplicate_contribution(phy, id, revision, &selected))??;
        let Some(decisions) = evaluate_contribution_once(
            routes,
            &mut state.contribution_vectors,
            &mut state.malformed_contribution,
            &work,
        )?
        else {
            return Err(unavailable(
                "PHY Perception contribution waits for changed evidence",
            ));
        };
        operations.extend(decisions);
    }
    Ok((drafts, operations))
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
