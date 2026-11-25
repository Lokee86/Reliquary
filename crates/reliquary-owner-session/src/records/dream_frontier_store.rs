use super::*;
use crate::records::dream_frontier::{FrontierRecord, InitialDreamFrontier};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct StoredFrontier {
    source: crate::records::dream_batch::Observation,
    candidates: Vec<crate::records::dream_batch::Observation>,
    graph_version: u64,
    memory_version: u64,
    has_begin: bool,
}

impl From<FrontierRecord> for StoredFrontier {
    fn from(record: FrontierRecord) -> Self {
        Self {
            source: record.source,
            candidates: record.candidates,
            graph_version: record.graph_version,
            memory_version: record.memory_version,
            has_begin: record.begin.is_some(),
        }
    }
}

impl DreamEvidenceStore {
    pub(super) fn apply_frontier(&mut self, record: FrontierRecord) -> Result<(), String> {
        let begin = self
            .begins
            .get(&record.pass_id)
            .ok_or("frontier lacks Begin")?;
        self.validate_frontier(&record, begin.source_id, &self.journal)?;
        self.frontiers.insert(record.pass_id.clone(), record.into());
        Ok(())
    }

    fn validate_frontier(
        &self,
        record: &FrontierRecord,
        source_id: MemoryId,
        journal: &DreamFreshnessJournal,
    ) -> Result<(), String> {
        if record.owner != self.owner_uuid
            || source_id != record.source.cut().id
            || self.frontiers.contains_key(&record.pass_id)
            || journal.pending_ref(&record.pass_id).is_none()
        {
            return Err("invalid or duplicate Dream frontier".into());
        }
        Ok(())
    }

    pub fn initial_frontier(&self, pass: &str) -> Option<InitialDreamFrontier> {
        self.frontiers
            .get(pass)
            .map(|frontier| InitialDreamFrontier {
                source: frontier.source.cut(),
                candidates: frontier.candidates.iter().map(|o| o.cut()).collect(),
                graph_version: frontier.graph_version,
            })
    }
    pub(crate) fn frontier_has_bound_begin(&self, pass: &str) -> bool {
        self.frontiers.get(pass).is_some_and(|frontier| {
            frontier.has_begin
                && self.admissions.get(pass).is_some_and(|admission| {
                    frontier.memory_version == admission.memory_cut
                        && frontier.graph_version == admission.graph_version_before
                })
        })
    }

    pub(crate) fn has_other_pending_frontier(&self, prefix: &str, pass: &str) -> bool {
        self.journal
            .pending_pass_ids()
            .any(|id| id != pass && id.starts_with(prefix) && self.frontiers.contains_key(id))
    }

    pub(crate) fn append_frontier(
        &mut self,
        container: &mut Container,
        admission: DreamBeginAdmission,
        source: crate::DreamMemoryCut,
        candidates: Vec<crate::DreamMemoryCut>,
        graph_version: u64,
        memory_version: u64,
    ) -> Result<DreamBeginAdmission, String> {
        self.check_owner(container)?;
        if let Some(old) = self.frontiers.get(&admission.pass_id) {
            return if old.source.cut() == source
                && old.candidates.iter().map(|o| o.cut()).collect::<Vec<_>>() == candidates
            {
                Ok(self.admissions[&admission.pass_id].to_admission())
            } else {
                Err("frontier identity changed".into())
            };
        }
        let (new_begin, begin_payload) = if self.begins.contains_key(&admission.pass_id) {
            (None, None)
        } else {
            if self.has_other_initial_pass(source.id, &admission.pass_id) {
                return Err("source already has initial pass".into());
            }
            let begin = BeginRecord {
                owner_uuid: admission.owner_uuid,
                pass_id: admission.pass_id.clone(),
                source_id: admission.source_id,
                turn: admission.turn,
                graph_version_before: admission.graph_version_before,
                population: BeginPopulation::ExplicitSet(admission.preexisting_ids.clone()),
            };
            let stored = StoredAdmission::from(admission.clone());
            self.validate_begin(&begin, &stored)?;
            let payload = dream_begin_codec::encode(&begin)?;
            let mut journal = self.journal.clone();
            journal.ingest_admitted_begin(self.owner_uuid, pending_pass(&stored))?;
            let begin = StoredBegin::new(&stored, (&begin).into());
            (Some((begin, stored, journal)), Some(payload))
        };
        let record = FrontierRecord {
            owner: self.owner_uuid,
            pass_id: admission.pass_id.clone(),
            begin: begin_payload,
            source: source.into(),
            candidates: candidates.into_iter().map(Into::into).collect(),
            graph_version,
            memory_version,
        };
        let (begin, journal) = match &new_begin {
            Some((begin, _, journal)) => (begin, journal),
            None => (
                self.begins
                    .get(&record.pass_id)
                    .ok_or("frontier lacks Begin")?,
                &self.journal,
            ),
        };
        self.validate_frontier(&record, begin.source_id, journal)?;
        let bytes = record.encode()?;
        container.append(&bytes).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        if let Some((begin, stored, journal)) = new_begin {
            self.journal = journal;
            self.admissions.insert(admission.pass_id.clone(), stored);
            self.install_begin(admission.pass_id.clone(), begin);
        }
        self.frontiers.insert(record.pass_id.clone(), record.into());
        Ok(admission)
    }
}
