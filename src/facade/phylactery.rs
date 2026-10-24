pub use crate::phy_execution_owner::PhyExecutionOwner as Phylactery;
use crate::{Memory, MemoryDraft, MemoryError, MemoryId, MemoryStats, PhylacteryError};

impl Phylactery {
    pub fn owner_id(&self) -> Option<String> {
        self.runtime_owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.runtime_owner_uuid()
    }

    pub fn profile(&self) -> crate::PhylacteryProfile {
        self.runtime_profile()
    }

    pub fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, PhylacteryError> {
        self.native_set_profile(display_name)
    }

    pub fn latest_global_version(&self) -> u64 {
        self.runtime_latest_global_version()
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.runtime_transaction_time_ns(version)
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.runtime_version_at_or_before(transaction_time_ns)
    }

    pub fn publish_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.runtime_publish_knowledge_memory(id, expected_revision, draft, None)
    }

    pub fn tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        self.native_tombstone_memory(id, expected_revision)
    }

    pub fn memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.native_memory_retention_descriptor(id)
    }

    pub fn historical_memory_revision(
        &mut self,
        id: MemoryId,
        revision: u64,
    ) -> Result<
        crate::owner_lifecycle::HistoricalPayload<Memory, crate::MemoryRetainedRevision>,
        MemoryError,
    > {
        let value = match self.memory_revision(id, revision) {
            Ok(value) => value,
            Err(MemoryError::PayloadUnavailable(reason)) => {
                let structure = self
                    .memory_retention_descriptor(id)?
                    .revisions
                    .into_iter()
                    .find(|r| r.revision == revision)
                    .ok_or(MemoryError::MissingMemory)?;
                return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                    structural_history: structure,
                    reason,
                });
            }
            Err(error) => return Err(error),
        };

        Ok(match self.memory_lifecycle_state(id)? {
            crate::owner_lifecycle::LifecycleState::Active => {
                crate::owner_lifecycle::HistoricalPayload::Available(value)
            }
            crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
            }
        })
    }

    pub fn memory_lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, MemoryError> {
        self.native_memory_lifecycle_state(id)
    }

    pub fn set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_set_memory_temporal_status(id, expected_revision, temporal_status, mutation_id)
    }

    pub fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.native_memory_revision(id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.native_memory_stats()
    }

    pub fn memory_version(&self) -> u64 {
        self.native_memory_version()
    }

    pub fn memory_routing_metadata(&self, id: MemoryId) -> Option<&crate::MemoryRoutingMetadata> {
        self.native_memory_routing_metadata(id)
    }

    pub fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.native_put_memory_routing_metadata(metadata)
    }
}
