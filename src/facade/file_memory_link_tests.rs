use crate::{
    ArchiveError, Cva, CvaError, FileId, IncomingAttachment, IncomingTurn, MemoryDraft, MemoryId,
};
use std::fs;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-file-memory-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "reference".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: "Framing reference".into(),
        content: "Use the attached framing plan.".into(),
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
        mutation_id: "file-memory-link".into(),
        created_at_ns: 2,
        updated_at_ns: 2,
    }
}

fn ingest_file(cva: &mut Cva) -> FileId {
    cva.ingest_turn(IncomingTurn {
        id: "turn-1".into(),
        conversation_id: "conversation-1".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 1,
        content: "attached".into(),
        attachments: vec![IncomingAttachment {
            filename: "framing-plan.pdf".into(),
            mime_type: Some("application/pdf".into()),
            bytes: b"pdf bytes".to_vec(),
        }],
        project_attachments: Vec::new(),
    })
    .unwrap()
    .attachments[0]
        .id
}

#[test]
fn file_memory_links_are_explicit_idempotent_and_reopenable() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    let file_id = ingest_file(&mut cva);
    let (memory, created) = cva.publish_memory(None, 0, memory_draft()).unwrap();
    assert!(created);

    let link = cva.link_file_to_memory(file_id, memory.id).unwrap();
    let archive_version = cva.archive_version();
    assert_eq!(cva.link_file_to_memory(file_id, memory.id).unwrap(), link);
    assert_eq!(cva.archive_version(), archive_version);
    assert_eq!(cva.stats().file_memory_links, 1);
    assert_eq!(cva.file_memory_links(file_id), vec![link]);
    assert_eq!(
        cva.files_for_memory(memory.id),
        vec![cva.file(file_id).unwrap().clone()]
    );

    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.stats().file_memory_links, 1);
    assert_eq!(reopened.file_memory_links(file_id), vec![link]);
    assert_eq!(reopened.files_for_memory(memory.id).len(), 1);
}

#[test]
fn file_memory_links_reject_missing_files_and_memories() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    let file_id = ingest_file(&mut cva);

    assert!(matches!(
        cva.link_file_to_memory(file_id, MemoryId([9; 32])),
        Err(ArchiveError::MissingFileMemoryTarget)
    ));
    let (memory, _) = cva.publish_memory(None, 0, memory_draft()).unwrap();
    assert!(matches!(
        cva.link_file_to_memory(FileId([7; 32]), memory.id),
        Err(ArchiveError::MissingFile)
    ));
    assert_eq!(cva.stats().file_memory_links, 0);
}

#[test]
fn reopen_rejects_file_link_to_missing_memory() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    let file_id = ingest_file(&mut cva);
    cva.archive
        .link_file_to_memory(&mut cva.container, file_id, MemoryId([6; 32]))
        .unwrap();
    cva.sync().unwrap();
    drop(cva);

    assert!(matches!(
        Cva::open(path),
        Err(CvaError::Archive(ArchiveError::MissingFileMemoryTarget))
    ));
}
