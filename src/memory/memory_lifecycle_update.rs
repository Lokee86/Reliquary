use crate::memory_store::MemoryOwnerWriter;
use crate::{Memory, MemoryDraft, MemoryError, MemoryId};

/// Passive, operation-specific state requested by an upper semantic owner.
/// This carries no Dream policy or callback capability.
pub(crate) struct MemoryLifecycleUpdate {
    pub(crate) id: MemoryId,
    pub(crate) lifecycle_state: String,
    pub(crate) archived: bool,
    pub(crate) superseded_by: Option<MemoryId>,
    pub(crate) mutation_id: String,
}

pub(crate) fn apply_memory_lifecycle_updates(
    memories: &mut MemoryOwnerWriter<'_>,
    updates: &[MemoryLifecycleUpdate],
) -> Result<Vec<Option<Memory>>, MemoryError> {
    let mut applied = Vec::with_capacity(updates.len());
    for update in updates {
        let current = memories.memory(update.id)?;
        if current.lifecycle_state == update.lifecycle_state
            && current.archived == update.archived
            && current.superseded_by == update.superseded_by
        {
            applied.push(None);
            continue;
        }
        let draft = MemoryDraft {
            category: current.category.clone(),
            memory_type: current.memory_type.clone(),
            authority_kind: current.authority_kind.clone(),
            temporal_status: current.temporal_status.clone(),
            title: current.title.clone(),
            content: current.content.clone(),
            scope: current.scope.clone(),
            lifecycle_state: update.lifecycle_state.clone(),
            archived: update.archived,
            superseded_by: update.superseded_by,
            parent_id: current.parent_id,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: current.source_time_ns,
            mutation_id: update.mutation_id.clone(),
            created_at_ns: current.created_at_ns,
            updated_at_ns: current.updated_at_ns.saturating_add(1),
        };
        let (memory, changed) = memories.publish(update.id, current.revision, draft)?;
        applied.push(changed.then_some(memory));
    }
    Ok(applied)
}
