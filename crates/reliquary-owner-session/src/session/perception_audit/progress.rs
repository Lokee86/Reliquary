//! Source frontier and sampling progress, independent of provider replies.
use super::*;

pub const AUDIT_CADENCE: usize = 25;
pub const AUDIT_STALE_SAMPLES: usize = 15;
pub const AUDIT_DORMANT_SAMPLES: usize = 5;
pub const AUDIT_WORK_PAGE_SIZE: usize = 125;
const MAX_PROGRESS_BYTES: usize = 32 * 1024;
const PROGRESS_VERSION: u8 = 1;
// A Memory change contributes at most one source, a Graph change at most two
// and an Entity change up to MAX_NEIGHBORS. Reserve room for the cadence group
// and both sample bands before moving any of the three change cursors.
const CHANGES_PER_STREAM: usize =
    (AUDIT_WORK_PAGE_SIZE - AUDIT_CADENCE - AUDIT_STALE_SAMPLES - AUDIT_DORMANT_SAMPLES)
        / (1 + 2 + MAX_NEIGHBORS);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditWorkSource {
    pub memory: MemoryId,
    pub sampled: bool,
    /// First settlements can contribute local Freshness credit when routed.
    pub newly_settled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionAuditPage {
    pub cut: PerceptionSourceCut,
    pub sources: Vec<AuditWorkSource>,
    pub derived: Vec<ObservationId>,
    pub discovery: AuditDiscoveryCursor,
    /// Discovery cursor through deterministic request batches in this page.
    pub batch: u32,
    pub capture_key: Option<[u8; 32]>,
    pub ego_accepted: bool,
    pub perception_accepted: bool,
    /// Contribution routes independently of accepted discovery output.
    pub contribution_cursor: u16,
    pub contribution_target_after: Option<ObservationId>,
}

impl PerceptionAuditPage {
    pub fn bind_capture(&mut self, key: [u8; 32]) {
        if self.capture_key != Some(key) {
            self.capture_key = Some(key);
            self.batch = 0;
            self.ego_accepted = false;
            self.perception_accepted = false;
        }
    }

    pub fn finish_batch(&mut self) -> Result<(), OwnerSessionError> {
        self.batch = self.batch.checked_add(1).ok_or_else(invalid_progress)?;
        self.ego_accepted = false;
        self.perception_accepted = false;
        Ok(())
    }

    pub fn finish_capture(&mut self, next: AuditDiscoveryCursor) -> Result<(), OwnerSessionError> {
        if next <= self.discovery
            || usize::from(next.source) > self.sources.len() + self.derived.len()
        {
            return Err(invalid_progress());
        }
        self.discovery = next;
        self.capture_key = None;
        self.batch = 0;
        self.ego_accepted = false;
        self.perception_accepted = false;
        Ok(())
    }
}

/// Seek through mandatory pieces without hydrating a whole page of large bodies.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditDiscoveryCursor {
    pub source: u16,
    pub field: MemoryTextField,
    pub byte: u32,
}

