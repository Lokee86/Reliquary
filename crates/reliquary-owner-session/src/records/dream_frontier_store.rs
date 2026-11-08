use super::*;
use crate::records::dream_frontier::{FrontierRecord, InitialDreamFrontier};
impl DreamEvidenceStore {
    pub(super) fn apply_frontier(&mut self, record: FrontierRecord) -> Result<(), String> {
        let begin = self
            .begins
            .get(&record.pass_id)
            .ok_or("frontier lacks Begin")?;
        if record.owner != self.owner_uuid
            || begin.source_id != record.source.cut().id
            || self.frontiers.contains_key(&record.pass_id)
            || self.journal.pending(&record.pass_id).is_none()
        {
            return Err("invalid or duplicate Dream frontier".into());
        }
        self.frontiers.insert(record.pass_id.clone(), record);
        Ok(())
    }
    pub fn initial_frontier(&self, pass: &str) -> Option<InitialDreamFrontier> {
        self.frontiers.get(pass).map(FrontierRecord::view)
    }
    pub(crate) fn has_other_pending_frontier(&self, prefix: &str, pass: &str) -> bool {
        self.frontiers
            .keys()
            .any(|id| id != pass && id.starts_with(prefix) && self.journal.pending(id).is_some())
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
                Ok(self.admissions[&admission.pass_id].clone())
            } else {
                Err("frontier identity changed".into())
            };
        }
        let mut staged = self.clone();
        let mut begin_payload = None;
        if !staged.begins.contains_key(&admission.pass_id) {
            if staged.has_other_initial_pass(source.id, &admission.pass_id) {
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
            staged.validate_begin(&begin, &admission)?;
            begin_payload = Some(dream_begin_codec::encode(&begin)?);
            staged
                .admissions
                .insert(admission.pass_id.clone(), admission.clone());
            staged
                .journal
                .ingest_materialized_begin(self.owner_uuid, materialize(&begin, &admission))?;
            staged.begins.insert(admission.pass_id.clone(), begin);
        }
        let record = FrontierRecord {
            owner: self.owner_uuid,
            pass_id: admission.pass_id.clone(),
            begin: begin_payload,
            source: source.into(),
            candidates: candidates.into_iter().map(Into::into).collect(),
            graph_version,
            memory_version,
        };
        staged.apply_frontier(record.clone())?;
        let bytes = record.encode()?;
        container.append(&bytes).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        *self = staged;
        Ok(admission)
    }
}
