//! Canonical runtime Memory contracts consumed by supported product bindings.
use crate::{Memory, MemoryDraft, MemoryError, MemoryId, MemoryStats};

impl crate::RelExecutionOwner {
    pub(crate) fn native_tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        self.owner.tombstone_memory_native(id, expected_revision)
    }

    pub(crate) fn native_memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.memories.retention_descriptor(id)
    }

    pub(crate) fn native_memory_lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, MemoryError> {
        self.memories.lifecycle_state(id)
    }

    pub(crate) fn native_memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub(crate) fn native_memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub(crate) fn native_put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.owner
            .memories
            .put_routing_metadata(&mut self.owner.container, metadata)
    }

    pub(crate) fn native_set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        if !matches!(temporal_status, "current" | "future" | "historical") {
            return Err(MemoryError::InvalidField("temporal status"));
        }
        let current = self.runtime_memory(id)?;
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
        self.runtime_publish_knowledge_memory(Some(id), expected_revision, draft)
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_tombstone_memory(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        self.owner.tombstone_memory_native(id, expected_revision)
    }

    pub(crate) fn native_memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, MemoryError> {
        self.memories.retention_descriptor(id)
    }

    pub(crate) fn native_memory_lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, MemoryError> {
        self.memories.lifecycle_state(id)
    }

    pub(crate) fn native_memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub(crate) fn native_memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub(crate) fn native_put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.owner
            .memories
            .put_routing_metadata(&mut self.owner.container, metadata)
    }

    pub(crate) fn native_set_memory_temporal_status(
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

    pub(crate) fn native_memory_revision(
        &mut self,
        id: MemoryId,
        revision: u64,
    ) -> Result<Memory, MemoryError> {
        self.owner
            .memories
            .memory_revision(&mut self.owner.container, id, revision)
    }

    pub(crate) fn native_memory_routing_metadata(
        &self,
        id: MemoryId,
    ) -> Option<&crate::MemoryRoutingMetadata> {
        self.memories.routing_metadata(id)
    }
}

use crate::MemorySourceRef;
impl crate::phy_execution_owner::PhyExecutionOwner {
    #[cfg(test)]
    pub(crate) fn native_publish_memory_with_source_ref(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
    ) -> Result<(Memory, bool), MemoryError> {
        self.native_publish_memory_with_source_ref_and_temporal_inference(
            id,
            expected_revision,
            draft,
            source_ref,
            None,
        )
    }
    #[cfg(test)]
    pub(crate) fn native_publish_memory_with_source_ref_and_temporal_inference(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
        temporal_inference: Option<crate::MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.owner
            .memories
            .publish_with_source_ref_and_temporal_inference(
                &mut self.owner.container,
                id,
                expected_revision,
                draft,
                Some(source_ref),
                temporal_inference,
            )
    }
    pub(crate) fn native_backfill_memory_source_ref(
        &mut self,
        id: MemoryId,
        source_ref: MemorySourceRef,
        updated_at_ns: i64,
    ) -> Result<(Memory, bool), MemoryError> {
        let current = self.owner.memories.memory(&mut self.owner.container, id)?;
        if let Some(existing) = current.source_ref.as_ref() {
            return if existing == &source_ref {
                Ok((current, false))
            } else {
                Err(MemoryError::InvalidProvenance)
            };
        }
        let next_revision = current
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let mutation_id = format!(
            "source-ref-backfill:{}:{next_revision}",
            id.0.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let draft = MemoryDraft {
            category: current.category.clone(),
            memory_type: current.memory_type.clone(),
            authority_kind: current.authority_kind.clone(),
            temporal_status: current.temporal_status.clone(),
            title: current.title.clone(),
            content: current.content.clone(),
            scope: current.scope.clone(),
            lifecycle_state: current.lifecycle_state.clone(),
            archived: current.archived,
            superseded_by: current.superseded_by,
            parent_id: current.parent_id,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: current.source_time_ns,
            mutation_id,
            created_at_ns: current.created_at_ns,
            updated_at_ns: updated_at_ns.max(current.updated_at_ns),
        };
        self.owner.memories.publish_with_source_ref(
            &mut self.owner.container,
            Some(id),
            current.revision,
            draft,
            Some(source_ref),
        )
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn native_external_memory_evidence(
        &self,
        id: MemoryId,
    ) -> Result<
        Option<crate::external_source::ExternalMemoryEvidence>,
        crate::external_source::ExternalSourceError,
    > {
        let body = self
            .memories
            .current_body_id(id)
            .map_err(|_| crate::external_source::ExternalSourceError::Unavailable)?;
        Ok(self.memories.external_evidence.get(&(id, body)).cloned())
    }
}
