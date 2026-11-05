//! Standard metadata delta for an owner-approved extracted-to-knowledge pass.
use super::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedMemoryInitialPromotion {
    pub source: MemoryId,
    pub previous_revision: u64,
    pub mutation_id: String,
    pub delta: Vec<u8>,
}
impl MemoryStore {
    pub fn prepare_initial_promotion(
        &self,
        source: MemoryId,
        expected_revision: u64,
        mutation_id: String,
    ) -> Result<Vec<u8>, MemoryError> {
        let history = self.history(source)?;
        let current = &history
            .revisions
            .last()
            .ok_or(MemoryError::MissingMemory)?
            .record;
        if !history
            .revisions
            .last()
            .is_some_and(|r| matches!(r.payload, PayloadState::Available(_)))
            || history.tombstone.is_some()
            || current.revision != expected_revision
            || current.lifecycle_state != "extracted"
            || current.archived
            || current.superseded_by.is_some()
            || mutation_id.is_empty()
        {
            return Err(MemoryError::RevisionConflict);
        }
        let mut next = current.clone();
        next.revision = current
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        next.memory_version = self.state.next_memory_version;
        next.lifecycle_state = "knowledge".into();
        next.mutation_id = mutation_id;
        // Promotion is a deterministic metadata revision; content's original
        // update timestamp and provenance remain intact.
        let bytes = encode_delta(current, &next)?;
        self.preflight_grouped_lifecycle_delta(&bytes)?;
        Ok(bytes)
    }
    pub fn apply_initial_promotion(
        &mut self,
        record: &GroupedMemoryInitialPromotion,
    ) -> Result<(), MemoryError> {
        let expected = self.prepare_initial_promotion(
            record.source,
            record.previous_revision,
            record.mutation_id.clone(),
        )?;
        if expected != record.delta {
            return Err(MemoryError::Corrupt(
                "initial Dream changes unrelated metadata",
            ));
        }
        self.apply_grouped_lifecycle_delta(&record.delta)
    }
}
