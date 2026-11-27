//! Durable bounded scheduling over the active PHY's accepted owner cuts.

use crate::ego_audit_handoff::SharedPhyAudit;
use reliquary_application::{ego_pending::EgoRun, perception_workflow::ObservationProposal};
use reliquary_owner_session::{
    AuditDiscoveryCursor, MemoryId, OwnerSessionError, PerceptionSourceCut, PhyAuditCursor,
    PhyAuditCut, PhyObservationCut, PhyWriter, model::MemoryTextField, observation::ObservationId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const CHANGE_CADENCE: usize = 5;
const OLDER_SAMPLES: usize = 20;
// A native change record can name several affected Memories or claims.
const STRUCTURAL_PAGE_RECORDS: usize = 2;
const FORMAT: u8 = 1;

#[derive(Deserialize, Serialize)]
pub(super) struct Progress {
    format: u8,
    bootstrap: Option<Bootstrap>,
    covered: PhyObservationCut,
    incoming_covered: PhyObservationCut,
    structure: PerceptionSourceCut,
    count: Option<ChangeCount>,
    routine: Option<Changes>,
    incoming: Option<Changes>,
    samples: Option<Older>,
    sample_after: Option<MemoryId>,
    lane: u8,
    pub(super) page: Option<Page>,
    #[serde(default)]
    closed_ego: Option<[u8; 32]>,
}

#[derive(Deserialize, Serialize)]
struct Bootstrap {
    cut: PhyObservationCut,
    after: Option<MemoryId>,
    #[serde(default)]
    ego_route: Option<[u8; 32]>,
}

#[derive(Deserialize, Serialize)]
struct ChangeCount {
    cut: PhyObservationCut,
    cursor: PhyAuditCursor,
    distinct: BTreeSet<MemoryId>,
}

#[derive(Deserialize, Serialize)]
struct Changes {
    cut: PhyObservationCut,
    cursor: PhyAuditCursor,
    #[serde(default)]
    ego_route: Option<[u8; 32]>,
}

#[derive(Deserialize, Serialize)]
struct Older {
    before: PhyObservationCut,
    through: PhyObservationCut,
    remaining: usize,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum PageKind {
    Bootstrap,
    Routine,
    Structural,
    Incoming,
    Older,
}

impl PageKind {
    pub(super) fn discovery(self) -> bool {
        matches!(self, Self::Bootstrap | Self::Routine | Self::Structural)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) struct SharedPage {
    pub(super) window: SharedPhyAudit,
    pub(super) route: [u8; 32],
    last: bool,
}

#[derive(Deserialize, Serialize)]
pub(super) struct Page {
    pub(super) kind: PageKind,
    pub(super) cut: PhyObservationCut,
    pub(super) sources: Vec<MemoryId>,
    pub(super) derived: Vec<ObservationId>,
    pub(super) cursor: AuditDiscoveryCursor,
    pub(super) batch: u32,
    pub(super) capture: Option<[u8; 32]>,
    pub(super) proposals: Option<Vec<ObservationProposal>>,
    #[serde(default)]
    pub(super) shared: Option<SharedPage>,
    pub(super) contribution_cursor: u16,
    pub(super) target_after: Option<ObservationId>,
}

impl Page {
    fn new(
        kind: PageKind,
        cut: PhyObservationCut,
        sources: Vec<MemoryId>,
        derived: Vec<ObservationId>,
    ) -> Self {
        Self {
            kind,
            cut,
            sources,
            derived,
            cursor: AuditDiscoveryCursor {
                source: 0,
                field: MemoryTextField::Title,
                byte: 0,
            },
            batch: 0,
            capture: None,
            proposals: None,
            shared: None,
            contribution_cursor: 0,
            target_after: None,
        }
    }
}

impl Progress {
    pub(super) fn new(phy: &PhyWriter) -> Self {
        let cut = phy.phy_observation_cut();
        Self {
            format: FORMAT,
            bootstrap: Some(Bootstrap {
                cut,
                after: None,
                ego_route: None,
            }),
            covered: cut,
            incoming_covered: cut,
            structure: phy.perception_source_cut(),
            count: None,
            routine: None,
            incoming: None,
            samples: None,
            sample_after: None,
            lane: 0,
            page: None,
            closed_ego: None,
        }
    }

    pub(super) fn decode(bytes: &[u8], cut: PhyObservationCut) -> Result<Self, OwnerSessionError> {
        let value: Self = rmp_serde::from_slice(bytes)
            .map_err(|_| OwnerSessionError::Record("invalid PHY audit progress".into()))?;
        if value.format != FORMAT
            || value.lane >= 4
            || !value.covered.audit.is_at_or_before(cut.audit)
            || !value.incoming_covered.audit.is_at_or_before(cut.audit)
            || value
                .page
                .as_ref()
                .is_some_and(|page| !page.cut.audit.is_at_or_before(cut.audit))
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        Ok(value)
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, OwnerSessionError> {
        rmp_serde::to_vec_named(self).map_err(|_| OwnerSessionError::ManagementEvidence)
    }

    /// Plan one bounded change, without walking source bodies during idle polls.
    pub(super) fn schedule(
        &mut self,
        phy: &PhyWriter,
        ego_route: Option<[u8; 32]>,
    ) -> Result<bool, OwnerSessionError> {
        if self.page.is_some() {
            return Err(OwnerSessionError::ManagementStale);
        }
        for offset in 0..4 {
            let lane = (self.lane + offset) % 4;
            let changed = match lane {
                0 => self.plan_bootstrap(phy, ego_route)?,
                1 => self.plan_incoming(phy)?,
                2 => self.plan_routine(phy, ego_route)?,
                _ => self.plan_structure(phy)?,
            };
            if changed {
                self.lane = (lane + 1) % 4;
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn finish_page(&mut self, shared: Option<SharedPage>) {
        if let Some(shared) = shared.filter(|shared| shared.last) {
            self.closed_ego = Some(shared.window.run(shared.route).id());
        }
    }

    pub(super) fn closed_ego(&self, run: EgoRun) -> bool {
        self.closed_ego == Some(run.id())
    }

    pub(super) fn tracks_ego(&self, run: EgoRun) -> bool {
        let matches = |window: SharedPhyAudit, route| window.run(route).id() == run.id();
        self.page
            .as_ref()
            .and_then(|page| page.shared)
            .is_some_and(|page| matches(page.window, page.route))
            || self.bootstrap.as_ref().is_some_and(|state| {
                state.ego_route.is_some_and(|route| {
                    matches(
                        SharedPhyAudit {
                            cut: state.cut.audit,
                            prior: None,
                        },
                        route,
                    )
                })
            })
            || self.routine.as_ref().is_some_and(|state| {
                state.ego_route.is_some_and(|route| {
                    matches(
                        SharedPhyAudit {
                            cut: state.cut.audit,
                            prior: Some(self.covered.audit),
                        },
                        route,
                    )
                })
            })
    }

    pub(super) fn abandon_ego(&mut self, shared: SharedPage) {
        if let Some(state) = &mut self.bootstrap
            && state.cut.audit == shared.window.cut
        {
            state.ego_route = None;
        }
        if let Some(state) = &mut self.routine
            && state.cut.audit == shared.window.cut
        {
            state.ego_route = None;
        }
    }

    pub(super) fn expects_ego(&self, phy: &PhyWriter, route: [u8; 32]) -> bool {
        if let Some(shared) = self.page.as_ref().and_then(|page| page.shared)
            && shared.route == route
            && ego_matches(phy, shared.window.prior, shared.window.cut)
        {
            return true;
        }
        if let Some(run) = &self.bootstrap {
            return (run.after.is_none() || run.ego_route == Some(route))
                && ego_matches(phy, None, run.cut.audit);
        }
        if let Some(run) = &self.routine {
            return run.ego_route == Some(route)
                && ego_matches(phy, Some(self.covered.audit), run.cut.audit);
        }
        ego_matches(phy, Some(self.covered.audit), self.covered.audit)
    }

    fn plan_bootstrap(
        &mut self,
        phy: &PhyWriter,
        ego_route: Option<[u8; 32]>,
    ) -> Result<bool, OwnerSessionError> {
        let Some(run) = &mut self.bootstrap else {
            return Ok(false);
        };
        if run.after.is_none() {
            run.ego_route = ego_route.filter(|_| ego_matches(phy, None, run.cut.audit));
        }
        let page = phy.phy_audit_initial_page(run.cut.audit, run.after)?;
        if !page.sources.is_empty() || run.ego_route.is_some() {
            let mut work = Page::new(PageKind::Bootstrap, run.cut, page.sources, Vec::new());
            work.shared = run.ego_route.map(|route| SharedPage {
                window: SharedPhyAudit {
                    cut: run.cut.audit,
                    prior: None,
                },
                route,
                last: page.next.is_none(),
            });
            self.page = Some(work);
        }
        match page.next {
            Some(next) => run.after = Some(next),
            None => self.bootstrap = None,
        }
        Ok(true)
    }

    fn plan_incoming(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
        let cut = phy.phy_observation_cut();
        if self.incoming.is_none() {
            if same_memory(&cut, &self.incoming_covered) {
                return Ok(false);
            }
            self.incoming = Some(Changes {
                cut,
                cursor: self.incoming_covered.audit.into(),
                ego_route: None,
            });
        }
        if let Some(done) = read_changes(
            phy,
            PageKind::Incoming,
            self.incoming_covered,
            &mut self.incoming,
            &mut self.page,
        )? {
            self.incoming_covered = done;
        }
        Ok(true)
    }

    fn plan_routine(
        &mut self,
        phy: &PhyWriter,
        ego_route: Option<[u8; 32]>,
    ) -> Result<bool, OwnerSessionError> {
        if self.samples.is_some() {
            return self.plan_older(phy);
        }
        if self.routine.is_some() {
            if let Some(through) = read_changes(
                phy,
                PageKind::Routine,
                self.covered,
                &mut self.routine,
                &mut self.page,
            )? {
                self.samples = Some(Older {
                    before: self.covered,
                    through,
                    remaining: OLDER_SAMPLES,
                });
                self.covered = through;
            }
            return Ok(true);
        }
        let cut = phy.phy_observation_cut();
        if same_memory(&cut, &self.covered) {
            return Ok(false);
        }
        let count = self.count.get_or_insert_with(|| ChangeCount {
            cut,
            cursor: self.covered.audit.into(),
            distinct: BTreeSet::new(),
        });
        if count.cursor == count.cut.audit.into() {
            if same_memory(&cut, &count.cut) {
                return Ok(false);
            }
            count.cut = cut;
        }
        let page = phy.phy_audit_changes_page(self.covered.audit, count.cut.audit, count.cursor)?;
        count.cursor = page.next;
        for change in page.changes {
            count.distinct.insert(change.memory);
            if count.distinct.len() >= CHANGE_CADENCE {
                self.routine = Some(Changes {
                    cut: count.cut,
                    cursor: self.covered.audit.into(),
                    ego_route: ego_route
                        .filter(|_| ego_matches(phy, Some(self.covered.audit), count.cut.audit)),
                });
                self.count = None;
                break;
            }
        }
        Ok(true)
    }

    fn plan_structure(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
        let through = phy.perception_source_cut();
        if self.structure.graph == through.graph
            && self.structure.entity == through.entity
            && self.structure.observation == through.observation
        {
            return Ok(false);
        }
        let changes =
            phy.phy_perception_changes_page(&self.structure, &through, STRUCTURAL_PAGE_RECORDS)?;
        self.structure.graph = changes.graph_through;
        self.structure.entity = changes.entity_through;
        self.structure.observation = changes.observation_through;
        if !changes.memories.is_empty() || !changes.derived.is_empty() {
            self.page = Some(Page::new(
                PageKind::Structural,
                phy.phy_observation_cut(),
                changes.memories,
                changes.derived,
            ));
        }
        Ok(true)
    }

    fn plan_older(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
        let older = self
            .samples
            .as_mut()
            .ok_or(OwnerSessionError::ManagementStale)?;
        // A native page moves each rotation even when nothing in it is eligible.
        let page = phy.phy_audit_initial_page(older.through.audit, self.sample_after)?;
        let mut sources = Vec::new();
        let mut reached_limit = false;
        for id in page.sources {
            self.sample_after = Some(id);
            let previous = phy.phy_audit_revision(older.before.audit, id)?;
            let current = phy.phy_audit_revision(older.through.audit, id)?;
            if previous.is_some() && previous == current {
                sources.push(id);
                older.remaining -= 1;
                if older.remaining == 0 {
                    reached_limit = true;
                    break;
                }
            }
        }
        if !sources.is_empty() {
            self.page = Some(Page::new(
                PageKind::Older,
                older.through,
                sources,
                Vec::new(),
            ));
        }
        if reached_limit || page.next.is_none() {
            if page.next.is_none() && !reached_limit {
                self.sample_after = None;
            }
            self.samples = None;
        } else {
            self.sample_after = page.next;
        }
        Ok(true)
    }
}

fn same_memory(left: &PhyObservationCut, right: &PhyObservationCut) -> bool {
    left.audit.memory == right.audit.memory && left.audit.settlements == right.audit.settlements
}

fn read_changes(
    phy: &PhyWriter,
    kind: PageKind,
    prior: PhyObservationCut,
    lane: &mut Option<Changes>,
    output: &mut Option<Page>,
) -> Result<Option<PhyObservationCut>, OwnerSessionError> {
    let run = lane.as_mut().ok_or(OwnerSessionError::ManagementStale)?;
    let page = phy.phy_audit_changes_page(prior.audit, run.cut.audit, run.cursor)?;
    let sources = page
        .changes
        .into_iter()
        .filter_map(|change| change.current_revision.map(|_| change.memory))
        .collect::<Vec<_>>();
    run.cursor = page.next;
    let last = run.cursor == run.cut.audit.into();
    if !sources.is_empty() || (last && run.ego_route.is_some()) {
        let mut work = Page::new(kind, run.cut, sources, Vec::new());
        work.shared = run.ego_route.map(|route| SharedPage {
            window: SharedPhyAudit {
                cut: run.cut.audit,
                prior: Some(prior.audit),
            },
            route,
            last,
        });
        *output = Some(work);
    }
    if run.cursor == run.cut.audit.into() {
        let cut = run.cut;
        *lane = None;
        Ok(Some(cut))
    } else {
        Ok(None)
    }
}

pub(super) fn ego_matches(phy: &PhyWriter, prior: Option<PhyAuditCut>, cut: PhyAuditCut) -> bool {
    let completion = phy
        .ego_phy_summary()
        .map(|summary| summary.completion.cut)
        .filter(|previous| previous.history_generation == cut.history_generation);
    match (prior, completion) {
        (None, None) => true,
        (Some(prior), Some(previous)) => {
            prior.owner_id == previous.owner_id
                && prior.history_generation == previous.history_generation
                && prior.memory == previous.memory_version
                && prior.settlements == previous.audit_frontier
        }
        _ => false,
    }
}
