pub use crate::phy_execution_owner::PhyExecutionOwner as Phylactery;
use crate::{Memory, MemoryDraft, MemoryError, MemoryId, MemoryStats, PhylacteryError};

impl Phylactery {
    pub fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }

    pub fn profile(&self) -> crate::PhylacteryProfile {
        self.profile.current()
    }

    pub fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, PhylacteryError> {
        let normalize = |value: Option<String>| {
            value.and_then(|value| {
                let trimmed = value.trim().to_owned();
                (!trimmed.is_empty()).then_some(trimmed)
            })
        };
        self.owner
            .profile
            .put(
                &mut self.owner.container,
                crate::PhylacteryProfile {
                    display_name: normalize(display_name),
                },
            )
            .map_err(PhylacteryError::Profile)
    }

    pub fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.container.transaction_time_ns(version)
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.container.version_at_or_before(transaction_time_ns)
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
        let result =
            self.owner
                .memories
                .tombstone(&mut self.owner.container, id, expected_revision)?;
        self.owner
            .graph
            .exclude_from_current(crate::SemanticNodeRef::memory(id))
            .map_err(|_| MemoryError::InvalidField("Memory graph projection"))?;
        Ok(result)
    }

    pub fn memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.memories.retention_descriptor(id)
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
        self.memories.lifecycle_state(id)
    }

    pub fn set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        if !matches!(temporal_status, "current" | "future" | "historical") {
            return Err(MemoryError::InvalidField("temporal status"));
        }
        let current = self.memory(id)?;
        if current.revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if current.temporal_status == temporal_status {
            return Ok((current, false));
        }
        let draft = MemoryDraft {
            category: current.category.clone(),
            memory_type: current.memory_type.clone(),
            authority_kind: current.authority_kind.clone(),
            temporal_status: temporal_status.to_owned(),
            title: current.title.clone(),
            content: current.content.clone(),
            scope: current.scope.clone(),
            lifecycle_state: current.lifecycle_state.clone(),
            archived: current.archived,
            superseded_by: current.superseded_by,
            parent_id: current.parent_id,
            source_node_id: current.source_node_id.clone(),
            content_source_conversation_id: current.content_source_conversation_id.clone(),
            content_source_node_id: current.content_source_node_id.clone(),
            grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
            grounding_source_node_id: current.grounding_source_node_id.clone(),
            source_episode_id: current.source_episode_id,
            source_time_ns: current.source_time_ns,
            mutation_id,
            created_at_ns: current.created_at_ns,
            updated_at_ns: current.updated_at_ns,
        };
        self.owner.memories.publish_with_source_ref(
            &mut self.owner.container,
            Some(id),
            expected_revision,
            draft,
            current.source_ref.clone(),
        )
    }

    pub fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.owner
            .memories
            .memory_revision(&mut self.owner.container, id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub fn memory_routing_metadata(&self, id: MemoryId) -> Option<&crate::MemoryRoutingMetadata> {
        self.memories.routing_metadata(id)
    }

    pub fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.owner
            .memories
            .put_routing_metadata(&mut self.owner.container, metadata)
    }
}
