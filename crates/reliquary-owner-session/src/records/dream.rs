//! Durable Dream first-cycle records. Population authority is supplied by the
//! owner after it proves the exact Memory physical prefix.
use self::dream_begin_codec::{BeginPopulation, BeginRecord};
use self::freshness_journal::{DreamFreshnessJournal, InitialDreamPass};
use crate::MemoryId;
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "dream_begin_codec.rs"]
pub mod dream_begin_codec;
#[path = "freshness_journal.rs"]
pub mod freshness_journal;

const JOURNAL_MAGIC: &[u8; 8] = b"CVDREAM1";
const BEGIN_MAGIC: &[u8; 8] = b"CVDREAM2";

#[derive(Clone, Debug, Eq, PartialEq)]
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

pub struct DreamEvidenceStore {
    owner_uuid: [u8; 16],
    admissions: BTreeMap<String, DreamBeginAdmission>,
    begins: BTreeMap<String, BeginRecord>,
    journal: DreamFreshnessJournal,
}

impl DreamEvidenceStore {
    pub fn open(
        container: &Container,
        admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, String> {
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
            journal: DreamFreshnessJournal::new(Some(owner_uuid)),
        };
        for frame in container.read_frames().map_err(|error| error.to_string())? {
            if frame.payload.starts_with(BEGIN_MAGIC) {
                let begin = dream_begin_codec::decode(&frame.payload)?;
                let admission = store
                    .admissions
                    .get(&begin.pass_id)
                    .ok_or("Dream Begin lacks authenticated population admission")?;
                store.validate_begin(&begin, admission)?;
                let pass = materialize(&begin, admission);
                store.journal.ingest_materialized_begin(owner_uuid, pass)?;
                if let Some(previous) = store.begins.get(&begin.pass_id) {
                    if previous != &begin {
                        return Err("conflicting Dream Begin identity".into());
                    }
                } else {
                    store.begins.insert(begin.pass_id.clone(), begin);
                }
            } else if frame.payload.starts_with(JOURNAL_MAGIC) {
                store.journal.ingest(&frame.payload)?;
            }
        }
        Ok(store)
    }

    pub fn begin(&self, pass_id: &str) -> Option<&BeginRecord> {
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
        let admission = self
            .admissions
            .get(&begin.pass_id)
            .ok_or("Dream Begin lacks authenticated population admission")?;
        self.validate_begin(&begin, admission)?;
        let pass = materialize(&begin, admission);
        if self.journal.begin_is_retry(&pass)? {
            return Ok(false);
        }
        let bytes = dream_begin_codec::encode(&begin)?;
        let mut staged = self.journal.clone();
        staged.ingest_materialized_begin(self.owner_uuid, pass)?;
        container
            .append(&bytes)
            .map_err(|error| error.to_string())?;
        container.sync().map_err(|error| error.to_string())?;
        self.journal = staged;
        self.begins.insert(begin.pass_id.clone(), begin);
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
        admission: &DreamBeginAdmission,
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
                    || *digest != population_digest(&admission.preexisting_ids)
                {
                    return Err("Dream compact population lacks exact prefix proof".into());
                }
            }
            BeginPopulation::ExplicitSet(ids) => {
                if ids != &admission.preexisting_ids {
                    return Err("Dream explicit population differs from admitted owner set".into());
                }
            }
        }
        Ok(())
    }
}

pub fn population_digest(ids: &BTreeSet<MemoryId>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary:dream-population:v1\0");
    hash.update((ids.len() as u64).to_le_bytes());
    for id in ids {
        hash.update(id.0);
    }
    hash.finalize().into()
}

fn materialize(begin: &BeginRecord, admission: &DreamBeginAdmission) -> InitialDreamPass {
    let preexisting_ids = admission.preexisting_ids.clone();
    InitialDreamPass {
        pass_id: begin.pass_id.clone(),
        source_id: begin.source_id,
        first_revision: 1,
        turn: begin.turn,
        accepted_settlement_turn: None,
        preexisting_ids,
        graph_version_before: begin.graph_version_before,
        roots: BTreeSet::new(),
    }
}
