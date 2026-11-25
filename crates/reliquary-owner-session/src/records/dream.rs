//! Durable Dream first-cycle records. Population authority is supplied by the
//! owner after it proves the exact Memory physical prefix.
use self::dream_begin_codec::{BeginPopulation, BeginRecord};
use self::freshness_journal::{DreamFreshnessJournal, StoredDreamPass};
pub(crate) use self::population_history::{Population, PopulationHistory};
#[path = "dream_settlement.rs"]
mod settlement;
use super::dream_empty::InitialDreamSettlement;
use super::dream_frontier::FrontierRecord;
use crate::MemoryId;
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::{Arc, OnceLock};

#[path = "dream_begin_codec.rs"]
pub mod dream_begin_codec;
#[path = "freshness_journal.rs"]
pub mod freshness_journal;

const JOURNAL_MAGIC: &[u8; 8] = b"CVDREAM1";
const BEGIN_MAGIC: &[u8; 8] = b"CVDREAM2";

fn dream_evidence_frame(prefix: &[u8]) -> bool {
    // The evidence store admits raw carriers. Match decoder families where
    // an unsupported marker must still reach validation.
    prefix.starts_with(JOURNAL_MAGIC)
        || prefix.starts_with(BEGIN_MAGIC)
        || prefix.starts_with(b"RELDRC")
        || prefix.starts_with(b"RELDRF")
}

#[path = "dream_checkpoint.rs"]
mod checkpoint;
#[path = "dream_population_history.rs"]
mod population_history;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DreamBeginAdmission {
    pub owner_uuid: [u8; 16],
    pub pass_id: String,
    pub source_id: MemoryId,
    pub turn: u64,
    pub graph_version_before: u64,
    /// Active Memory identities at the exact admitted historical prefix.
    pub preexisting_ids: BTreeSet<MemoryId>,
    /// Physical cut is independently authenticated by the owner; its value is
    /// not inferred from active-set cardinality.
    pub memory_cut: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoredAdmission {
    owner_uuid: [u8; 16],
    pub(crate) pass_id: String,
    source_id: MemoryId,
    turn: u64,
    graph_version_before: u64,
    preexisting_ids: Population,
    pub(crate) memory_cut: u64,
}

impl From<DreamBeginAdmission> for StoredAdmission {
    fn from(admission: DreamBeginAdmission) -> Self {
        Self {
            owner_uuid: admission.owner_uuid,
            pass_id: admission.pass_id,
            source_id: admission.source_id,
            turn: admission.turn,
            graph_version_before: admission.graph_version_before,
            preexisting_ids: Population::from_set(admission.preexisting_ids),
            memory_cut: admission.memory_cut,
        }
    }
}

impl StoredAdmission {
    pub(crate) fn new(
        owner_uuid: [u8; 16],
        pass_id: String,
        source_id: MemoryId,
        turn: u64,
        graph_version_before: u64,
        memory_cut: u64,
        population: Population,
    ) -> Self {
        Self {
            owner_uuid,
            pass_id,
            source_id,
            turn,
            graph_version_before,
            memory_cut,
            preexisting_ids: population,
        }
    }

    pub(crate) fn with_population(
        begin: BeginRecord,
        memory_cut: u64,
        population: Population,
    ) -> Self {
        Self::new(
            begin.owner_uuid,
            begin.pass_id,
            begin.source_id,
            begin.turn,
            begin.graph_version_before,
            memory_cut,
            population,
        )
    }

    #[cfg(test)]
    pub(crate) fn population_is_materialized(&self) -> bool {
        self.preexisting_ids.is_materialized()
    }

    #[cfg(test)]
    pub(crate) fn shares_population_history(&self, other: &Self) -> bool {
        self.preexisting_ids.shares_history(&other.preexisting_ids)
    }

    pub(crate) fn to_admission(&self) -> DreamBeginAdmission {
        DreamBeginAdmission {
            owner_uuid: self.owner_uuid,
            pass_id: self.pass_id.clone(),
            source_id: self.source_id,
            turn: self.turn,
            graph_version_before: self.graph_version_before,
            preexisting_ids: self.preexisting_ids.to_set(),
            memory_cut: self.memory_cut,
        }
    }

