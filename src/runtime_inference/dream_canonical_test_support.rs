use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeId, EpisodeOrigin, Memory, MemoryDraft, MemoryId,
};

pub(crate) struct AuthorityAnchor {
    episode_id: EpisodeId,
    node_id: String,
}

pub(crate) fn authority_anchor(cva: &mut Cva, key: &str, timestamp_ns: i64) -> AuthorityAnchor {
    let conversation = format!("conversation-{key}");
    let node_id = format!("user-{key}");
    cva.append_node(
        node_id.clone(),
        conversation.clone(),
        None,
        "user".into(),
        timestamp_ns,
        "Authority statement.",
    )
    .unwrap();
    let episode = cva
        .materialize_path_episodes(
            &conversation,
            &node_id,
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, timestamp_ns + 1)),
        )
        .unwrap()
        .created
        .into_iter()
        .next()
        .unwrap();
    AuthorityAnchor {
        episode_id: episode.id,
        node_id,
    }
}

pub(crate) fn draft_from(memory: &Memory, lifecycle: &str, mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: lifecycle.into(),
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id.clone(),
        content_source_conversation_id: memory.content_source_conversation_id.clone(),
        content_source_node_id: memory.content_source_node_id.clone(),
        grounding_source_conversation_id: memory.grounding_source_conversation_id.clone(),
        grounding_source_node_id: memory.grounding_source_node_id.clone(),
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: mutation.into(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns + 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn publish(
    cva: &mut Cva,
    mutation: &str,
    anchor: &AuthorityAnchor,
    category: &str,
    memory_type: &str,
    authority_kind: &str,
    lifecycle: &str,
    content: &str,
) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: category.into(),
            memory_type: memory_type.into(),
            authority_kind: authority_kind.into(),
            temporal_status: "unknown".into(),
            title: content.into(),
            content: content.into(),
            scope: "private".into(),
            lifecycle_state: lifecycle.into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some(anchor.node_id.clone()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(reliquary_memory_core::SourceEpisodeId(
                (anchor.episode_id).0,
            )),
            source_time_ns: None,
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        },
    )
    .unwrap()
    .0
    .id
}
