use crate::{Memory, MemoryDraft, MemoryError, MemoryId, MemorySourceRef, Phylactery};

impl Phylactery {
    pub(crate) fn publish_memory_with_source_ref(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_memory_with_source_ref_and_temporal_inference(
            id,
            expected_revision,
            draft,
            source_ref,
            None,
        )
    }

    pub(crate) fn publish_memory_with_source_ref_and_temporal_inference(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: MemorySourceRef,
        temporal_inference: Option<crate::MemoryTemporalInference>,
    ) -> Result<(Memory, bool), MemoryError> {
        self.memories
            .publish_with_source_ref_and_temporal_inference(
                &mut self.container,
                id,
                expected_revision,
                draft,
                Some(source_ref),
                temporal_inference,
            )
    }

    pub fn backfill_memory_source_ref(
        &mut self,
        id: MemoryId,
        source_ref: MemorySourceRef,
        updated_at_ns: i64,
    ) -> Result<(Memory, bool), MemoryError> {
        let current = self.memories.memory(&mut self.container, id)?;
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
        self.memories.publish_with_source_ref(
            &mut self.container,
            Some(id),
            current.revision,
            draft,
            Some(source_ref),
        )
    }
}