    pub(crate) fn preexisting_contains(&self, id: &MemoryId) -> bool {
        self.preexisting_ids.contains(id)
    }

    pub(crate) fn preexisting_len(&self) -> usize {
        self.preexisting_ids.len()
    }

    pub(crate) fn preexisting_ids(&self) -> &BTreeSet<MemoryId> {
        self.preexisting_ids.as_set()
    }

    pub(crate) fn copy_preexisting_ids(&self) -> BTreeSet<MemoryId> {
        self.preexisting_ids.to_set()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
enum BeginKind {
    Explicit,
    Compact {
        digest: [u8; 32],
    },
    NativeBirth {
        first_memory_version: u64,
        digest: [u8; 32],
    },
    InferredExplicit {
        first_memory_version: u64,
    },
}

impl From<&BeginRecord> for BeginKind {
    fn from(begin: &BeginRecord) -> Self {
        match &begin.population {
            BeginPopulation::ExplicitSet(_) => Self::Explicit,
            BeginPopulation::CompactCut { digest, .. } => Self::Compact { digest: *digest },
            BeginPopulation::NativeBirth {
                first_memory_version,
                digest,
            } => Self::NativeBirth {
                first_memory_version: *first_memory_version,
                digest: *digest,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredBegin {
    pub(crate) source_id: MemoryId,
    pub(crate) turn: u64,
    pub(crate) graph_version_before: u64,
    kind: BeginKind,
    view: OnceLock<Arc<BeginRecord>>,
}

impl StoredBegin {
    fn new(admission: &StoredAdmission, kind: BeginKind) -> Self {
        Self {
            source_id: admission.source_id,
            turn: admission.turn,
            graph_version_before: admission.graph_version_before,
            kind,
            view: OnceLock::new(),
        }
    }

    pub(crate) fn is_compact(&self) -> bool {
        matches!(self.kind, BeginKind::Compact { .. })
    }

    pub(crate) fn native_birth(&self) -> Option<u64> {
        match self.kind {
            BeginKind::NativeBirth {
                first_memory_version,
                ..
            } => Some(first_memory_version),
            _ => None,
        }
    }

    pub(crate) fn inferred_explicit_birth(&self) -> Option<u64> {
        match self.kind {
            BeginKind::InferredExplicit {
                first_memory_version,
            } => Some(first_memory_version),
            _ => None,
        }
    }

    fn view(&self, admission: &StoredAdmission) -> &BeginRecord {
        self.view.get_or_init(|| {
            Arc::new(BeginRecord {
                owner_uuid: admission.owner_uuid,
                pass_id: admission.pass_id.clone(),
                source_id: self.source_id,
                turn: self.turn,
                graph_version_before: self.graph_version_before,
                population: match self.kind {
                    BeginKind::Explicit | BeginKind::InferredExplicit { .. } => {
                        BeginPopulation::ExplicitSet(admission.preexisting_ids.to_set())
                    }
                    BeginKind::Compact { digest } => BeginPopulation::CompactCut {
                        memory_cut: admission.memory_cut,
                        digest,
                    },
                    BeginKind::NativeBirth {
                        first_memory_version,
                        digest,
                    } => BeginPopulation::NativeBirth {
                        first_memory_version,
                        digest,
                    },
                },
            })
        })
    }
}

#[derive(Clone)]
pub struct DreamEvidenceStore {
    owner_uuid: [u8; 16],
    admissions: BTreeMap<String, StoredAdmission>,
    begins: BTreeMap<String, StoredBegin>,
    begun_sources: BTreeMap<MemoryId, String>,
    journal: DreamFreshnessJournal,
    initial_settlements: BTreeMap<String, InitialDreamSettlement>,
    settled_sources: Vec<MemoryId>,
    settlement_positions: BTreeMap<MemoryId, u64>,
    frontiers: BTreeMap<String, frontier::StoredFrontier>,
}

/// A raw explicit Frontier whose embedded population was matched against
/// the exact physical birth cut. The inferred admission owns that population.
pub(crate) struct InferredExplicitFrontier {
    pub position: ObjectRef,
    pub frontier: FrontierRecord,
    pub turn: u64,
    pub first_memory_version: u64,
}

impl DreamEvidenceStore {
    pub fn open(
        container: &Container,
        admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, String> {
        Self::open_admitted(
            container,
            admissions.into_iter().map(StoredAdmission::from),
            Vec::new(),
        )
        .map(|(store, _)| store)
    }

    pub(crate) fn open_admitted(
        container: &Container,
        admissions: impl IntoIterator<Item = StoredAdmission>,
        explicit_frontiers: Vec<InferredExplicitFrontier>,
    ) -> Result<(Self, Vec<(ObjectRef, String)>), String> {
        let owner_uuid = container.owner_id();
        let mut admitted = BTreeMap::new();
        for admission in admissions {
            if admission.owner_uuid != owner_uuid || admission.pass_id.trim().is_empty() {
                return Err("Dream admission owner or pass identity mismatch".into());
            }
            if admitted
                .insert(admission.pass_id.clone(), admission)
                .is_some()
            {
                return Err("duplicate Dream Begin admission".into());
            }
        }
        let mut store = Self {
            owner_uuid,
            admissions: admitted,
            begins: BTreeMap::new(),
            begun_sources: BTreeMap::new(),
            journal: DreamFreshnessJournal::new(Some(owner_uuid)),
            initial_settlements: BTreeMap::new(),
            settled_sources: Vec::new(),
            settlement_positions: BTreeMap::new(),
            frontiers: BTreeMap::new(),
        };
        let mut frontier_positions = Vec::new();
        let mut explicit_frontiers = explicit_frontiers.into_iter().peekable();
        let mut frames = container
            .matching_physical_cursor(dream_evidence_frame)
            .map_err(|error| error.to_string())?;
        let mut buffer = Vec::new();
        while let Some(mut frame) = frames.next_frame().map_err(|error| error.to_string())? {
            let object = frame.object();
            if explicit_frontiers
                .peek()
                .is_some_and(|entry| entry.position.offset < object.offset)
            {
                return Err("inferred Dream Frontier position mismatch".into());
            }
            if let Some(entry) =
                explicit_frontiers.next_if(|entry| entry.position.offset == object.offset)
            {
                let mut magic = [0; 8];
                frame
                    .read_exact(&mut magic)
                    .map_err(|error| error.to_string())?;
                if entry.position != object || &magic != b"RELDRC01" {
                    return Err("inferred Dream Frontier position mismatch".into());
                }
                let pass_id = entry.frontier.pass_id.clone();
                store.ingest_inferred_explicit_frontier(entry)?;
                frontier_positions.push((object, pass_id));
                continue;
            }
            buffer.clear();
            frame
                .read_to_end(&mut buffer)
                .map_err(|error| error.to_string())?;
            let payload = buffer.as_slice();
            if let Some((frontier, begin)) =
                super::dream_frontier::FrontierRecord::decode_with_begin(payload)?
            {
                if let Some(begin) = begin {
                    store.ingest_physical_begin(begin)?;
                }
                let pass_id = frontier.pass_id.clone();
                store.apply_frontier(frontier)?;
                frontier_positions.push((object, pass_id));
            } else if payload.starts_with(BEGIN_MAGIC) {
                let begin = dream_begin_codec::decode(payload)?;
                if matches!(begin.population, BeginPopulation::NativeBirth { .. }) {
                    return Err("Dream native birth requires a bound frontier".into());
                }
                store.ingest_physical_begin(begin)?;
            } else if payload.starts_with(JOURNAL_MAGIC) {
                store.journal.ingest(payload)?;
            } else if let Some(settlement) = InitialDreamSettlement::decode(payload)? {
                if container.identity().file_kind == reliquary_container::FileKind::Phylactery
                    && !settlement.freshness_effects.is_empty()
                {
                    return Err("PHY Dream cannot carry REL Freshness effects".into());
                }
                store.apply_initial_settlement(settlement)?;
            }
        }
        if explicit_frontiers.next().is_some() {
            return Err("inferred Dream Frontier position mismatch".into());
        }
        Ok((store, frontier_positions))
    }

    pub fn begin(&self, pass_id: &str) -> Option<&BeginRecord> {
        Some(
            self.begins
                .get(pass_id)?
                .view(self.admissions.get(pass_id)?),
        )
    }

    pub(crate) fn begun(&self, pass_id: &str) -> Option<&StoredBegin> {
        self.begins.get(pass_id)
    }
    pub fn journal(&self) -> &DreamFreshnessJournal {
        &self.journal
    }
    pub fn owner_uuid(&self) -> [u8; 16] {
        self.owner_uuid
    }

    pub fn append_begin(
        &mut self,
        container: &mut Container,
        begin: BeginRecord,
    ) -> Result<bool, String> {
        self.check_owner(container)?;
        if matches!(begin.population, BeginPopulation::NativeBirth { .. }) {
            return Err("Dream native birth requires a bound frontier".into());
        }
        if self.has_other_initial_pass(begin.source_id, &begin.pass_id) {
            return Err("Memory already has a first-cycle Dream Begin".into());
        }
        let admission = self
            .admissions
            .get(&begin.pass_id)
            .ok_or("Dream Begin lacks authenticated population admission")?;
        self.validate_begin(&begin, admission)?;
        let pass = pending_pass(admission);
        if self.journal.begin_is_admitted_retry(&pass)? {
            return Ok(false);
        }
        let stored = StoredBegin::new(admission, (&begin).into());
        let bytes = dream_begin_codec::encode(&begin)?;
        let mut staged = self.journal.clone();
        staged.ingest_admitted_begin(self.owner_uuid, pass)?;
        container
            .append(&bytes)
            .map_err(|error| error.to_string())?;
        container.sync().map_err(|error| error.to_string())?;
        self.journal = staged;
        self.install_begin(begin.pass_id, stored);
        Ok(true)
    }

    /// Replays one caller-generated CVDREAM1 journal event after checking the
    /// existing owner and transition rules. The store never invents an event.
    pub fn append_journal(
        &mut self,
        container: &mut Container,
        payload: &[u8],
    ) -> Result<(), String> {
        self.check_owner(container)?;
        if !payload.starts_with(JOURNAL_MAGIC) {
            return Err("not a Dream first-cycle journal record".into());
        }
        let mut staged = self.journal.clone();
        staged.ingest(payload)?;
        container
            .append(payload)
            .map_err(|error| error.to_string())?;
        container.sync().map_err(|error| error.to_string())?;
        self.journal = staged;
        Ok(())
    }

    fn ingest_physical_begin(&mut self, begin: BeginRecord) -> Result<(), String> {
        let admission = self
            .admissions
            .get(&begin.pass_id)
            .ok_or("Dream Begin lacks authenticated population admission")?;
        self.validate_begin(&begin, admission)?;
        let kind = (&begin).into();
        self.ingest_admitted_physical_begin(begin.pass_id, kind)
    }

    fn ingest_inferred_explicit_frontier(
        &mut self,
        entry: InferredExplicitFrontier,
    ) -> Result<(), String> {
        let frontier = entry.frontier;
        let admission = self
            .admissions
            .get(&frontier.pass_id)
            .ok_or("Dream Begin lacks authenticated population admission")?;
        // The prefix validator supplied the exact explicit set in this admission.
        if frontier.owner != self.owner_uuid
            || admission.source_id != MemoryId(frontier.source.id)
            || admission.turn != entry.turn
            || admission.graph_version_before != frontier.graph_version
            || admission.memory_cut != frontier.memory_version
            || admission.preexisting_ids.contains(&admission.source_id)
        {
            return Err("Dream Begin does not match authenticated owner evidence".into());
        }
        self.ingest_admitted_physical_begin(
            frontier.pass_id.clone(),
            BeginKind::InferredExplicit {
                first_memory_version: entry.first_memory_version,
            },
        )?;
        self.apply_frontier_with_begin(frontier, true)
    }

    fn ingest_admitted_physical_begin(
        &mut self,
        pass_id: String,
        kind: BeginKind,
    ) -> Result<(), String> {
        let admission = self
            .admissions
            .get(&pass_id)
            .ok_or("Dream Begin lacks authenticated population admission")?;
        if self.begins.contains_key(&pass_id)
            || self.begun_sources.contains_key(&admission.source_id)
        {
            return Err("duplicate physical first-cycle Dream Begin".into());
        }
        let stored = StoredBegin::new(admission, kind);
        self.journal
            .ingest_admitted_begin(self.owner_uuid, pending_pass(admission))?;
        self.install_begin(pass_id, stored);
        Ok(())
    }

    fn install_begin(&mut self, pass_id: String, begin: StoredBegin) {
        self.begun_sources.insert(begin.source_id, pass_id.clone());
        self.begins.insert(pass_id, begin);
    }

    pub(crate) fn replay_initial_frontier(
        &mut self,
        record: super::dream_frontier::FrontierRecord,
        begin: BeginRecord,
        admission: impl Into<StoredAdmission>,
    ) -> Result<(), String> {
        let admission = admission.into();
        if self.admissions.contains_key(&admission.pass_id) {
            return Err("duplicate Dream Begin admission".into());
        }
        self.admissions.insert(admission.pass_id.clone(), admission);
        self.ingest_physical_begin(begin)?;
        self.apply_frontier(record)
    }

    fn check_owner(&self, container: &Container) -> Result<(), String> {
        if container.owner_id() == self.owner_uuid {
            Ok(())
        } else {
            Err("Dream first-cycle owner mismatch".into())
        }
    }

    fn validate_begin(
        &self,
        begin: &BeginRecord,
        admission: &StoredAdmission,
    ) -> Result<(), String> {
        if begin.owner_uuid != self.owner_uuid
            || admission.owner_uuid != self.owner_uuid
            || begin.pass_id != admission.pass_id
            || begin.source_id != admission.source_id
            || begin.turn != admission.turn
            || begin.graph_version_before != admission.graph_version_before
            || admission.preexisting_ids.contains(&begin.source_id)
        {
            return Err("Dream Begin does not match authenticated owner evidence".into());
        }
        match &begin.population {
            BeginPopulation::CompactCut { memory_cut, digest } => {
                if *memory_cut != admission.memory_cut
                    || *digest != admission.preexisting_ids.digest()
                {
                    return Err("Dream compact population lacks exact prefix proof".into());
                }
            }
            BeginPopulation::NativeBirth {
                first_memory_version,
                digest,
            } => {
                if *first_memory_version == 0
                    || *first_memory_version > admission.memory_cut
                    || *digest != admission.preexisting_ids.digest()
                {
                    return Err("Dream native birth population lacks exact birth proof".into());
                }
            }
            BeginPopulation::ExplicitSet(ids) => {
                if !admission.preexisting_ids.matches_set(ids) {
                    return Err("Dream explicit population differs from admitted owner set".into());
                }
            }
        }
        Ok(())
    }
}

pub fn population_digest(ids: &BTreeSet<MemoryId>) -> [u8; 32] {
    population_digest_iter(ids.len(), ids)
}

fn population_digest_iter<'a>(len: usize, ids: impl IntoIterator<Item = &'a MemoryId>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:dream-population:v1\0");
    hash.update((len as u64).to_le_bytes());
    for id in ids {
        hash.update(id.0);
    }
    hash.finalize().into()
}

fn pending_pass(admission: &StoredAdmission) -> StoredDreamPass {
    StoredDreamPass {
        pass_id: admission.pass_id.clone(),
        source_id: admission.source_id,
        first_revision: 1,
        turn: admission.turn,
        accepted_settlement_turn: None,
        preexisting_ids: admission.preexisting_ids.clone(),
        graph_version_before: admission.graph_version_before,
        roots: BTreeSet::new(),
    }
}

#[path = "dream_frontier_store.rs"]
mod frontier;
