use crate::freshness::FreshnessRecord;
use crate::{CvaError, MemoryId};
use std::collections::BTreeSet;

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_has_pending_publication(&self, id: MemoryId) -> bool {
        self.owner
            .dream_freshness
            .pending_initialization(id)
            .is_some()
    }

    pub(crate) fn runtime_prepare_memory_publication(
        &mut self,
        id: MemoryId,
        mutation_id: &str,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), CvaError> {
        if turn > self.owner.archive.rel_turn_count() {
            return Err(CvaError::Freshness(
                "publication turn exceeds accepted owner REL activity".into(),
            ));
        }
        if self.owner.container.owner_uuid().is_none() {
            return Err(CvaError::Freshness(
                "legacy owner requires explicit Freshness migration".into(),
            ));
        }
        let payload = self
            .owner
            .dream_freshness
            .publication_intent_payload(id, mutation_id, turn, admit_at_birth)
            .map_err(CvaError::Freshness)?;
        if !payload.is_empty() {
            self.owner.container.append(&payload)?;
            self.owner.container.sync()?;
            self.owner
                .dream_freshness
                .ingest(&payload)
                .map_err(CvaError::Freshness)?;
        }
        Ok(())
    }

    pub(crate) fn runtime_require_freshness_memory(&self, id: MemoryId) -> Result<(), CvaError> {
        if self.owner.memories.contains_memory(id) {
            Ok(())
        } else {
            Err(CvaError::Freshness(format!(
                "unknown owner-local Memory {:?}",
                id
            )))
        }
    }

    pub(crate) fn runtime_freshness_initialize(&mut self, id: MemoryId) -> Result<bool, CvaError> {
        self.runtime_require_freshness_memory(id)?;
        let intent = self.owner.dream_freshness.pending_initialization(id);
        if self.owner.freshness.records().contains_key(&id) {
            if let Some((mutation_id, turn, admit_at_birth)) = intent {
                if self.owner.memories.first_mutation_id(id) != Some(mutation_id.as_str()) {
                    return Err(CvaError::Freshness(
                        "publication intent does not match first accepted Memory identity".into(),
                    ));
                }
                if !self
                    .owner
                    .freshness
                    .has_durable_birth(id)
                    .map_err(CvaError::Freshness)?
                {
                    self.runtime_persist_accepted_freshness_birth(id, turn, admit_at_birth)?;
                }
                self.runtime_consume_publication_intent(id)?;
            }
            return Ok(false);
        }
        let (mutation_id, turn, admit_at_birth) = intent.clone().ok_or_else(|| {
            CvaError::Freshness(
                "Freshness enrollment requires accepted first-publication proof; legacy enrollment needs explicit migration".into(),
            )
        })?;
        if self.owner.memories.first_mutation_id(id) != Some(mutation_id.as_str()) {
            return Err(CvaError::Freshness(
                "publication intent does not match first accepted Memory identity".into(),
            ));
        }
        self.runtime_persist_accepted_freshness_birth(id, turn, admit_at_birth)?;
        self.runtime_freshness_refresh_due([id], turn)?;
        if intent.is_some() {
            self.runtime_consume_publication_intent(id)?;
        }
        Ok(true)
    }

    fn runtime_persist_accepted_freshness_birth(
        &mut self,
        id: MemoryId,
        turn: u64,
        admit_at_birth: bool,
    ) -> Result<(), CvaError> {
        let owner = self.owner.container.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("legacy owner requires explicit migration before enrollment".into())
        })?;
        let mut record = FreshnessRecord::created();
        if admit_at_birth {
            record
                .admit(turn)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
        }
        self.owner
            .freshness
            .persist_policy_if_needed(&mut self.owner.container, owner)
            .map_err(CvaError::Freshness)?;
        let payload = self
            .owner
            .freshness
            .encode_initialize(owner, id, turn, record);
        self.owner.container.append(&payload)?;
        self.owner.container.sync()?;
        self.owner
            .freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)
    }

    fn runtime_consume_publication_intent(&mut self, id: MemoryId) -> Result<(), CvaError> {
        if let Some(payload) = self
            .owner
            .dream_freshness
            .consume_publication_intent_payload(id)
            .map_err(CvaError::Freshness)?
        {
            self.owner.container.append(&payload)?;
            self.owner.container.sync()?;
            self.owner
                .dream_freshness
                .ingest(&payload)
                .map_err(CvaError::Freshness)?;
        }
        Ok(())
    }

    pub(crate) fn runtime_freshness_refresh_due(
        &mut self,
        ids: impl IntoIterator<Item = MemoryId>,
        accepted_turn: u64,
    ) -> Result<(), CvaError> {
        let schedule_turn = self.owner.archive.rel_turn_count().max(accepted_turn);
        let affected: BTreeSet<_> = ids.into_iter().collect();
        for id in &affected {
            let due = self
                .freshness_due
                .take_due_for(*id, schedule_turn)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            for crossing in due {
                self.buffer_freshness_crossing(crossing);
            }
        }
        self.freshness_on_accepted_activity(schedule_turn);
        let policy = self.owner.freshness.policy();
        let as_of_turn = self.owner.archive.rel_turn_count();
        for id in affected {
            let Some(record) = self.owner.freshness.current_rewrite_record(id, as_of_turn) else {
                continue;
            };
            let refreshed = self
                .freshness_due
                .refresh_with_transition(id, &record, schedule_turn, &policy)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            if let Some(reversal) = refreshed.reversal {
                self.freshness_notifications.entry(id).or_default().reversal = Some(reversal);
            }
        }
        Ok(())
    }
}
