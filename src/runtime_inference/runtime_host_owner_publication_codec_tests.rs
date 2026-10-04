use super::*;
use crate::insomnia::PreparedMemory;
use crate::{EpisodeId, MemorySourceRef};

fn prepared() -> (PreparedMemory, MemorySourceRef) {
    let draft = MemoryDraft {
        category: "preference".into(),
        memory_type: "fact".into(),
        authority_kind: "explicit".into(),
        temporal_status: "current".into(),
        title: "Tea".into(),
        content: "Likes green tea".into(),
        scope: "user".into(),
        lifecycle_state: "active".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: Some("turn-1".into()),
        content_source_conversation_id: Some("conversation-1".into()),
        content_source_node_id: Some("turn-1".into()),
        grounding_source_conversation_id: Some("conversation-1".into()),
        grounding_source_node_id: Some("turn-1".into()),
        source_episode_id: Some(EpisodeId([4; 32])),
        source_time_ns: Some(123),
        mutation_id: "episode-1/candidate-1".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    };
    let id = memory_model::memory_id(&draft.mutation_id);
    let body_id = memory_model::memory_body_id(&draft.title, &draft.content);
    let routing = MemoryRoutingMetadata {
        memory_id: id,
        body_id,
        entity_mentions: vec![],
    };
    let temporal = crate::insomnia::temporal::assess_draft(&draft);
    let source_ref = MemorySourceRef {
        owner_id: "rel-00000000-0000-0000-0000-000000000001".into(),
        principal_id: Some("phy-00000000-0000-0000-0000-000000000002".into()),
        source_episode_id: EpisodeId([4; 32]),
        source_node_id: "turn-1".into(),
        content_source_conversation_id: Some("conversation-1".into()),
        content_source_node_id: Some("turn-1".into()),
        grounding_source_conversation_id: Some("conversation-1".into()),
        grounding_source_node_id: Some("turn-1".into()),
    };
    (
        PreparedMemory {
            draft,
            temporal,
            temporal_inference: None,
            routing_metadata: Some(routing),
        },
        source_ref,
    )
}

fn memory_from(prepared: &PreparedMemory, source_ref: Option<MemorySourceRef>) -> Memory {
    let id = memory_model::memory_id(&prepared.draft.mutation_id);
    Memory {
        id,
        revision: 1,
        category: prepared.draft.category.clone(),
        memory_type: prepared.draft.memory_type.clone(),
        authority_kind: prepared.draft.authority_kind.clone(),
        temporal_status: prepared.draft.temporal_status.clone(),
        title: prepared.draft.title.clone(),
        content: prepared.draft.content.clone(),
        scope: prepared.draft.scope.clone(),
        lifecycle_state: prepared.draft.lifecycle_state.clone(),
        archived: prepared.draft.archived,
        superseded_by: prepared.draft.superseded_by,
        parent_id: prepared.draft.parent_id,
        source_node_id: prepared.draft.source_node_id.clone(),
        content_source_conversation_id: prepared.draft.content_source_conversation_id.clone(),
        content_source_node_id: prepared.draft.content_source_node_id.clone(),
        grounding_source_conversation_id: prepared.draft.grounding_source_conversation_id.clone(),
        grounding_source_node_id: prepared.draft.grounding_source_node_id.clone(),
        source_episode_id: prepared.draft.source_episode_id,
        source_time_ns: prepared.draft.source_time_ns,
        source_ref,
        temporal_inference: None,
        routing_metadata: prepared.routing_metadata.clone(),
        mutation_id: prepared.draft.mutation_id.clone(),
        created_at_ns: prepared.draft.created_at_ns,
        updated_at_ns: prepared.draft.updated_at_ns,
        global_version: 81,
        memory_version: 37,
    }
}

#[test]
fn freezes_and_thaws_exact_prepared_memory_and_verifies_global_versions_away() {
    let (original, source_ref) = prepared();
    let frozen = freeze_memory(&original, Some(&source_ref)).unwrap();
    let (thawed, thawed_source) = thaw_memory(&frozen).unwrap();
    assert_eq!(thawed.draft, original.draft);
    assert_eq!(thawed.temporal_inference, original.temporal_inference);
    assert_eq!(thawed.routing_metadata, original.routing_metadata);
    assert_eq!(thawed_source, Some(source_ref.clone()));
    verify_memory(&frozen, &memory_from(&thawed, thawed_source)).unwrap();
}

#[test]
fn rejects_envelope_and_record_tampering() {
    let (original, source_ref) = prepared();
    let frozen = freeze_memory(&original, Some(&source_ref)).unwrap();

    let mut changed = frozen.clone();
    changed
        .prepared_memory
        .as_object_mut()
        .unwrap()
        .insert("extra".into(), Value::Bool(true));
    assert!(thaw_memory(&changed).is_err());

    let mut changed = frozen.clone();
    changed.prepared_memory["title"] = Value::String("Altered title".into());
    assert!(thaw_memory(&changed).is_err());

    let mut changed = frozen;
    let bytes = bytes_field(&changed.prepared_memory["record"]).unwrap();
    let mut record = crate::memory_codec::decode_record(&bytes).unwrap().unwrap();
    record.revision = 2;
    changed.prepared_memory["record"] = json!(crate::memory_codec::encode_record(&record).unwrap());
    assert!(thaw_memory(&changed).is_err());
}

#[test]
fn record_verification_precedes_attachment_and_full_verification_requires_it() {
    let (original, source_ref) = prepared();
    let frozen = freeze_memory(&original, Some(&source_ref)).unwrap();
    let mut memory = memory_from(&original, Some(source_ref));
    memory.routing_metadata = None;
    verify_memory_record(&frozen, &memory).unwrap();
    assert!(verify_memory(&frozen, &memory).is_err());
    memory.routing_metadata = original.routing_metadata.clone();
    verify_memory(&frozen, &memory).unwrap();
    memory.source_ref.as_mut().unwrap().principal_id = Some("another-principal".into());
    assert!(verify_memory_record(&frozen, &memory).is_err());
}

#[test]
fn verification_rejects_changed_metadata_and_noninitial_revision() {
    let (original, source_ref) = prepared();
    let frozen = freeze_memory(&original, Some(&source_ref)).unwrap();
    let (thawed, source) = thaw_memory(&frozen).unwrap();
    let mut memory = memory_from(&thawed, source);
    memory.category = "different".into();
    assert!(verify_memory(&frozen, &memory).is_err());
    memory.category = thawed.draft.category.clone();
    memory.revision = 2;
    assert!(verify_memory(&frozen, &memory).is_err());
}
