use crate::memory_store::MemoryStore;
use crate::{
    Archive, Container, Memory, MemoryDraft, MemoryError, MemoryId, MemorySourceRef,
    MemoryTemporalInference,
};

pub(crate) fn preflight_memory_parts(
    archive: &Archive,
    memories: &mut MemoryStore,
    container: &mut Container,
    id: Option<MemoryId>,
    expected_revision: u64,
    draft: &MemoryDraft,
) -> Result<crate::memory_store::PublishPreflight, MemoryError> {
    validate_memory_provenance(archive, draft)?;
    memories.preflight_publish(container, id, expected_revision, draft, None, None)
}

pub(crate) fn publish_memory_parts(
    archive: &Archive,
    memories: &mut MemoryStore,
    container: &mut Container,
    id: Option<MemoryId>,
    expected_revision: u64,
    draft: MemoryDraft,
) -> Result<(Memory, bool), MemoryError> {
    publish_memory_parts_with_state(
        archive,
        memories,
        container,
        id,
        expected_revision,
        draft,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_memory_parts_with_state(
    archive: &Archive,
    memories: &mut MemoryStore,
    container: &mut Container,
    id: Option<MemoryId>,
    expected_revision: u64,
    draft: MemoryDraft,
    source_ref: Option<MemorySourceRef>,
    temporal_inference: Option<MemoryTemporalInference>,
) -> Result<(Memory, bool), MemoryError> {
    validate_memory_provenance(archive, &draft)?;
    memories.publish_with_source_ref_and_temporal_inference(
        container,
        id,
        expected_revision,
        draft,
        source_ref,
        temporal_inference,
    )
}

fn validate_memory_provenance(archive: &Archive, draft: &MemoryDraft) -> Result<(), MemoryError> {
    match (draft.source_episode_id, draft.source_node_id.as_deref()) {
        (Some(episode_id), Some(node_id)) => {
            if !archive
                .episode_contains_node(episode_id, node_id)
                .map_err(|_| MemoryError::InvalidProvenance)?
            {
                return Err(MemoryError::InvalidProvenance);
            }
        }
        (None, None) => {}
        _ => return Err(MemoryError::InvalidProvenance),
    }
    match (
        draft.content_source_conversation_id.as_deref(),
        draft.content_source_node_id.as_deref(),
    ) {
        (Some(conversation_id), Some(node_id)) if archive.has_node(conversation_id, node_id) => {}
        (None, None) => {}
        _ => return Err(MemoryError::InvalidProvenance),
    }
    match (
        draft.grounding_source_conversation_id.as_deref(),
        draft.grounding_source_node_id.as_deref(),
    ) {
        (Some(conversation_id), Some(node_id)) if archive.has_node(conversation_id, node_id) => {}
        (None, None) => {}
        _ => return Err(MemoryError::InvalidProvenance),
    }
    Ok(())
}
