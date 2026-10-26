use crate::archive_codec::ArchiveRecord;
use crate::archive_codec::publication::{self, ArchivePublication};
use crate::archive_history_codec::encode_archive_format;
use crate::archive_rebuild::ArchiveOpenState;
use crate::{Archive, ArchiveError, ImportReceipt, ImportedConversation, ImportedMessage};
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};
use reliquary_container::{Container, ObjectRef};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-output");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}-{}.rel", uuid::Uuid::new_v4()))
}

fn receipt() -> ImportReceipt {
    ImportReceipt {
        schema_version: 1,
        import_key: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        source_namespace: "provider/account".into(),
        source_format: "fixture-v1".into(),
        conversations: vec![ImportedConversation {
            source_id: "source-conversation".into(),
            conversation_id: "conversation".into(),
            title: Some("Fixture".into()),
            messages: vec![ImportedMessage {
                source_id: "source-message".into(),
                revision: "r1".into(),
                source_parent_id: None,
                replaces: None,
                deleted: false,
                node_id: Some("node".into()),
                fingerprint: "sha256:fixture".into(),
                artifacts: vec![],
            }],
            branches: vec![],
        }],
    }
}

fn append_record(
    container: &mut Container,
    archive_version: u64,
    record: ArchiveRecord,
) -> (u64, ObjectRef) {
    container
        .append_publication_envelope(
            &publication::encode(&ArchivePublication::Record {
                archive_version,
                record,
            })
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn replay_is_read_only_and_preserves_import_receipt_idempotence() {
    let path = fixture_path("archive-replay-read-only");
    let mut container = Container::create(&path).unwrap();
    let archive = Archive::empty();
    archive.initialize_history_format(&mut container).unwrap();
    let receipt = receipt();
    let (_, first) = append_record(
        &mut container,
        1,
        ArchiveRecord::ImportReceipt(receipt.clone()),
    );
    let (_, second) = append_record(
        &mut container,
        2,
        ArchiveRecord::ImportReceipt(receipt.clone()),
    );
    container.sync().unwrap();
    let before = std::fs::read(&path).unwrap();

    let mut state = ArchiveOpenState::new();
    let chunks = container.chunk_refs().unwrap();
    for chunk in chunks {
        let payload = container.read(chunk).unwrap();
        state.ingest(chunk, &payload, 2).unwrap();
    }
    let replayed = state.finish().unwrap();

    assert_eq!(replayed.archive_version(), 2);
    assert_eq!(
        replayed.import_receipts.get(&receipt.import_key),
        Some(&receipt)
    );
    assert_eq!(replayed.record_versions().len(), 2);
    assert_eq!(replayed.record_versions()[0].record, first);
    assert_eq!(replayed.record_versions()[1].record, second);
    assert_eq!(std::fs::read(&path).unwrap(), before);

    drop(container);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn replay_rejects_local_version_gap_without_advancing_state() {
    let path = fixture_path("archive-replay-version-order");
    let mut container = Container::create(&path).unwrap();
    container.append(&encode_archive_format()).unwrap();
    let object = container.append(b"test record frame").unwrap();
    let gap_body = publication::encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::ImportReceipt(receipt()),
    })
    .unwrap();
    let mut state = ArchiveOpenState::new();
    let format_object = container.chunk_refs().unwrap()[0];
    let format_payload = container.read(format_object).unwrap();
    state.ingest(format_object, &format_payload, 2).unwrap();
    let result = state.ingest_publication_envelope(
        object,
        PublicationEnvelope {
            global_version: 2,
            clock: PublicationClock::BindReserved,
            body: &gap_body,
        },
        2,
    );
    assert!(matches!(
        result,
        Err(ArchiveError::InvalidArchiveRecordVersion)
    ));
    let valid_body = publication::encode(&ArchivePublication::Record {
        archive_version: 1,
        record: ArchiveRecord::ImportReceipt(receipt()),
    })
    .unwrap();
    state
        .ingest_publication_envelope(
            object,
            PublicationEnvelope {
                global_version: 1,
                clock: PublicationClock::BindReserved,
                body: &valid_body,
            },
            2,
        )
        .unwrap();
    assert_eq!(state.finish().unwrap().archive_version(), 1);

    drop(container);
    std::fs::remove_file(path).unwrap();
}
