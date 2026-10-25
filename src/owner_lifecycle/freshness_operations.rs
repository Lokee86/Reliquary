use crate::{
    Container, ContainerError, FreshnessEventCut, FreshnessEventProof, FreshnessRecord, Memory,
    MemoryError, MemoryId,
};

pub(crate) fn initial_dream_pass_id(owner: [u8; 16], source: MemoryId) -> String {
    crate::dream_pass_identity::initial_pass_id(owner, source)
}

pub(crate) fn created_record() -> FreshnessRecord {
    FreshnessRecord::created()
}

pub(crate) fn admit_record(
    record: &mut FreshnessRecord,
    turn: u64,
) -> Result<bool, crate::freshness::FreshnessError> {
    record.admit(turn)
}

pub(crate) fn take_due_for(
    dispatcher: &mut crate::FreshnessDueDispatcher,
    memory: MemoryId,
    through_turn: u64,
) -> Result<Vec<crate::FreshnessDue>, crate::freshness::FreshnessError> {
    dispatcher.take_due_for(memory, through_turn)
}

pub(crate) fn accept_next_due_crossing(
    dispatcher: &mut crate::FreshnessDueDispatcher,
    accepted_turn: u64,
) -> Option<crate::FreshnessDue> {
    let due = dispatcher.drain_due(accepted_turn, 1).into_iter().next()?;
    if dispatcher.advance(due).is_err() {
        let _ = dispatcher.defer(due);
        return None;
    }
    Some(due)
}

pub(crate) fn refresh_due_with_transition(
    dispatcher: &mut crate::FreshnessDueDispatcher,
    memory: MemoryId,
    record: &FreshnessRecord,
    accepted_turn: u64,
    policy: &crate::freshness::FreshnessPolicy,
) -> Result<crate::FreshnessRefresh, crate::freshness::FreshnessError> {
    dispatcher.refresh_with_transition(memory, record, accepted_turn, policy)
}

pub(crate) fn require_write_admission(container: &Container) -> Result<(), ContainerError> {
    container.require_write_admission()
}

pub(crate) fn owner_uuid(container: &Container) -> Option<[u8; 16]> {
    container.owner_uuid()
}

pub(crate) fn read_memory(
    memories: &crate::memory_store::MemoryStore,
    container: &Container,
    id: MemoryId,
) -> Result<Memory, MemoryError> {
    memories.memory(container, id)
}

pub(crate) fn append_and_sync(
    container: &mut Container,
    payload: &[u8],
) -> Result<(), ContainerError> {
    container.append(payload)?;
    container.sync()
}

pub(crate) fn persist_policy_if_needed(
    owner: &mut crate::owner_lifecycle::ReliquaryOwner,
    owner_uuid: [u8; 16],
) -> Result<(), String> {
    owner
        .freshness
        .persist_policy_if_needed(&mut owner.container, owner_uuid)
}

impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn freshness_record_at_cut(
        &self,
        id: MemoryId,
        cut: &FreshnessEventCut,
    ) -> Result<Option<FreshnessRecord>, String> {
        self.freshness.record_at_cut(id, cut)
    }

    pub(crate) fn freshness_record_at(
        &self,
        id: MemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, String> {
        self.freshness.record_at(id, turn)
    }

    pub(crate) fn freshness_records_at(
        &self,
        turn: u64,
    ) -> Result<std::collections::BTreeMap<MemoryId, FreshnessRecord>, String> {
        self.freshness.records_at(turn)
    }

    pub(crate) fn freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.freshness.records().get(&id).copied()
    }

    pub(crate) fn freshness_contains_event(&self, id: &str) -> Result<bool, String> {
        self.freshness.contains_event(id)
    }

    pub(crate) fn freshness_event_matches(
        &self,
        id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &std::collections::BTreeMap<MemoryId, i16>,
    ) -> Result<bool, String> {
        self.freshness.event_matches(id, turn, proof, effects)
    }

    pub(crate) fn freshness_prepare_event_postimages(
        &self,
        id: &str,
        turn: u64,
        effects: &std::collections::BTreeMap<MemoryId, i16>,
    ) -> Result<std::collections::BTreeMap<MemoryId, FreshnessRecord>, String> {
        self.freshness
            .prepare_event_postimages_for(id, turn, effects)
    }

    pub(crate) fn freshness_validate_event_payload(
        &self,
        id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &std::collections::BTreeMap<MemoryId, i16>,
        records: &std::collections::BTreeMap<MemoryId, FreshnessRecord>,
    ) -> Result<(), String> {
        self.freshness
            .validate_event_payload(id, turn, proof, effects, records)
    }

    pub(crate) fn freshness_encode_event(
        &self,
        owner: [u8; 16],
        id: String,
        turn: u64,
        proof: FreshnessEventProof,
        effects: std::collections::BTreeMap<MemoryId, i16>,
        records: std::collections::BTreeMap<MemoryId, FreshnessRecord>,
    ) -> Vec<u8> {
        self.freshness
            .encode_event(owner, id, turn, proof, effects, records)
    }

    pub(crate) fn freshness_persist_policy_if_needed(
        &mut self,
        owner_uuid: [u8; 16],
    ) -> Result<(), String> {
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner_uuid)
    }

    pub(crate) fn freshness_ingest_appended(&mut self, payload: &[u8]) -> Result<(), String> {
        self.freshness.ingest_appended(payload)
    }

    pub(crate) fn freshness_replay_entries(
        &self,
    ) -> Result<Vec<crate::freshness_storage::FreshnessReplayEntry>, String> {
        self.freshness.replay_entries()
    }

    pub(crate) fn freshness_event_receipt(
        &self,
        id: &str,
    ) -> Result<
        Option<(
            u64,
            FreshnessEventProof,
            std::collections::BTreeMap<MemoryId, i16>,
        )>,
        String,
    > {
        self.freshness.event_receipt(id)
    }

    pub(crate) fn freshness_has_record(&self, id: MemoryId) -> bool {
        self.freshness.records().contains_key(&id)
    }

    pub(crate) fn freshness_has_any_record(&self) -> bool {
        !self.freshness.records().is_empty()
    }

    pub(crate) fn freshness_has_durable_birth(&self, id: MemoryId) -> Result<bool, String> {
        self.freshness.has_durable_birth(id)
    }

    pub(crate) fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        self.freshness.policy()
    }

    pub(crate) fn current_freshness_rewrite_record(
        &self,
        id: MemoryId,
        as_of_turn: u64,
    ) -> Option<crate::FreshnessRecord> {
        self.freshness.current_rewrite_record(id, as_of_turn)
    }
}
