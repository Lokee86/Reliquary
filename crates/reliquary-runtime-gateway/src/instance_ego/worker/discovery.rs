use super::*;
use reliquary_application::perception_workflow::{
    Eligibility, ListResult, LookupError, LookupRequest, audit_lookups, capture_discovery,
    capture_phy_discovery, discover, discover_phy, phy_audit_citation, phy_audit_lookups,
    retain_current_ego_evidence,
};
use reliquary_owner_session::{
    AUDIT_WORK_PAGE_SIZE, AuditDiscoveryCursor, AuditWorkSource, MemoryId, PerceptionAuditPage,
    PerceptionSourceCut, PhyAuditChange, PhyAuditCursor, PhyAuditCut, model::MemoryTextField,
};
use serde::{Deserialize, Serialize};

const PHY_REFRESH_CHANGES: usize = 5;
const REL_REFRESH_SETTLEMENTS: u64 = 25;

#[derive(Deserialize, Serialize)]
struct Page {
    sources: Vec<MemoryId>,
    next: Option<MemoryId>,
    cursor: AuditDiscoveryCursor,
}

impl Page {
    fn new(sources: Vec<MemoryId>, next: Option<MemoryId>) -> Self {
        Self {
            sources,
            next,
            cursor: AuditDiscoveryCursor {
                source: 0,
                field: MemoryTextField::Title,
                byte: 0,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
struct RelBootstrap {
    cut: PerceptionSourceCut,
    after: Option<MemoryId>,
    page: Option<Page>,
    done: bool,
    #[serde(default)]
    incremental: Option<RelIncremental>,
}

#[derive(Deserialize, Serialize)]
struct RelIncremental {
    base: EgoSourceCut,
    settlements: u64,
    memory: u64,
}

impl RelIncremental {
    fn due(prior: EgoSourceCut, current: &PerceptionSourceCut) -> bool {
        prior.owner_id == current.owner_id
            && prior.history_generation == current.history_generation
            && prior.memory_version <= current.memory
            && current.settlements.saturating_sub(prior.audit_frontier) >= REL_REFRESH_SETTLEMENTS
    }

    fn next_page(&mut self, rel: &RelSession, cut: &PerceptionSourceCut) -> Page {
        let rows = if self.settlements < cut.settlements {
            let rows = rel.perception_settlements_page(
                self.settlements,
                cut.settlements,
                AUDIT_WORK_PAGE_SIZE,
            );
            self.settlements = rows
                .last()
                .map_or(cut.settlements, |(position, _)| *position);
            rows
        } else {
            let rows =
                rel.perception_memory_changes_page(self.memory, cut.memory, AUDIT_WORK_PAGE_SIZE);
            self.memory = rows.last().map_or(cut.memory, |(position, _)| *position);
            rows
        };
        let mut sources: Vec<_> = rows.into_iter().map(|(_, id)| id).collect();
        sources.sort_unstable();
        sources.dedup();
        Page::new(sources, None)
    }

    fn complete(&self, cut: &PerceptionSourceCut) -> bool {
        self.settlements == cut.settlements && self.memory == cut.memory
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct PhyDiscovery {
    pub(super) cut: PhyAuditCut,
    pub(super) changes: Vec<PhyAuditChange>,
    phase: PhyPhase,
}

#[derive(Deserialize, Serialize)]
enum PhyPhase {
    Initial {
        after: Option<MemoryId>,
        page: Option<Page>,
    },
    Changes {
        prior: PhyAuditCut,
        cursor: PhyAuditCursor,
    },
    Waiting,
    Refresh {
        position: usize,
        page: Option<Page>,
    },
    Done,
}

impl EgoWorker {
    pub(super) fn advance_rel_bootstrap(
        &self,
        owner: OwnerId,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        self.advance_rel_walk(owner, pending, None)
    }

    pub(super) fn advance_rel_recurring(
        &self,
        owner: OwnerId,
        prior: EgoSourceCut,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        self.advance_rel_walk(owner, pending, Some(prior))
    }

    fn advance_rel_walk(
        &self,
        owner: OwnerId,
        pending: &mut EgoPendingWork,
        prior: Option<EgoSourceCut>,
    ) -> Result<bool, String> {
        let kind = if prior.is_some() {
            EgoRunKind::RelIndependentRecurring
        } else {
            EgoRunKind::RelBootstrap
        };
        let current = self
            .instance
            .with_reliquary(owner, RelSession::perception_source_cut)
            .map_err(as_error)?;
        let (run, mut walk) = if let Some(run) = pending
            .open_checkpoint_run(self.phy.0, owner.0)
            .map_err(as_error)?
        {
            let walk: RelBootstrap = pending
                .checkpoint(run)
                .map_err(as_error)?
                .ok_or("Ego REL checkpoint missing")?;
            if run.kind != kind
                || walk
                    .incremental
                    .as_ref()
                    .map(|incremental| incremental.base)
                    != prior
                || run.route != self.routes.identity
                || run.contract != CONTRACT_VERSION
                || walk.cut.history_generation != current.history_generation
                || walk.cut.source_removal != current.source_removal
            {
                pending.discard_stale(run).map_err(as_error)?;
                return Ok(true);
            }
            (run, walk)
        } else {
            let incremental = if let Some(prior) = prior {
                if !RelIncremental::due(prior, &current) {
                    return Ok(false);
                }
                Some(RelIncremental {
                    base: prior,
                    settlements: prior.audit_frontier,
                    memory: prior.memory_version,
                })
            } else {
                None
            };
            let run = self.new_run(
                owner,
                current.history_generation,
                current.memory,
                current.settlements,
                kind,
            );
            let walk = RelBootstrap {
                cut: current,
                after: None,
                page: None,
                done: false,
                incremental,
            };
            pending.save_checkpoint(run, &walk).map_err(as_error)?;
            (run, walk)
        };
        if walk.done {
            pending.seal(run).map_err(as_error)?;
            return Ok(true);
        }
        let Some(page) = &mut walk.page else {
            walk.page = Some(
                self.instance
                    .with_reliquary(owner, |rel| {
                        if let Some(incremental) = &mut walk.incremental {
                            return incremental.next_page(rel, &walk.cut);
                        }
                        let sources =
                            rel.perception_source_ids_page(walk.after, AUDIT_WORK_PAGE_SIZE);
                        let next = (sources.len() == AUDIT_WORK_PAGE_SIZE)
                            .then(|| sources[AUDIT_WORK_PAGE_SIZE - 1]);
                        Page::new(sources, next)
                    })
                    .map_err(as_error)?,
            );
            pending.save_checkpoint(run, &walk).map_err(as_error)?;
            return Ok(true);
        };
        if usize::from(page.cursor.source) == page.sources.len() {
            walk.after = page.next;
            walk.done = walk.incremental.as_ref().map_or_else(
                || page.next.is_none(),
                |incremental| incremental.complete(&walk.cut),
            );
            walk.page = None;
            pending.save_checkpoint(run, &walk).map_err(as_error)?;
            return Ok(true);
        }
        let page_input = PerceptionAuditPage {
            cut: walk.cut.clone(),
            sources: page
                .sources
                .iter()
                .map(|&memory| AuditWorkSource {
                    memory,
                    sampled: false,
                    newly_settled: false,
                    ego_only: true,
                })
                .collect(),
            derived: Vec::new(),
            discovery: page.cursor,
            batch: 0,
            capture_key: None,
            ego_accepted: false,
            perception_accepted: true,
            contribution_cursor: 0,
            contribution_target_after: None,
        };
        let capture = self
            .instance
            .with_reliquary(owner, |rel| {
                capture_discovery(rel, &page_input, self.routes.identity)
            })
            .map_err(as_error)?
            .map_err(as_error)?;
        for ordinal in 0..capture.batch_count() {
            let key = EgoBatchKey {
                capture: capture.key,
                ordinal: ordinal as u32,
            };
            if pending
                .accepted_batch(run, key)
                .map_err(as_error)?
                .is_some()
            {
                continue;
            }
            let mut lookup = |feature, request: LookupRequest<'_>| {
                self.still_authorized(owner)
                    .map_err(|_| LookupError::Invalidated)?;
                self.instance
                    .with_reliquary(owner, |rel| audit_lookups(rel, &walk.cut, feature, request))
                    .map_err(|_| LookupError::Invalidated)?
                    .map_err(|_| LookupError::Unavailable)
            };
            let result = discover(
                &capture,
                ordinal,
                self.routes.general.as_ref(),
                Eligibility::EGO,
                &mut lookup,
            )
            .map_err(as_error)?;
            let mut candidates = accepted(result.ego)?;
            self.still_authorized(owner).map_err(as_error)?;
            if run.kind == EgoRunKind::RelIndependentRecurring
                && self.shared_perception_available(owner)
            {
                pending.discard_stale(run).map_err(as_error)?;
                return Ok(true);
            }
            self.instance
                .with_reliquary(owner, |rel| {
                    retain_current_ego_evidence(rel, &mut candidates)
                })
                .map_err(as_error)?
                .map_err(as_error)?;
            pending
                .accept_batch(run, key, &candidates)
                .map_err(as_error)?;
            return Ok(true);
        }
        if capture.next <= page.cursor {
            return Err("Ego REL discovery cursor did not advance".into());
        }
        page.cursor = capture.next;
        pending.save_checkpoint(run, &walk).map_err(as_error)?;
        Ok(true)
    }

    pub(super) fn advance_phy(&self, pending: &mut EgoPendingWork) -> Result<bool, String> {
        let phy = self.phy.0;
        let (cut, previous) = self
            .instance
            .with_ego_phy_writer(self.phy, |owner| {
                let cut = owner.phy_audit_cut();
                let previous = owner
                    .ego_phy_summary()
                    .map(|summary| summary.completion.cut)
                    .filter(|prior| prior.history_generation == cut.history_generation);
                Ok((cut, previous))
            })
            .map_err(as_error)?;
        let (run, mut walk) = if let Some(run) =
            pending.open_checkpoint_run(phy, phy).map_err(as_error)?
        {
            let walk: PhyDiscovery = pending
                .checkpoint(run)
                .map_err(as_error)?
                .ok_or("Ego PHY checkpoint missing")?;
            if run.route != self.routes.identity
                || run.contract != CONTRACT_VERSION
                || !walk.cut.is_at_or_before(cut)
                || (matches!(walk.phase, PhyPhase::Waiting)
                    && (cut.memory != walk.cut.memory || cut.settlements != walk.cut.settlements))
            {
                pending.discard_stale(run).map_err(as_error)?;
                return Ok(true);
            }
            (run, walk)
        } else {
            let (kind, phase) = match previous {
                Some(previous)
                    if previous.memory_version == cut.memory
                        && previous.audit_frontier == cut.settlements =>
                {
                    return Ok(false);
                }
                Some(previous) => {
                    let prior = PhyAuditCut {
                        owner_id: phy,
                        history_generation: cut.history_generation,
                        memory: previous.memory_version,
                        graph: 0,
                        settlements: previous.audit_frontier,
                    };
                    (
                        EgoRunKind::PhyRefresh,
                        PhyPhase::Changes {
                            prior,
                            cursor: prior.into(),
                        },
                    )
                }
                None => (
                    EgoRunKind::PhyBootstrap,
                    PhyPhase::Initial {
                        after: None,
                        page: None,
                    },
                ),
            };
            let run = self.new_run(
                self.phy,
                cut.history_generation,
                cut.memory,
                cut.settlements,
                kind,
            );
            let walk = PhyDiscovery {
                cut,
                changes: Vec::new(),
                phase,
            };
            pending.save_checkpoint(run, &walk).map_err(as_error)?;
            (run, walk)
        };
        walk.phase = match walk.phase {
            PhyPhase::Done => {
                pending.seal(run).map_err(as_error)?;
                return Ok(true);
            }
            PhyPhase::Waiting => return Ok(false),
            PhyPhase::Changes { prior, cursor } => {
                let page = self
                    .instance
                    .with_ego_phy_writer(self.phy, |owner| {
                        owner.phy_audit_changes_page(prior, walk.cut, cursor)
                    })
                    .map_err(as_error)?;
                walk.changes.extend(page.changes);
                walk.changes.sort_unstable_by_key(|change| change.memory);
                walk.changes.dedup_by_key(|change| change.memory);
                if page.next == PhyAuditCursor::from(walk.cut) {
                    if walk.changes.len() < PHY_REFRESH_CHANGES {
                        PhyPhase::Waiting
                    } else {
                        PhyPhase::Refresh {
                            position: 0,
                            page: None,
                        }
                    }
                } else if page.next == cursor {
                    return Err("Ego PHY change cursor did not advance".into());
                } else {
                    PhyPhase::Changes {
                        prior,
                        cursor: page.next,
                    }
                }
            }
            PhyPhase::Initial { after, page: None } => {
                let page = self
                    .instance
                    .with_ego_phy_writer(self.phy, |owner| {
                        owner.phy_audit_initial_page(walk.cut, after)
                    })
                    .map_err(as_error)?;
                PhyPhase::Initial {
                    after,
                    page: Some(Page::new(page.sources, page.next)),
                }
            }
            PhyPhase::Initial {
                after,
                page: Some(mut page),
            } => {
                if self.visit_phy_page(run, walk.cut, &mut page, pending)? {
                    match page.next {
                        Some(next) => PhyPhase::Initial {
                            after: Some(next),
                            page: None,
                        },
                        None => PhyPhase::Done,
                    }
                } else {
                    PhyPhase::Initial {
                        after,
                        page: Some(page),
                    }
                }
            }
            PhyPhase::Refresh {
                position,
                page: None,
            } => {
                let end = (position + AUDIT_WORK_PAGE_SIZE).min(walk.changes.len());
                let sources = walk.changes[position..end]
                    .iter()
                    .filter(|change| change.current_revision.is_some())
                    .map(|change| change.memory)
                    .collect();
                PhyPhase::Refresh {
                    position: end,
                    page: Some(Page::new(sources, None)),
                }
            }
            PhyPhase::Refresh {
                position,
                page: Some(mut page),
            } => {
                if self.visit_phy_page(run, walk.cut, &mut page, pending)? {
                    if position == walk.changes.len() {
                        PhyPhase::Done
                    } else {
                        PhyPhase::Refresh {
                            position,
                            page: None,
                        }
                    }
                } else {
                    PhyPhase::Refresh {
                        position,
                        page: Some(page),
                    }
                }
            }
        };
        pending.save_checkpoint(run, &walk).map_err(as_error)?;
        Ok(true)
    }

    fn visit_phy_page(
        &self,
        run: EgoRun,
        cut: PhyAuditCut,
        page: &mut Page,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        if usize::from(page.cursor.source) == page.sources.len() {
            return Ok(true);
        }
        let capture = self
            .instance
            .with_ego_phy_writer(self.phy, |phy| {
                capture_phy_discovery(
                    phy,
                    cut,
                    &page.sources,
                    page.cursor,
                    self.routes.identity,
                    limits(),
                )
            })
            .map_err(as_error)?;
        for ordinal in 0..capture.batch_count() {
            let key = EgoBatchKey {
                capture: capture.key,
                ordinal: ordinal as u32,
            };
            if pending
                .accepted_batch(run, key)
                .map_err(as_error)?
                .is_some()
            {
                continue;
            }
            let mut lookup = |request: LookupRequest<'_>| {
                self.still_authorized(self.phy)
                    .map_err(|_| LookupError::Invalidated)?;
                self.instance
                    .with_ego_phy_writer(self.phy, |phy| phy_audit_lookups(phy, cut, request))
                    .map_err(|_| LookupError::Invalidated)
            };
            let result = discover_phy(&capture, ordinal, self.routes.general.as_ref(), &mut lookup)
                .map_err(as_error)?;
            let mut candidates = accepted(result)?;
            self.still_authorized(self.phy).map_err(as_error)?;
            self.instance
                .with_ego_phy_writer(self.phy, |phy| {
                    for topic in &mut candidates {
                        topic
                            .sources
                            .retain(|source| phy_audit_citation(phy, cut, *source).is_ok());
                    }
                    candidates.retain(|topic| !topic.sources.is_empty());
                    Ok(())
                })
                .map_err(as_error)?;
            pending
                .accept_batch(run, key, &candidates)
                .map_err(as_error)?;
            return Ok(false);
        }
        if capture.next <= page.cursor {
            return Err("Ego PHY discovery cursor did not advance".into());
        }
        page.cursor = capture.next;
        Ok(usize::from(page.cursor.source) == page.sources.len())
    }
}

fn accepted(list: ListResult<EgoCandidate>) -> Result<Vec<EgoCandidate>, String> {
    match list {
        ListResult::Accepted { candidates, .. } => Ok(candidates),
        ListResult::Inactive => Ok(Vec::new()),
        ListResult::Incomplete(_) => Err("Ego discovery reply incomplete".into()),
    }
}
