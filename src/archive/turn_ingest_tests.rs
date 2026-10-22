use crate::turn_ingest_codec::{decode_ingested_turn, encode_ingested_turn};
use crate::{
    ArchiveError, ContentId, Cva, FileId, IncomingAttachment, IncomingTurn, IngestedTurn, Node,
    StoredFile,
};
use std::fs;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-turn-ingest-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

fn turn() -> IncomingTurn {
    IncomingTurn {
        id: "turn-1".into(),
        conversation_id: "conversation-1".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 1,
        content: "See the attached framing documents.".into(),
        attachments: vec![
            IncomingAttachment {
                filename: "framing-plan.pdf".into(),
                mime_type: Some("application/pdf".into()),
                bytes: b"pdf bytes".to_vec(),
            },
            IncomingAttachment {
                filename: "takeoff.xlsx".into(),
                mime_type: Some(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
                ),
                bytes: b"sheet bytes".to_vec(),
            },
        ],
        project_attachments: Vec::new(),
    }
}

#[test]
fn turn_ingestion_publishes_node_and_attachments_together() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();

    let ingested = cva.ingest_turn(turn()).unwrap();
    assert_eq!(ingested.node.id, "turn-1");
    assert_eq!(ingested.attachments.len(), 2);
    assert_eq!(cva.archive_version(), 1);
    assert_eq!(cva.record_versions().len(), 1);
    assert_eq!(cva.stats().nodes, 1);
    assert_eq!(cva.stats().files, 2);
    assert_eq!(cva.stats().source_attachments, 2);
    assert_eq!(cva.stats().content_objects, 3);
    assert_eq!(
        cva.files_for_source("conversation-1", "turn-1"),
        ingested.attachments
    );

    let hits = cva.search_files("framing pdf", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].file.filename, "framing-plan.pdf");
    assert_eq!(
        cva.file_bytes(ingested.attachments[0].id).unwrap(),
        b"pdf bytes"
    );

    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.archive_version(), 1);
    assert_eq!(reopened.stats().nodes, 1);
    assert_eq!(reopened.stats().files, 2);
    assert_eq!(reopened.stats().source_attachments, 2);
    let attachments = reopened.files_for_source("conversation-1", "turn-1");
    assert_eq!(attachments.len(), 2);
    assert_eq!(
        reopened.file_bytes(attachments[1].id).unwrap(),
        b"sheet bytes"
    );
}

#[test]
fn turn_ingestion_is_idempotent_and_rejects_attachment_drift() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    let incoming = turn();

    let first = cva.ingest_turn(incoming.clone()).unwrap();
    let version = cva.archive_version();
    let same = cva.ingest_turn(incoming.clone()).unwrap();
    assert_eq!(same, first);
    assert_eq!(cva.archive_version(), version);

    let mut changed = incoming;
    changed.attachments[0].filename = "different-name.pdf".into();
    assert!(matches!(
        cva.ingest_turn(changed),
        Err(ArchiveError::ConflictingTurnIngest)
    ));
    assert_eq!(cva.archive_version(), version);
}

#[test]
fn turn_ingestion_rejects_duplicate_attachment_identity() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    let mut incoming = turn();
    incoming.attachments.push(incoming.attachments[0].clone());

    assert!(matches!(
        cva.ingest_turn(incoming),
        Err(ArchiveError::DuplicateTurnAttachment)
    ));
    assert_eq!(cva.archive_version(), 0);
    assert_eq!(cva.stats().nodes, 0);
    assert_eq!(cva.stats().files, 0);
}

#[test]
fn unversioned_ingested_turn_is_inert_on_reopen() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    let published = cva
        .append_node("n1".into(), "c1".into(), None, "user".into(), 1, "body")
        .unwrap();
    let orphan = IngestedTurn {
        node: Node {
            id: "orphan".into(),
            conversation_id: "c1".into(),
            parent_id: Some("n1".into()),
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 2,
            content_id: published.content_id,
        },
        attachments: vec![StoredFile {
            id: FileId([7; 32]),
            content_id: ContentId([8; 32]),
            filename: "orphan.pdf".into(),
            mime_type: Some("application/pdf".into()),
            byte_length: 10,
        }],
    };
    cva.container
        .append(&encode_ingested_turn(&orphan).unwrap())
        .unwrap();
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.stats().nodes, 1);
    assert_eq!(reopened.stats().files, 0);
    assert_eq!(reopened.stats().source_attachments, 0);
}
