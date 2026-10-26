use super::*;
use reliquary_archive::archive_codec::ArchiveRecord;
use reliquary_archive::archive_codec::publication::{self, ArchivePublication};

#[test]
fn native_node_and_terminal_prune_only_payload_and_reopen_with_bound_proof() {
    let dir = std::env::temp_dir().join(format!("archive-envelope-prune-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let source_path = dir.join("source.rel");
    let output_path = dir.join("pruned.rel");
    let text = "private native Archive payload ".repeat(2000);
    let content_id = reliquary_archive::archive_store::hash_content(text.as_bytes());
    let mut cva = crate::Cva::create_project(&source_path).unwrap();
    cva.archive
        .put_content_bytes(&mut cva.container, content_id, text.as_bytes())
        .unwrap();
    let node = publication::encode(&ArchivePublication::Record {
        archive_version: 1,
        record: ArchiveRecord::Node(crate::Node {
            id: "n".into(),
            conversation_id: "c".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 1,
            content_id,
        }),
    })
    .unwrap();
    cva.container.append_publication_envelope(&node).unwrap();
    let terminal = publication::encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::ConversationTombstone(crate::ConversationTombstone {
            conversation_id: "c".into(),
            archive_version: 2,
            global_version: 999,
            closure: crate::ConversationTombstoneClosure {
                node_ids: vec!["n".into()],
                branch_ids: vec![],
                fragment_ids: vec![],
                episode_ids: vec![],
                file_ids: vec![],
                metadata_archive_versions: vec![],
            },
        }),
    })
    .unwrap();
    cva.container
        .append_publication_envelope(&terminal)
        .unwrap();
    let orphan_bytes = b"unpublished orphan content".repeat(1000);
    let orphan_id = reliquary_archive::archive_store::hash_content(&orphan_bytes);
    cva.container
        .append(
            &reliquary_archive::archive_codec::encode_content(orphan_id, &orphan_bytes).unwrap(),
        )
        .unwrap();
    cva.container
        .append(
            &publication::encode(&ArchivePublication::Record {
                archive_version: 1000,
                record: ArchiveRecord::Node(crate::Node {
                    id: "orphan".into(),
                    conversation_id: "unused".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: 2,
                    content_id: orphan_id,
                }),
            })
            .unwrap(),
        )
        .unwrap();
    cva.sync().unwrap();
    drop(cva);
    let before = std::fs::read(&source_path).unwrap();
    let mut source = crate::Cva::open(&source_path).unwrap();
    let times = (1..=2)
        .map(|v| source.transaction_time_ns(v))
        .collect::<Vec<_>>();
    let reclaimed_path = dir.join("reclaimed.rel");
    let report = source.reclaim_storage(&reclaimed_path).unwrap();
    assert!(report.unpublished_backing_chunks >= 1);
    assert!(report.orphan_content_chunks >= 1);
    let mut reclaimed = crate::Cva::open(&reclaimed_path).unwrap();
    assert_eq!(reclaimed.archive_version(), 2);
    assert_eq!(
        reclaimed
            .archive
            .content_bytes(&mut reclaimed.container, content_id)
            .unwrap(),
        text.as_bytes()
    );
    assert_eq!(
        (1..=2)
            .map(|v| reclaimed.transaction_time_ns(v))
            .collect::<Vec<_>>(),
        times
    );
    drop(reclaimed);
    let identified_path = dir.join("identified.rel");
    let principal = "phy-00000000-0000-0000-0000-000000000123";
    source
        .repack_missing_user_principal(&identified_path, principal)
        .unwrap();
    let identified = crate::Cva::open(&identified_path).unwrap();
    assert_eq!(
        identified
            .archive
            .source_node("c", "n")
            .unwrap()
            .principal_id
            .as_deref(),
        Some(principal)
    );
    assert_eq!(identified.archive_version(), 2);
    assert_eq!(
        (1..=2)
            .map(|v| identified.transaction_time_ns(v))
            .collect::<Vec<_>>(),
        times
    );
    drop(identified);
    source
        .prune_tombstoned_history(
            &output_path,
            crate::HistoryPruneRequest {
                targets: vec![crate::owner_lifecycle::LifecycleTarget::Conversation(
                    "c".into(),
                )],
            },
        )
        .unwrap();
    assert_eq!(std::fs::read(&source_path).unwrap(), before);
    let mut output = crate::Cva::open(&output_path).unwrap();
    assert_eq!(output.archive_version(), 2);
    assert_eq!(
        (1..=2)
            .map(|v| output.transaction_time_ns(v))
            .collect::<Vec<_>>(),
        times
    );
    let node = output.archive.source_node("c", "n").unwrap().clone();
    assert!(matches!(
        output.archive.node_content(&mut output.container, &node),
        Err(ArchiveError::PayloadUnavailable(_))
    ));
    assert!(std::fs::metadata(&output_path).unwrap().len() < before.len() as u64);
    drop(output);
    drop(source);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_terminal_uses_envelope_coordinates_and_exact_closure() {
    for invalid in [false, true] {
        let dir = std::env::temp_dir().join(format!(
            "archive-envelope-terminal-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.rel");
        let mut cva = crate::Cva::create(&path).unwrap();
        cva.append_node("n".into(), "c".into(), None, "user".into(), 1, "content")
            .unwrap();
        let mut closure = cva.archive.discover_conversation_tombstone_closure("c");
        if invalid {
            closure.node_ids.clear();
        }
        let body = publication::encode(&ArchivePublication::Record {
            archive_version: 2,
            record: ArchiveRecord::ConversationTombstone(crate::ConversationTombstone {
                conversation_id: "c".into(),
                archive_version: 2,
                global_version: 999,
                closure,
            }),
        })
        .unwrap();
        let (global, _) = cva.container.append_publication_envelope(&body).unwrap();
        drop(cva);
        let result = crate::Cva::open(&path);
        if invalid {
            assert!(matches!(
                result,
                Err(crate::CvaError::Archive(
                    ArchiveError::ConflictingConversationTombstone
                ))
            ));
        } else {
            let opened = result.unwrap();
            assert_eq!(
                opened
                    .archive
                    .conversation_tombstone("c")
                    .unwrap()
                    .global_version,
                global
            );
            assert_eq!(opened.archive_version(), 2);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn rel_dispatch_reopens_native_records_and_repeat_at_their_own_objects() {
    let dir = std::env::temp_dir().join(format!("archive-envelope-open-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.rel");
    let mut cva = crate::Cva::create(&path).unwrap();
    cva.append_node("n".into(), "c".into(), None, "user".into(), 1, "content")
        .unwrap();
    let branch = publication::encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::Branch(crate::Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: true,
        }),
    })
    .unwrap();
    let (first_global, first_object) = cva.container.append_publication_envelope(&branch).unwrap();
    let updated = publication::encode(&ArchivePublication::Record {
        archive_version: 3,
        record: ArchiveRecord::Branch(crate::Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: false,
        }),
    })
    .unwrap();
    cva.container.append_publication_envelope(&updated).unwrap();
    let repeat = publication::encode(&ArchivePublication::Repeat {
        archive_version: 4,
        first_applying_archive_version: 2,
    })
    .unwrap();
    let (repeat_global, repeat_object) =
        cva.container.append_publication_envelope(&repeat).unwrap();
    drop(cva);
    let mut reopened = crate::Cva::open(&path).unwrap();
    assert_eq!(reopened.archive_version(), 4);
    let versions = reopened.archive.record_versions();
    assert_eq!(versions[1].global_version, first_global);
    assert_eq!(versions[1].record, first_object);
    assert_eq!(versions[3].global_version, repeat_global);
    assert_eq!(versions[3].record, repeat_object);
    let repeated = versions[3];
    let (record, applying) = reopened
        .archive
        .read_publication(&mut reopened.container, repeated)
        .unwrap();
    assert_eq!(applying.archive_version, 2);
    assert!(matches!(
        record,
        ArchiveRecord::Branch(crate::Branch {
            canonical: true,
            ..
        })
    ));
    assert!(
        reopened
            .archive
            .branch_at(&mut reopened.container, "c", "b", 2)
            .unwrap()
            .unwrap()
            .canonical
    );
    assert!(
        !reopened
            .archive
            .branch_at(&mut reopened.container, "c", "b", 4)
            .unwrap()
            .unwrap()
            .canonical
    );
    let tail = crate::cva_reconcile_archive::read_archive_tail(&mut reopened, 0).unwrap();
    let mut destination = crate::Cva::create(dir.join("destination.rel")).unwrap();
    let published =
        crate::cva_reconcile_archive::replay_archive_tail(&mut destination, &tail).unwrap();
    assert_eq!(published, 3);
    assert!(
        !destination
            .archive
            .branch_at(&mut destination.container, "c", "b", 3)
            .unwrap()
            .unwrap()
            .canonical
    );
    drop(destination);
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}
