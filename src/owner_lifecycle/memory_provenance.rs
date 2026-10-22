//! REL-only composition of Memory references against accepted Archive evidence.
use crate::memory_store::MemoryStore;
use crate::{Archive, MemoryError};
use std::collections::{HashMap, HashSet};

pub(crate) fn validate(memories: &MemoryStore, archive: &Archive) -> Result<(), MemoryError> {
    memories.validate_provenance_fields()?;
    let mut episode_nodes: HashMap<_, HashSet<&str>> = HashMap::new();
    for record in memories.records() {
        if let (Some(episode), Some(node)) =
            (record.source_episode_id, record.source_node_id.as_deref())
        {
            episode_nodes.entry(episode).or_default().insert(node);
        }
        for pair in [
            (
                record.content_source_conversation_id.as_deref(),
                record.content_source_node_id.as_deref(),
            ),
            (
                record.grounding_source_conversation_id.as_deref(),
                record.grounding_source_node_id.as_deref(),
            ),
        ] {
            match pair {
                (Some(conversation), Some(node)) if archive.has_node(conversation, node) => {}
                (None, None) => {}
                _ => return Err(MemoryError::InvalidProvenance),
            }
        }
    }
    for (episode, nodes) in episode_nodes {
        if !archive
            .episode_contains_all_nodes(episode, &nodes)
            .map_err(|_| MemoryError::InvalidProvenance)?
        {
            return Err(MemoryError::InvalidProvenance);
        }
    }
    Ok(())
}
