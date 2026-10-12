//! Recovery and idempotence primitives for the historical import projection.
use super::{FreshnessBoundaryIndex, FreshnessError, FreshnessPolicy, FreshnessRecord};
use crate::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalCheckpoint {
    pub import_id: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub position: u64,
    pub records: BTreeMap<MemoryId, FreshnessRecord>,
    pub applied_event_ids: BTreeSet<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalCheckpointError {
    InvalidCheckpoint,
    ConflictingImport,
    UnknownRecipient(MemoryId),
    DuplicateEvent([u8; 32]),
    Score(FreshnessError),
}

impl std::fmt::Display for HistoricalCheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "historical checkpoint: {self:?}")
    }
}
impl std::error::Error for HistoricalCheckpointError {}
impl From<FreshnessError> for HistoricalCheckpointError {
    fn from(error: FreshnessError) -> Self {
        Self::Score(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoricalCheckpointCommit {
    Published,
    AlreadyCommitted,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HistoricalCheckpointStore {
    committed: Option<HistoricalCheckpoint>,
}

impl HistoricalCheckpointStore {
    pub fn committed(&self) -> Option<&HistoricalCheckpoint> {
        self.committed.as_ref()
    }

    /// Publishes a fully built projection as one replacement. A repeated identical
    /// import is a no-op; a same-ID divergent input fails closed.
    pub fn publish(
        &mut self,
        candidate: HistoricalCheckpoint,
    ) -> Result<HistoricalCheckpointCommit, HistoricalCheckpointError> {
        validate_checkpoint(&candidate)?;
        if let Some(existing) = &self.committed {
            if existing.import_id != candidate.import_id {
                return Err(HistoricalCheckpointError::ConflictingImport);
            }
            if existing == &candidate {
                return Ok(HistoricalCheckpointCommit::AlreadyCommitted);
            }
            return Err(HistoricalCheckpointError::ConflictingImport);
        }
        self.committed = Some(candidate);
        Ok(HistoricalCheckpointCommit::Published)
    }

    /// Crash recovery only publishes a complete candidate. A dropped staged
    /// candidate leaves the prior authoritative projection untouched.
    pub fn recover_complete_candidate(
        &mut self,
        candidate: HistoricalCheckpoint,
    ) -> Result<HistoricalCheckpointCommit, HistoricalCheckpointError> {
        self.publish(candidate)
    }

    /// Rebuilds the sparse due index from the authoritative projection after reopen.
    pub fn rebuild_due_index(
        &self,
        as_of_position: u64,
    ) -> Result<FreshnessBoundaryIndex, HistoricalCheckpointError> {
        let checkpoint = self
            .committed
            .as_ref()
            .ok_or(HistoricalCheckpointError::InvalidCheckpoint)?;
        let mut index = FreshnessBoundaryIndex::default();
        for (&memory, record) in &checkpoint.records {
            index.insert_with_policy(
                memory,
                record,
                as_of_position,
                &FreshnessPolicy::default_v1(),
            )?;
        }
        Ok(index)
    }

    /// Applies a legitimate post-checkpoint event exactly once on the normal REL clock.
    pub fn apply_post_checkpoint_event(
        &mut self,
        event_id: [u8; 32],
        recipient: MemoryId,
        amount: i16,
        position: u64,
        policy: &FreshnessPolicy,
    ) -> Result<bool, HistoricalCheckpointError> {
        let checkpoint = self
            .committed
            .as_mut()
            .ok_or(HistoricalCheckpointError::InvalidCheckpoint)?;
        if checkpoint.applied_event_ids.contains(&event_id) {
            return Ok(false);
        }
        let record = checkpoint
            .records
            .get_mut(&recipient)
            .ok_or(HistoricalCheckpointError::UnknownRecipient(recipient))?;
        record.reinforce_with_policy(amount, position, policy)?;
        checkpoint.applied_event_ids.insert(event_id);
        checkpoint.position = checkpoint.position.max(position);
        Ok(true)
    }
}

fn validate_checkpoint(checkpoint: &HistoricalCheckpoint) -> Result<(), HistoricalCheckpointError> {
    if checkpoint.import_id == [0; 32]
        || checkpoint.manifest_digest == [0; 32]
        || checkpoint.position == 0
        || checkpoint.records.is_empty()
    {
        return Err(HistoricalCheckpointError::InvalidCheckpoint);
    }
    Ok(())
}

#[cfg(test)]
#[path = "historical_checkpoint_tests.rs"]
mod tests;
