//! Standard, narrowly scoped metadata transitions for accepted Dream plans.
use super::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedMemoryLifecycle {
    pub id: MemoryId,
    pub previous_revision: u64,
    pub state: String,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
    pub mutation_id: String,
    pub updated_at_ns: i64,
    pub delta: Vec<u8>,
}
impl MemoryStore {
    pub fn prepare_dream_lifecycle(
        &self,
        intent: &GroupedMemoryLifecycle,
    ) -> Result<Vec<u8>, MemoryError> {
        let current = self.current(intent.id)?.record;
        if current.revision != intent.previous_revision
            || intent.mutation_id.is_empty()
            || intent.updated_at_ns < current.updated_at_ns
            || !matches!(
                (intent.state.as_str(), intent.archived),
                ("knowledge", false) | ("canonical", false) | ("archived", true)
            )
            || (!intent.archived && intent.superseded_by.is_some())
        {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(target) = intent.superseded_by {
            if target == intent.id || self.current(target)?.record.archived {
                return Err(MemoryError::RevisionConflict);
            }
        }
        let mut next = current.clone();
        next.revision = current
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        next.memory_version = self.state.next_memory_version;
        next.lifecycle_state = intent.state.clone();
        next.archived = intent.archived;
        next.superseded_by = intent.superseded_by;
        next.mutation_id = intent.mutation_id.clone();
        next.updated_at_ns = intent.updated_at_ns;
        let bytes = encode_delta(&current, &next)?;
        self.preflight_grouped_lifecycle_delta(&bytes)?;
        Ok(bytes)
    }
    pub fn apply_dream_lifecycle(
        &mut self,
        intent: &GroupedMemoryLifecycle,
    ) -> Result<(), MemoryError> {
        if self.prepare_dream_lifecycle(intent)? != intent.delta {
            return Err(MemoryError::Corrupt(
                "Dream batch changes unrelated metadata",
            ));
        }
        self.apply_grouped_lifecycle_delta(&intent.delta)
    }
}
