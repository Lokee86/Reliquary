use crate::{
    Cva, MemoryBodyId, MemoryDraft, MemoryEntityMention, MemoryError, MemoryId,
    MemoryRoutingMetadata, MemoryTextField,
};
use std::fs;

#[test]
fn routing_metadata_is_clock_neutral_body_bound_and_reopens() {
    let dir = std::env::temp_dir().join(format!("reliquary-routing-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("routing.rel");
    let mut cva = Cva::create_project(&path).unwrap();
    let (memory, created) = cva.publish_memory(None, 0, draft()).unwrap();
    assert!(created);
    let version = cva.latest_global_version();
    let start = memory.content.find("Vancouver office").unwrap();
    let metadata = MemoryRoutingMetadata {
        memory_id: memory.id,
        body_id: cva.memory_body_id(memory.id).unwrap(),
        entity_mentions: vec![MemoryEntityMention {
            field: MemoryTextField::Content,
            start_byte: start as u32,
            end_byte: (start + "Vancouver office".len()) as u32,
            text: "Vancouver office".into(),
        }],
    };

    assert!(
        cva.owner
            .memories
            .put_routing_metadata(&mut cva.owner.container, metadata.clone())
            .unwrap()
    );
    assert!(
        !cva.owner
            .memories
            .put_routing_metadata(&mut cva.owner.container, metadata.clone())
            .unwrap()
    );
    assert_eq!(cva.latest_global_version(), version);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    let memory = reopened.memory(metadata.memory_id).unwrap();
    assert_eq!(memory.routing_metadata.as_ref(), Some(&metadata));
    assert_eq!(
        reopened.memory_routing_metadata(metadata.memory_id),
        Some(&metadata)
    );

    let mut conflicting = metadata.clone();
    conflicting.entity_mentions.push(MemoryEntityMention {
        field: MemoryTextField::Title,
        start_byte: 0,
        end_byte: "Reliquary".len() as u32,
        text: "Reliquary".into(),
    });
    assert!(matches!(
        reopened
            .owner
            .memories
            .put_routing_metadata(&mut reopened.owner.container, conflicting),
        Err(MemoryError::RoutingMetadataConflict)
    ));
}

fn draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Reliquary deployment".into(),
        content: "Reliquary is used by the Vancouver office.".into(),
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
        mutation_id: "routing-metadata-test".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}
