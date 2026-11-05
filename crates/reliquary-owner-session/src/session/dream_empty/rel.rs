use super::*;
impl RelWriter {
    pub fn commit_empty_dream_pass(
        &mut self,
        pass_id: &str,
        observed: DreamMemoryCut,
        graph_version: u64,
        accepted_turn: u64,
    ) -> Result<bool, OwnerSessionError> {
        self.commit_initial_dream_pass(
            pass_id,
            observed,
            graph_version,
            accepted_turn,
            BTreeMap::new(),
            false,
        )
    }

    pub fn begin_first_empty_dream_pass(
        &mut self,
        pass_id: String,
        source: DreamMemoryCut,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        self.begin_initial_dream_pass(pass_id, source, turn)
    }

    /// Admit a retained revision-one Memory with its exact birth population. Returns
    /// owner-authenticated admission evidence required to reopen this pass.
    pub fn begin_initial_dream_pass(
        &mut self,
        pass_id: String,
        source: DreamMemoryCut,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        self.ensure_write_ready()?;
        let admission =
            initial_admission(&self.container, &self.session.common, source, pass_id, turn)?;
        if self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.session.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let result = self
            .session
            .common
            .dreams
            .append_initial_begin(&mut self.container, admission);
        match result {
            Ok(admission) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(admission)
            }
            Err(error) => {
                self.session.write_poisoned = true;
                Err(OwnerSessionError::Record(error))
            }
        }
    }

    /// Settle an authenticated Begin with retained direct initial relation evidence.
    /// Application supplies pure propagation effects at the checked Graph cut.
    pub fn commit_initial_dream_pass(
        &mut self,
        pass_id: &str,
        observed: DreamMemoryCut,
        graph_version: u64,
        accepted_turn: u64,
        effects: BTreeMap<MemoryId, i16>,
        promote: bool,
    ) -> Result<bool, OwnerSessionError> {
        self.ensure_write_ready()?;
        let lease = self.container.writer_lease_proof();
        if lease.is_none() || lease != self.session.writer_lease_proof {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        preflight_source(
            &self.session.common,
            pass_id,
            observed,
            graph_version,
            accepted_turn,
        )?;
        let memory_delta = promotion(&self.session.common, pass_id, observed, promote)?;
        let record = InitialDreamSettlement {
            memory_delta,
            owner: self.owner_id(),
            pass_id: pass_id.to_owned(),
            source: observed.id,
            revision: observed.revision,
            body: observed.body_id,
            graph_version,
            accepted_turn,
            community_generation: self
                .session
                .common
                .dreams
                .settlement_record(pass_id)
                .map(|r| r.community_generation)
                .unwrap_or_else(|| {
                    self.session
                        .common
                        .communities
                        .current()
                        .filter(|c| c.graph_version == graph_version)
                        .map(|c| c.generation)
                }),
            roots: super::dream_initial::roots(&self.session.common, pass_id)?,
            freshness_effects: match self
                .session
                .common
                .dreams
                .settlement_freshness_effects(pass_id)
            {
                Some(effects) => effects,
                None => {
                    let begin = self
                        .session
                        .common
                        .dreams
                        .begin(pass_id)
                        .ok_or(OwnerSessionError::StaleDreamCut)?;
                    self.session
                        .freshness
                        .prepare_initial_dream(
                            FreshnessMemoryId(observed.id.0),
                            begin.turn,
                            accepted_turn,
                            graph_version,
                            self.session
                                .common
                                .communities
                                .current()
                                .filter(|c| c.graph_version == graph_version)
                                .map(|c| c.generation),
                            effects
                                .iter()
                                .map(|(id, amount)| (FreshnessMemoryId(id.0), *amount))
                                .collect(),
                        )
                        .map_err(OwnerSessionError::Record)?
                }
            },
        };
        validate(&self.session.common, self.owner_id(), &record)?;
        super::dream_initial::validate_effects(
            &self.session.common,
            &record,
            &effects,
            Some(self.session.freshness.policy().linkage_principal),
        )?;
        let memory_event = record.memory_event();
        let effects = record.freshness_effects.clone();
        let result = self
            .session
            .common
            .dreams
            .commit_initial_settlement(&mut self.container, record);
        match result {
            Ok(accepted) => {
                if accepted {
                    if let Some(event) = memory_event {
                        if let Err(error) =
                            self.session.common.memories.apply_initial_promotion(&event)
                        {
                            self.session.write_poisoned = true;
                            return Err(error.into());
                        }
                    }
                }
                if accepted {
                    for effect in effects {
                        if let Err(error) = self.session.freshness.ingest_payload(&effect) {
                            self.session.write_poisoned = true;
                            return Err(OwnerSessionError::Record(error));
                        }
                    }
                }
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(accepted)
            }
            Err(message) => {
                self.session.write_poisoned = true;
                Err(OwnerSessionError::Record(message))
            }
        }
    }
}
