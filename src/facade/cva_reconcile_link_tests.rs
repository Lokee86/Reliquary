use crate::{Cva, MemoryDraft};
use std::fs;
use std::path::PathBuf;

fn test_dir() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-cva-link-merge-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reconcile_replays_file_memory_links_after_targets() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    Cva::create_project(&left).unwrap().sync().unwrap();
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva
        .append_node(
            "left".into(),
            "left-c".into(),
            None,
            "user".into(),
            1,
            "left",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let mut right_cva = Cva::open(&right).unwrap();
    let file = right_cva
        .store_file(
            "invoice.pdf".into(),
            Some("application/pdf".into()),
            b"invoice",
        )
        .unwrap();
    let (memory, _) = right_cva
        .publish_memory(
            None,
            0,
            MemoryDraft {
                category: "fact".into(),
                memory_type: "project".into(),
                authority_kind: "unknown".into(),
                temporal_status: "unknown".into(),
                title: "Invoice".into(),
                content: "Invoice is attached.".into(),
                scope: "private".into(),
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
                mutation_id: "invoice-memory".into(),
                created_at_ns: 2,
                updated_at_ns: 2,
            },
        )
        .unwrap();
    right_cva.link_file_to_memory(file.id, memory.id).unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_memory_revisions, 1);
    assert_eq!(result.replayed_file_memory_links, 1);
    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.files_for_memory(memory.id), vec![file]);
}
