//! Durable bounded scheduling over the active PHY's accepted owner cuts.

use reliquary_application::perception_workflow::ObservationProposal;
use reliquary_owner_session::{
    AUDIT_WORK_PAGE_SIZE, AuditDiscoveryCursor, MemoryId, OwnerSessionError, PerceptionSourceCut,
    PhyAuditCursor, PhyObservationCut, PhyWriter, model::MemoryTextField,
    observation::ObservationId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const CHANGE_CADENCE: usize = 5;
const OLDER_SAMPLES: usize = 20;
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
}

#[derive(Deserialize, Serialize)]
struct Bootstrap {
    cut: PhyObservationCut,
    after: Option<MemoryId>,
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
            bootstrap: Some(Bootstrap { cut, after: None }),
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
    pub(super) fn schedule(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
        if self.page.is_some() {
            return Err(OwnerSessionError::ManagementStale);
        }
        for offset in 0..4 {
            let lane = (self.lane + offset) % 4;
            let changed = match lane {
                0 => self.plan_bootstrap(phy)?,
                1 => self.plan_incoming(phy)?,
                2 => self.plan_routine(phy)?,
                _ => self.plan_structure(phy)?,
            };
            if changed {
                self.lane = (lane + 1) % 4;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn plan_bootstrap(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
        let Some(run) = &mut self.bootstrap else {
            return Ok(false);
        };
        let page = phy.phy_audit_initial_page(run.cut.audit, run.after)?;
        if !page.sources.is_empty() {
            self.page = Some(Page::new(
                PageKind::Bootstrap,
                run.cut,
                page.sources,
                Vec::new(),
            ));
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

    fn plan_routine(&mut self, phy: &PhyWriter) -> Result<bool, OwnerSessionError> {
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
            phy.phy_perception_changes_page(&self.structure, &through, AUDIT_WORK_PAGE_SIZE)?;
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
            let previous = phy.phy_audit_evidence(older.before.audit, id)?;
            let current = phy.phy_audit_evidence(older.through.audit, id)?;
            if let (Some((old, _)), Some((new, _))) = (previous, current)
                && old.revision == new.revision
            {
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
    if !sources.is_empty() {
        *output = Some(Page::new(kind, run.cut, sources, Vec::new()));
    }
    run.cursor = page.next;
    if run.cursor == run.cut.audit.into() {
        let cut = run.cut;
        *lane = None;
        Ok(Some(cut))
    } else {
        Ok(None)
    }
}
