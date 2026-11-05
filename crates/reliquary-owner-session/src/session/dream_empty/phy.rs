use super::*;
impl PhyWriter {
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

    pub fn begin_initial_dream_pass(
        &mut self,
        pass_id: String,
        source: DreamMemoryCut,
        turn: u64,
    ) -> Result<DreamBeginAdmission, OwnerSessionError> {
        let admission = initial_admission(&self.container, &self.common, source, pass_id, turn)?;
        if self.poisoned
            || self.container.writer_lease_proof().is_none()
            || self.container.writer_lease_proof() != self.writer_lease_proof
        {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let result = self
            .common
            .dreams
            .append_initial_begin(&mut self.container, admission);
        match result {
            Ok(admission) => {
                self.writer_lease_proof = self.container.writer_lease_proof();
                Ok(admission)
            }
            Err(error) => {
                self.poisoned = true;
                Err(OwnerSessionError::Record(error))
            }
        }
    }

    /// Private-owner counterpart; no REL activity or Archive is inferred.
    pub fn commit_initial_dream_pass(
        &mut self,
        pass_id: &str,
        observed: DreamMemoryCut,
        graph_version: u64,
        accepted_turn: u64,
        effects: BTreeMap<MemoryId, i16>,
        promote: bool,
    ) -> Result<bool, OwnerSessionError> {
        let lease = self.container.writer_lease_proof();
        if self.poisoned || lease.is_none() || lease != self.writer_lease_proof {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        preflight_source(
            &self.common,
            pass_id,
            observed,
            graph_version,
            accepted_turn,
        )?;
        let memory_delta = promotion(&self.common, pass_id, observed, promote)?;
        let admit = self
            .common
            .dreams
            .settlement_record(pass_id)
            .map(|r| r.admit)
            .unwrap_or(!self.common.memories.current(observed.id)?.record.archived);
        let credit_roots = self
            .common
            .dreams
            .settlement_record(pass_id)
            .map(|r| r.credit_roots.clone())
            .unwrap_or_else(|| Default::default());
        let record = InitialDreamSettlement {
            admit,
            credit_roots,
            memory_delta,
            owner: self.owner_id,
            pass_id: pass_id.to_owned(),
            source: observed.id,
            revision: observed.revision,
            body: observed.body_id,
            graph_version,
            accepted_turn,
            community_generation: self
                .common
                .dreams
                .settlement_record(pass_id)
                .map(|r| r.community_generation)
                .unwrap_or_else(|| {
                    self.common
                        .communities
                        .current()
                        .filter(|c| c.graph_version == graph_version)
                        .map(|c| c.generation)
                }),
            roots: super::dream_initial::roots(&self.common, pass_id)?,
            freshness_effects: Vec::new(),
        };
        validate(&self.common, self.owner_id, &record)?;
        super::dream_initial::validate_effects(&self.common, &record, &effects, None)?;
        let memory_event = record.memory_event();
        let result = self
            .common
            .dreams
            .commit_initial_settlement(&mut self.container, record);
        match result {
            Ok(accepted) => {
                if accepted {
                    if let Some(event) = memory_event {
                        if let Err(error) = self.common.memories.apply_initial_promotion(&event) {
                            self.poisoned = true;
                            return Err(error.into());
                        }
                    }
                }
                self.writer_lease_proof = self.container.writer_lease_proof();
                if accepted {
                    if let Err(error) = self.common.refresh_dream_index(&[observed.id]) {
                        self.poisoned = true;
                        return Err(error);
                    }
                }
                Ok(accepted)
            }
            Err(message) => {
                self.poisoned = true;
                Err(OwnerSessionError::Record(message))
            }
        }
    }
}
