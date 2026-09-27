use crate::{Archive, Memory};

pub(crate) fn memory_source_timestamp_ns(memory: &Memory) -> Option<i64> {
    memory.source_time_ns
}

pub(crate) fn reliquary_memory_source_timestamp_ns(
    archive: &Archive,
    memory: &Memory,
) -> Option<i64> {
    if let (Some(conversation), Some(node_id)) = (
        memory.content_source_conversation_id.as_deref(),
        memory.content_source_node_id.as_deref(),
    ) && let Some(node) = archive.nodes.get(conversation, node_id)
    {
        return Some(node.timestamp_ns);
    }
    if let (Some(episode_id), Some(node_id)) =
        (memory.source_episode_id, memory.source_node_id.as_deref())
        && let Some(episode) = archive.episodes.get(episode_id)
        && let Some(node) = archive.nodes.get(&episode.conversation_id, node_id)
    {
        return Some(node.timestamp_ns);
    }
    if let Some(episode_time_ns) = memory
        .source_episode_id
        .and_then(|id| archive.episodes.get(id))
        .map(|episode| episode.source_through_ns)
    {
        return Some(episode_time_ns);
    }
    memory_source_timestamp_ns(memory)
}
