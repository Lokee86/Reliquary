use crate::{
    Cva, EntityDraft, EntityId, MemoryDraft, MemoryEntityMention, MemoryEntityMentionKey, MemoryId,
    MemoryRoutingMetadata, MemoryTextField, Phylactery,
};
use std::path::PathBuf;

pub(crate) fn rel_with_three_mentions() -> (Cva, MemoryId, Vec<MemoryEntityMentionKey>, PathBuf) {
    let path = temp_path("rel");
    let mut owner = Cva::create_project(&path).unwrap();
    let (memory, _) = owner.publish_memory(None, 0, draft()).unwrap();
    let body_id = owner.memory_body_id(memory.id).unwrap();
    let keys = install_mentions(
        &mut owner.owner.memories,
        &mut owner.owner.container,
        memory.id,
        body_id,
    );
    seed_rel_entities(&mut owner, memory.id);
    (owner, memory.id, keys, path)
}

pub(crate) fn phy_with_three_mentions()
-> (Phylactery, MemoryId, Vec<MemoryEntityMentionKey>, PathBuf) {
    let path = temp_path("phy");
    let mut owner = Phylactery::create(&path).unwrap();
    let (memory, _) = owner.publish_memory(None, 0, draft()).unwrap();
    let body_id = owner.memory_body_id(memory.id).unwrap();
    let keys = install_mentions(
        &mut owner.owner.memories,
        &mut owner.owner.container,
        memory.id,
        body_id,
    );
    seed_phy_entities(&mut owner, memory.id);
    (owner, memory.id, keys, path)
}

pub(crate) fn archive_rel_memory(owner: &mut Cva, id: MemoryId) {
    let memory = owner.memory(id).unwrap();
    let mut draft = draft_from(&memory);
    draft.archived = true;
    draft.mutation_id = format!("archive-{}", uuid::Uuid::new_v4());
    owner
        .publish_memory(Some(id), memory.revision, draft)
        .unwrap();
}

pub(crate) fn supersede_rel_memory(owner: &mut Cva, id: MemoryId) {
    let (replacement, _) = owner.publish_memory(None, 0, draft()).unwrap();
    let memory = owner.memory(id).unwrap();
    let mut updated = draft_from(&memory);
    updated.superseded_by = Some(replacement.id);
    updated.mutation_id = format!("supersede-{}", uuid::Uuid::new_v4());
    owner
        .publish_memory(Some(id), memory.revision, updated)
        .unwrap();
}

fn seed_rel_entities(owner: &mut Cva, _memory_id: MemoryId) {
    for value in 0_u8..=9 {
        owner
            .publish_entity(Some(EntityId([value; 32])), 0, entity_draft(value))
            .unwrap();
    }
}

fn seed_phy_entities(owner: &mut Phylactery, _memory_id: MemoryId) {
    for value in 0_u8..=9 {
        owner
            .publish_entity(Some(EntityId([value; 32])), 0, entity_draft(value))
            .unwrap();
    }
}

fn entity_draft(value: u8) -> EntityDraft {
    EntityDraft {
        canonical_name: format!("Entity {value}"),
        aliases: vec![format!("entity-{value}")],
        kind: "test".into(),
        summary: format!("Entity resolution test referent {value}."),
        mutation_id: format!("entity-resolution-seed-{value}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn install_mentions(
    memories: &mut crate::memory_store::MemoryStore,
    container: &mut crate::Container,
    memory_id: MemoryId,
    body_id: crate::MemoryBodyId,
) -> Vec<MemoryEntityMentionKey> {
    let content = draft().content;
    let mut mentions = Vec::new();
    for text in ["Alpha", "Beta", "Gamma"] {
        let start = content.find(text).unwrap() as u32;
        mentions.push(MemoryEntityMention {
            field: MemoryTextField::Content,
            start_byte: start,
            end_byte: start + text.len() as u32,
            text: text.into(),
        });
    }
    memories
        .put_routing_metadata(
            container,
            MemoryRoutingMetadata {
                memory_id,
                body_id,
                entity_mentions: mentions.clone(),
            },
        )
        .unwrap();
    mentions
        .into_iter()
        .map(|mention| MemoryEntityMentionKey {
            memory_id,
            field: mention.field,
            start_byte: mention.start_byte,
            end_byte: mention.end_byte,
        })
        .collect()
}

fn draft() -> MemoryDraft {
    let token = uuid::Uuid::new_v4();
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Entity resolution".into(),
        content: "Alpha works with Beta and Gamma.".into(),
        scope: "project".into(),
        lifecycle_state: "extracted".into(),
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
        mutation_id: format!("entity-resolution-{token}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn draft_from(memory: &crate::Memory) -> MemoryDraft {
    MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: memory.lifecycle_state.clone(),
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
        mutation_id: memory.mutation_id.clone(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns,
    }
}

fn temp_path(ext: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "reliquary-entity-resolution-{}",
            uuid::Uuid::new_v4()
        ))
        .with_extension(ext)
}
