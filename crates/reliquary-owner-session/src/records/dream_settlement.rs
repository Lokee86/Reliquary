use super::freshness_journal::AcceptedGraphCutProof;
use super::*;
impl DreamEvidenceStore {
    pub(crate) fn apply_initial_settlement(
        &mut self,
        settlement: InitialDreamSettlement,
    ) -> Result<(), String> {
        let journal = self.prepare_initial_settlement(&settlement)?;
        self.install_initial_settlement(settlement, journal);
        Ok(())
    }

    fn prepare_initial_settlement(
        &self,
        settlement: &InitialDreamSettlement,
    ) -> Result<DreamFreshnessJournal, String> {
        settlement.validate()?;
        if settlement.owner != self.owner_uuid
            || self.initial_settlements.contains_key(&settlement.pass_id)
            || self.settlement_positions.contains_key(&settlement.source)
        {
            return Err("duplicate or foreign initial Dream settlement".into());
        }
        let begun = self
            .begins
            .get(&settlement.pass_id)
            .ok_or("initial Dream settlement lacks prior Begin")?;
        if begun.source_id != settlement.source
            || settlement.graph_version < begun.graph_version_before
            || settlement.accepted_turn < begun.turn
        {
            return Err("initial Dream settlement conflicts with begun owner cut".into());
        }
        let pass = self
            .journal
            .pending_ref(&settlement.pass_id)
            .ok_or("initial Dream settlement is not pending")?;
        if !pass.roots.is_empty()
            || !settlement
                .roots
                .iter()
                .all(|id| pass.preexisting_ids.contains(id))
            || pass.source_id != settlement.source
            || pass.first_revision != 1
        {
            return Err("initial Dream settlement has roots or incorrect source revision".into());
        }
        let mut staged = self.journal.clone();
        staged.derive_publication_birth(settlement.source);
        let roots = staged.roots_payload(&settlement.pass_id, &settlement.roots)?;
        staged.ingest(&roots)?;
        let acceptance = staged.accept_graph_payload(
            &settlement.pass_id,
            settlement.accepted_turn,
            settlement.graph_version,
            AcceptedGraphCutProof {
                graph_version: settlement.graph_version,
                community_generation: settlement.community_generation,
                community_graph_version: settlement
                    .community_generation
                    .map(|_| settlement.graph_version),
            },
        )?;
        if acceptance.is_empty() {
            return Err("initial Dream already has an accepted graph cut".into());
        }
        staged.ingest(&acceptance)?;
        let settled = staged.settled_payload(&settlement.pass_id)?;
        staged.ingest(&settled)?;
        Ok(staged)
    }

    fn install_initial_settlement(
        &mut self,
        settlement: InitialDreamSettlement,
        journal: DreamFreshnessJournal,
    ) {
        self.journal = journal;
        self.settled_sources.push(settlement.source);
        self.settlement_positions
            .insert(settlement.source, self.settled_sources.len() as u64);
        self.initial_settlements
            .insert(settlement.pass_id.clone(), settlement);
    }

    /// One physical frame records accepted first-cycle Graph cut and final
    /// Dream journal settlement. A sync failure requires owner-session reopen.
    pub(crate) fn commit_initial_settlement(
        &mut self,
        container: &mut Container,
        settlement: InitialDreamSettlement,
    ) -> Result<bool, String> {
        self.check_owner(container)?;
        if let Some(existing) = self.initial_settlements.get(&settlement.pass_id) {
            return if existing == &settlement {
                Ok(false)
            } else {
                Err("conflicting settled Dream pass retry".into())
            };
        }
        let journal = self.prepare_initial_settlement(&settlement)?;
        let encoded = settlement.encode()?;
        container
            .append(&encoded)
            .map_err(|error| error.to_string())?;
        container.sync().map_err(|error| error.to_string())?;
        self.install_initial_settlement(settlement, journal);
        Ok(true)
    }

    pub fn initial_settlement(&self, pass_id: &str) -> Option<(u64, u64)> {
        self.initial_settlements
            .get(pass_id)
            .map(|record| (record.accepted_turn, record.graph_version))
    }

    pub(crate) fn settlement_freshness_effects(&self, pass_id: &str) -> Option<Vec<Vec<u8>>> {
        self.initial_settlements
            .get(pass_id)
            .map(|record| record.freshness_effects.clone())
    }

    pub(crate) fn first_pass(&self, source: MemoryId) -> Option<(&str, bool)> {
        let pass = self.begun_sources.get(&source)?;
        Some((pass, self.initial_settlements.contains_key(pass)))
    }

    pub(crate) fn has_other_initial_pass(&self, source: MemoryId, pass: &str) -> bool {
        self.begun_sources
            .get(&source)
            .is_some_and(|known| known != pass)
    }
    pub(crate) fn admission(&self, pass_id: &str) -> Option<&StoredAdmission> {
        self.admissions.get(pass_id)
    }
    pub(crate) fn settlement_record(&self, pass_id: &str) -> Option<&InitialDreamSettlement> {
        self.initial_settlements.get(pass_id)
    }
    pub(crate) fn initial_settlements(&self) -> impl Iterator<Item = &InitialDreamSettlement> {
        self.initial_settlements.values()
    }

    pub(crate) fn settlement_count(&self) -> u64 {
        self.settled_sources.len() as u64
    }

    pub(crate) fn settlement_position(&self, source: MemoryId) -> Option<u64> {
        self.settlement_positions.get(&source).copied()
    }

    /// First settlements in physical owner order, without reading source bodies.
    pub(crate) fn settlements_after(
        &self,
        after: u64,
        through: u64,
        limit: usize,
    ) -> impl Iterator<Item = (u64, MemoryId)> + '_ {
        let end = through.min(self.settlement_count()) as usize;
        let start = after.min(end as u64) as usize;
        self.settled_sources[start..end]
            .iter()
            .take(limit)
            .enumerate()
            .map(move |(offset, id)| ((start + offset + 1) as u64, *id))
    }
    pub(crate) fn append_initial_begin(
        &mut self,
        container: &mut Container,
        admission: DreamBeginAdmission,
    ) -> Result<DreamBeginAdmission, String> {
        if admission.pass_id.is_empty()
            || admission.pass_id.trim() != admission.pass_id
            || admission.pass_id.len() > 256
            || admission.turn == 0
        {
            return Err("invalid first-cycle Begin identity".into());
        }
        if self.has_other_initial_pass(admission.source_id, &admission.pass_id) {
            return Err("Memory already has a first-cycle Dream Begin".into());
        }
        let stored = StoredAdmission::from(admission.clone());
        if let Some(existing) = self.admissions.get(&admission.pass_id) {
            if existing != &stored {
                return Err("conflicting first-cycle Begin admission".into());
            }
            if self.begins.contains_key(&admission.pass_id) {
                return Ok(admission);
            }
        }
        let begin = BeginRecord {
            owner_uuid: admission.owner_uuid,
            pass_id: admission.pass_id.clone(),
            source_id: admission.source_id,
            turn: admission.turn,
            graph_version_before: admission.graph_version_before,
            population: BeginPopulation::ExplicitSet(admission.preexisting_ids.clone()),
        };
        self.admissions.insert(admission.pass_id.clone(), stored);
        self.append_begin(container, begin)?;
        Ok(admission)
    }
}
