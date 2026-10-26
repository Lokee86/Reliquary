use crate::{
    Cva, EntityDraft, MemoryDraft, MemoryEntityMention, MemoryEntityMentionKey, MemoryId,
    MemoryRoutingMetadata, MemoryTextField, Phylactery,
};
use std::path::PathBuf;

pub(crate) fn entity_draft(
    name: &str,
    aliases: &[&str],
    mutation: &str,
    revision: i64,
) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: aliases.iter().map(|value| (*value).into()).collect(),
        kind: "test".into(),
        summary: format!("{name} test entity"),
        mutation_id: format!("{mutation}-{revision}"),
        created_at_ns: 1,
        updated_at_ns: revision,
    }
}

pub(crate) fn publish_rel_memory(rel: &mut Cva, key: &str, title: &str, content: &str) -> MemoryId {
    rel.publish_memory(None, 0, memory_draft(key, title, content))
        .unwrap()
        .0
        .id
}

pub(crate) fn publish_phy_memory(phy: &mut Phylactery, content: &str) -> MemoryId {
    phy.publish_memory(None, 0, memory_draft("phy", "Local editor", content))
        .unwrap()
        .0
        .id
}

pub(crate) fn install_rel_mention(
    rel: &mut Cva,
    memory_id: MemoryId,
    text: &str,
) -> MemoryEntityMentionKey {
    let body_id = rel.memory_body_id(memory_id).unwrap();
    let memory = rel.memory(memory_id).unwrap();
    install_mention(
        &mut rel.memories,
        &mut rel.container,
        memory_id,
        body_id,
        &memory.content,
        text,
    )
}

pub(crate) fn install_phy_mention(
    phy: &mut Phylactery,
    memory_id: MemoryId,
    text: &str,
) -> MemoryEntityMentionKey {
    let body_id = phy.memory_body_id(memory_id).unwrap();
    let memory = phy.memory(memory_id).unwrap();
    install_mention(
        &mut phy.memories,
        &mut phy.container,
        memory_id,
        body_id,
        &memory.content,
        text,
    )
}

pub(crate) fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-entity-candidates-{name}-{}",
        uuid::Uuid::new_v4()
    ))
}

pub(crate) fn memory_draft(key: &str, title: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: content.into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: format!("{key}-{}", uuid::Uuid::new_v4()),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn install_mention(
    memories: &mut crate::memory_store::MemoryStore,
    container: &mut reliquary_container::Container,
    memory_id: MemoryId,
    body_id: crate::MemoryBodyId,
    content: &str,
    text: &str,
) -> MemoryEntityMentionKey {
    let start = content.find(text).unwrap() as u32;
    let mention = MemoryEntityMention {
        field: MemoryTextField::Content,
        start_byte: start,
        end_byte: start + text.len() as u32,
        text: text.into(),
    };
    memories
        .put_routing_metadata(
            container,
            MemoryRoutingMetadata {
                memory_id,
                body_id,
                entity_mentions: vec![mention.clone()],
            },
        )
        .unwrap();
    MemoryEntityMentionKey::new(memory_id, &mention)
}