/// Advance asks the caller to persist a local change, act on a capture page,
/// or sleep until owner state changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PerceptionAuditAdvance {
    Updated,
    Ready,
    Idle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionAuditProgress {
    version: u8,
    bootstrap_cut: Option<PerceptionSourceCut>,
    bootstrap_after: Option<MemoryId>,
    after: PerceptionSourceCut,
    settled_after: u64,
    due: Vec<MemoryId>,
    stale_after: Option<MemoryId>,
    dormant_after: Option<MemoryId>,
    run: Option<AuditRun>,
    pub page: Option<PerceptionAuditPage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditRun {
    cut: PerceptionSourceCut,
    due: Vec<MemoryId>,
    stale: SampleRotation,
    dormant: SampleRotation,
    placed_inputs: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleRotation {
    start: Option<MemoryId>,
    after: Option<MemoryId>,
    wrapped: bool,
    finished: bool,
    selected: Vec<MemoryId>,
}

impl SampleRotation {
    fn new(start: Option<MemoryId>) -> Self {
        Self {
            start,
            after: start,
            wrapped: start.is_none(),
            finished: false,
            selected: Vec::new(),
        }
    }

    fn advance(
        &mut self,
        rel: &RelSession,
        cut: &PerceptionSourceCut,
        due: &[MemoryId],
        sibling: &[MemoryId],
        grade: PerceptionSourceGrade,
        quota: usize,
    ) -> Result<(), OwnerSessionError> {
        let mut ids = rel.perception_source_ids_page(self.after, MAX_CAPTURE_PAGE);
        let end_of_order = ids.len() < MAX_CAPTURE_PAGE;
        let mut passed_start = false;
        if self.wrapped
            && let Some(start) = self.start
        {
            passed_start = ids.last().is_some_and(|id| *id >= start);
            ids.retain(|id| *id <= start);
        }
        let mut matches = rel
            .perception_source_metadata(cut, &ids)?
            .into_iter()
            .filter(|row| row.grade == grade)
            .map(|row| row.source.id)
            .peekable();
        for id in ids {
            self.after = Some(id);
            if matches.peek() == Some(&id) {
                matches.next();
                if !due.contains(&id) && !sibling.contains(&id) {
                    self.selected.push(id);
                    if self.selected.len() == quota {
                        self.finished = true;
                        return Ok(());
                    }
                }
            }
        }
        if end_of_order || passed_start {
            if self.wrapped {
                self.finished = true;
            } else {
                self.wrapped = true;
                self.after = None;
            }
        }
        Ok(())
    }
}

impl PerceptionAuditProgress {
    /// The first configured, managed mount establishes the boundary between
    /// retained bootstrap evidence and new cadence credits.
    pub fn new(cut: PerceptionSourceCut) -> Self {
        let settled_after = cut.settlements;
        Self {
            version: PROGRESS_VERSION,
            bootstrap_cut: (cut.memory != 0).then(|| cut.clone()),
            bootstrap_after: None,
            after: cut,
            settled_after,
            due: Vec::new(),
            stale_after: None,
            dormant_after: None,
            run: None,
            page: None,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, OwnerSessionError> {
        self.validate()?;
        let bytes = rmp_serde::to_vec_named(self).map_err(|_| invalid_progress())?;
        if bytes.len() > MAX_PROGRESS_BYTES {
            return Err(invalid_progress());
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, OwnerSessionError> {
        if bytes.len() > MAX_PROGRESS_BYTES {
            return Err(invalid_progress());
        }
        let progress: Self = rmp_serde::from_slice(bytes).map_err(|_| invalid_progress())?;
        if progress.encode()? != bytes {
            return Err(invalid_progress());
        }
        Ok(progress)
    }

    pub(in crate::session) fn verify_prefix(
        bytes: &[u8],
        owner: [u8; 16],
        generation: [u8; 32],
        memory: u64,
        entity: u64,
        observation: u64,
        rel_turn: u64,
    ) -> Result<(), OwnerSessionError> {
        let parsed = Self::decode(bytes)?;
        let cuts = std::iter::once(&parsed.after)
            .chain(&parsed.bootstrap_cut)
            .chain(parsed.run.as_ref().map(|run| &run.cut))
            .chain(parsed.page.as_ref().map(|page| &page.cut));
        for cut in cuts {
            if cut.owner_id != owner
                || cut.history_generation != generation
                || cut.memory > memory
                || cut.entity > entity
                || cut.observation > observation
                || cut.rel_turn > rel_turn
            {
                return Err(invalid_progress());
            }
        }
        Ok(())
    }

    pub fn advance(
        &mut self,
        rel: &RelSession,
    ) -> Result<PerceptionAuditAdvance, OwnerSessionError> {
        if self.page.is_some() {
            return Ok(PerceptionAuditAdvance::Ready);
        }
        if let Some(cut) = self.bootstrap_cut.clone() {
            self.advance_bootstrap(rel, cut)?;
            return Ok(PerceptionAuditAdvance::Updated);
        }
        if let Some(mut run) = self.run.take() {
            self.advance_run(rel, &mut run)?;
            if self.after != run.cut || self.page.is_some() {
                self.run = Some(run);
            }
            return Ok(PerceptionAuditAdvance::Updated);
        }
        self.advance_cadence(rel)
    }

    pub fn finish_page(&mut self) {
        self.page = None;
    }

    fn advance_bootstrap(
        &mut self,
        rel: &RelSession,
        cut: PerceptionSourceCut,
    ) -> Result<(), OwnerSessionError> {
        let ids = rel.perception_source_ids_page(self.bootstrap_after, AUDIT_WORK_PAGE_SIZE);
        let rows = rel.perception_source_metadata(&cut, &ids)?;
        if rows.is_empty() && ids.len() == AUDIT_WORK_PAGE_SIZE {
            self.bootstrap_after = ids.last().copied();
            return Ok(());
        }
        if !rows.is_empty() {
            self.page = Some(page(
                cut,
                rows.into_iter()
                    .map(|row| AuditWorkSource {
                        memory: row.source.id,
                        sampled: false,
                        newly_settled: false,
                    })
                    .collect(),
                Vec::new(),
            ));
        }
        self.bootstrap_cut = None;
        self.bootstrap_after = None;
        Ok(())
    }

    fn advance_cadence(
        &mut self,
        rel: &RelSession,
    ) -> Result<PerceptionAuditAdvance, OwnerSessionError> {
        let mut cut = rel.perception_source_cut();
        rel.check_perception_source_cut(&self.after)?;
        let settled =
            rel.perception_settlements_page(self.settled_after, cut.settlements, MAX_CAPTURE_PAGE);
        if settled.is_empty() {
            return Ok(PerceptionAuditAdvance::Idle);
        }
        let ids = settled.iter().map(|&(_, id)| id).collect::<Vec<_>>();
        let ready = rel.perception_source_metadata(&cut, &ids)?;
        let ready = ready
            .iter()
            .map(|row| row.source.id)
            .collect::<BTreeSet<_>>();
        for (position, id) in settled {
            self.settled_after = position;
            if ready.contains(&id) {
                self.due.push(id);
            }
            if self.due.len() == AUDIT_CADENCE {
                cut.settlements = position;
                self.run = Some(AuditRun {
                    cut,
                    due: std::mem::take(&mut self.due),
                    stale: SampleRotation::new(self.stale_after),
                    dormant: SampleRotation::new(self.dormant_after),
                    placed_inputs: false,
                });
                break;
            }
        }
        Ok(PerceptionAuditAdvance::Updated)
    }

    fn advance_run(
        &mut self,
        rel: &RelSession,
        run: &mut AuditRun,
    ) -> Result<(), OwnerSessionError> {
        if !run.stale.finished {
            return run.stale.advance(
                rel,
                &run.cut,
                &run.due,
                &run.dormant.selected,
                PerceptionSourceGrade::Stale,
                AUDIT_STALE_SAMPLES,
            );
        }
        if !run.dormant.finished {
            return run.dormant.advance(
                rel,
                &run.cut,
                &run.due,
                &run.stale.selected,
                PerceptionSourceGrade::Dormant,
                AUDIT_DORMANT_SAMPLES,
            );
        }
        let changes = rel.perception_source_changes(&self.after, &run.cut, CHANGES_PER_STREAM)?;
        let mut sources = Vec::with_capacity(AUDIT_WORK_PAGE_SIZE);
        let mut seen = run
            .due
            .iter()
            .chain(&run.stale.selected)
            .chain(&run.dormant.selected)
            .copied()
            .collect::<BTreeSet<_>>();
        if !run.placed_inputs {
            sources.extend(run.due.iter().map(|&memory| AuditWorkSource {
                memory,
                sampled: false,
                newly_settled: true,
            }));
            sources.extend(
                run.stale
                    .selected
                    .iter()
                    .chain(&run.dormant.selected)
                    .map(|&memory| AuditWorkSource {
                        memory,
                        sampled: true,
                        newly_settled: false,
                    }),
            );
            run.placed_inputs = true;
            self.stale_after = run.stale.after.or(run.stale.start);
            self.dormant_after = run.dormant.after.or(run.dormant.start);
        }
        sources.extend(
            changes
                .memories
                .into_iter()
                .filter(|id| seen.insert(*id))
                .map(|memory| AuditWorkSource {
                    memory,
                    sampled: false,
                    newly_settled: false,
                }),
        );
        self.after = PerceptionSourceCut {
            memory: changes.memory_through,
            graph: changes.graph_through,
            entity: changes.entity_through,
            observation: changes.observation_through,
            ..run.cut.clone()
        };
        if !sources.is_empty() || !changes.derived.is_empty() {
            self.page = Some(page(run.cut.clone(), sources, changes.derived));
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), OwnerSessionError> {
        let valid_cut = |cut: &PerceptionSourceCut| {
            cut.owner_id == self.after.owner_id
                && cut.history_generation == self.after.history_generation
        };
        if self.version != PROGRESS_VERSION
            || self.due.len() >= AUDIT_CADENCE
            || self
                .bootstrap_cut
                .as_ref()
                .is_some_and(|cut| !valid_cut(cut))
            || self.run.as_ref().is_some_and(|run| {
                !valid_cut(&run.cut)
                    || run.due.len() != AUDIT_CADENCE
                    || run.stale.selected.len() > AUDIT_STALE_SAMPLES
                    || run.dormant.selected.len() > AUDIT_DORMANT_SAMPLES
            })
            || self.page.as_ref().is_some_and(|page| {
                !valid_cut(&page.cut)
                    || (page.sources.is_empty() && page.derived.is_empty())
                    || page.sources.len() > AUDIT_WORK_PAGE_SIZE
                    || page.derived.len() > AUDIT_WORK_PAGE_SIZE
                    || usize::from(page.discovery.source) > page.sources.len() + page.derived.len()
                    || usize::from(page.contribution_cursor) > page.sources.len()
                    || page
                        .sources
                        .iter()
                        .map(|source| source.memory)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != page.sources.len()
            })
        {
            return Err(invalid_progress());
        }
        Ok(())
    }
}

fn page(
    cut: PerceptionSourceCut,
    sources: Vec<AuditWorkSource>,
    derived: Vec<ObservationId>,
) -> PerceptionAuditPage {
    PerceptionAuditPage {
        cut,
        sources,
        derived,
        discovery: AuditDiscoveryCursor {
            source: 0,
            field: MemoryTextField::Title,
            byte: 0,
        },
        batch: 0,
        capture_key: None,
        ego_accepted: false,
        perception_accepted: false,
        contribution_cursor: 0,
        contribution_target_after: None,
    }
}

fn invalid_progress() -> OwnerSessionError {
    OwnerSessionError::Record("invalid Perception audit progress".into())
}
