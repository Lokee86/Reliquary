use super::*;
use crate::archive_codec::publication::{self, ArchivePublication};
use crate::*;

fn standalone(record: &ArchiveRecord) -> Vec<u8> {
    use crate::archive_codec::*;
    match record {
        ArchiveRecord::Node(v) => encode_node(v).unwrap(),
        ArchiveRecord::Branch(v) => encode_branch(v).unwrap(),
        ArchiveRecord::ConversationMetadata(v) => {
            crate::conversation_metadata_codec::encode_conversation_metadata(v).unwrap()
        }
        ArchiveRecord::ConversationTombstone(v) => encode_conversation_tombstone(v).unwrap(),
        ArchiveRecord::Fragment(v) => encode_fragment(v).unwrap(),
        ArchiveRecord::Episode(v) => crate::episode_codec::encode_episode(v).unwrap(),
        ArchiveRecord::File(v) => encode_file(v).unwrap(),
        ArchiveRecord::IngestedTurn(v) => {
            crate::turn_ingest_codec::encode_ingested_turn(v).unwrap()
        }
        ArchiveRecord::ImportReceipt(v) => crate::archive_import::encode(v).unwrap(),
        ArchiveRecord::FileMemoryLink(v) => {
            crate::file_memory_link_codec::encode_file_memory_link(*v)
        }
        _ => panic!("semantic publication required"),
    }
}

#[test]
fn every_record_family_resolves_identical_evidence_at_all_retained_cuts() {
    let dir = std::env::temp_dir().join(format!("archive-reader-parity-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut legacy = Cva::create(dir.join("legacy.rel")).unwrap();
    let mut native = Cva::create(dir.join("native.rel")).unwrap();
    let mut old = ArchiveOpenState::new();
    let mut new = ArchiveOpenState::new();
    for (cva, state) in [(&mut legacy, &mut old), (&mut native, &mut new)] {
        let format = crate::archive_history_codec::encode_archive_format();
        let object = cva.container.append(&format).unwrap();
        state.ingest(object, &format, 0).unwrap();
        let bytes = crate::archive_codec::encode_content(
            crate::archive_store::hash_content(b"text"),
            b"text",
        )
        .unwrap();
        let object = cva.container.append(&bytes).unwrap();
        state.ingest(object, &bytes, 0).unwrap();
    }
    let content_id = crate::archive_store::hash_content(b"text");
    let node = Node {
        id: "n".into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: Some("principal".into()),
        timestamp_ns: -7,
        content_id,
    };
    let file = StoredFile {
        id: FileId([3; 32]),
        content_id,
        filename: "f.txt".into(),
        mime_type: Some("text/plain".into()),
        byte_length: 4,
    };
    let mut turn_node = node.clone();
    turn_node.id = "n2".into();
    turn_node.parent_id = Some("n".into());
    turn_node.timestamp_ns = 8;
    let records = vec![
        ArchiveRecord::Node(node),
        ArchiveRecord::ConversationMetadata(ConversationMetadata {
            conversation_id: "c".into(),
            title: Some("title".into()),
            active: true,
            policy: ConversationPolicy::VisibleAutoBranch,
            policy_revision: 2,
        }),
        ArchiveRecord::Branch(Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: true,
        }),
        ArchiveRecord::Fragment(Fragment {
            id: FragmentId([1; 32]),
            conversation_id: "c".into(),
            start_node_id: "n".into(),
            end_node_id: "n".into(),
        }),
        ArchiveRecord::Episode(Episode {
            id: EpisodeId([2; 32]),
            conversation_id: "c".into(),
            start_node_id: "n".into(),
            end_node_id: "n".into(),
            origin: EpisodeOrigin::Import,
            boundary: EpisodeBoundary::ImportEnd,
            source_through_ns: -7,
            finalized_at_ns: 12,
        }),
        ArchiveRecord::File(file.clone()),
        ArchiveRecord::IngestedTurn(IngestedTurn {
            node: turn_node,
            attachments: vec![file],
        }),
        ArchiveRecord::ImportReceipt(crate::imports::ImportReceipt {
            schema_version: 1,
            import_key: "ab".repeat(32),
            source_namespace: "account".into(),
            source_format: "json".into(),
            conversations: vec![],
        }),
        ArchiveRecord::FileMemoryLink(FileMemoryLink {
            file_id: FileId([3; 32]),
            memory_id: MemoryId([4; 32]),
        }),
    ];
    for record in records {
        append_pair(&mut legacy, &mut native, &mut old, &mut new, record);
    }
    let terminal = ArchiveRecord::ConversationTombstone(ConversationTombstone {
        conversation_id: "c".into(),
        archive_version: 10,
        global_version: 10,
        closure: old.discover_conversation_tombstone_closure("c"),
    });
    append_pair(&mut legacy, &mut native, &mut old, &mut new, terminal);
    legacy
        .container
        .set_next_transaction_time_override(Some(300));
    native
        .container
        .set_next_transaction_time_override(Some(300));
    let global = legacy.container.allocate_version().unwrap();
    let marker = crate::archive_history_codec::encode_record_version(ArchiveRecordVersion {
        archive_version: 11,
        global_version: global,
        record: old.record_versions[0].record,
    });
    let object = legacy.container.append(&marker).unwrap();
    old.ingest(object, &marker, global).unwrap();
    let repeat = publication::encode(&ArchivePublication::Repeat {
        archive_version: 11,
        first_applying_archive_version: 1,
    })
    .unwrap();
    let (global, object) = native
        .container
        .append_publication_envelope(&repeat)
        .unwrap();
    new.ingest(object, &native.container.read(object).unwrap(), global)
        .unwrap();
    let old = old.finish().unwrap();
    let new = new.finish().unwrap();
    assert_eq!(old.archive_version(), new.archive_version());
    for cut in 1..=11 {
        let a = *old.record_version(cut).unwrap();
        let b = *new.record_version(cut).unwrap();
        let (a_record, a_applied) = old.read_publication(&mut legacy.container, a).unwrap();
        let (b_record, b_applied) = new.read_publication(&mut native.container, b).unwrap();
        assert_eq!(
            standalone(&a_record),
            standalone(&b_record),
            "local cut {cut}"
        );
        assert_eq!(
            publication::import_evidence(a_record).unwrap(),
            publication::import_evidence(b_record).unwrap()
        );
        assert_eq!(a_applied.archive_version, b_applied.archive_version);
        assert_eq!(a_applied.global_version, b_applied.global_version);
        assert_eq!(
            legacy.container.transaction_time_ns(a.global_version),
            native.container.transaction_time_ns(b.global_version)
        );
        let time = legacy
            .container
            .transaction_time_ns(a.global_version)
            .unwrap();
        assert_eq!(
            legacy.container.version_at_or_before(time),
            native.container.version_at_or_before(time)
        );
        assert_eq!(
            old.branch_at(&mut legacy.container, "c", "b", cut).unwrap(),
            new.branch_at(&mut native.container, "c", "b", cut).unwrap()
        );
    }
    assert_eq!(old.conversation_tombstones, new.conversation_tombstones);
    let lengths = std::collections::HashMap::from([(content_id, 4)]);
    assert_eq!(
        old.retention_descriptor(&mut legacy.container, "c", 11, &lengths)
            .unwrap(),
        new.retention_descriptor(&mut native.container, "c", 11, &lengths)
            .unwrap()
    );
    for watermark in 0..=11 {
        let graph = crate::graph_store::GraphStore::empty();
        let legacy_digest = crate::owner_maintenance::owner_history_digest(
            &mut legacy.container,
            watermark,
            &graph,
        )
        .unwrap();
        let native_digest = crate::owner_maintenance::owner_history_digest(
            &mut native.container,
            watermark,
            &graph,
        )
        .unwrap();
        assert_eq!(legacy_digest, native_digest, "proof watermark {watermark}");
    }
    assert_eq!(
        old.nodes.iter().collect::<Vec<_>>(),
        new.nodes.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.branches.iter().collect::<Vec<_>>(),
        new.branches.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.conversations.iter().collect::<Vec<_>>(),
        new.conversations.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.fragments.iter().collect::<Vec<_>>(),
        new.fragments.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.episodes.iter().collect::<Vec<_>>(),
        new.episodes.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.files.iter().collect::<Vec<_>>(),
        new.files.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.file_memory_links.iter().collect::<Vec<_>>(),
        new.file_memory_links.iter().collect::<Vec<_>>()
    );
    assert_eq!(old.import_receipts, new.import_receipts);
    assert_eq!(
        old.source_attachments.conversation_file_ids("c"),
        new.source_attachments.conversation_file_ids("c")
    );
    assert_eq!(
        old.content_bytes(&mut legacy.container, content_id)
            .unwrap(),
        b"text"
    );
    assert_eq!(
        new.content_bytes(&mut native.container, content_id)
            .unwrap(),
        b"text"
    );
    drop(legacy);
    drop(native);
    std::fs::remove_dir_all(dir).unwrap();
}

fn append_pair(
    legacy: &mut Cva,
    native: &mut Cva,
    old: &mut ArchiveOpenState,
    new: &mut ArchiveOpenState,
    record: ArchiveRecord,
) {
    let local = old.next_archive_version;
    let time = Some(local as i64 * 100 - 900);
    legacy.container.set_next_transaction_time_override(time);
    native.container.set_next_transaction_time_override(time);
    let bytes = standalone(&record);
    let object = legacy.container.append(&bytes).unwrap();
    old.ingest(object, &bytes, local - 1).unwrap();
    let global = legacy.container.allocate_version().unwrap();
    let version = ArchiveRecordVersion {
        archive_version: local,
        global_version: global,
        record: object,
    };
    let marker = crate::archive_history_codec::encode_record_version(version);
    let marker_object = legacy.container.append(&marker).unwrap();
    old.ingest(marker_object, &marker, global).unwrap();
    let body = publication::encode(&ArchivePublication::Record {
        archive_version: local,
        record,
    })
    .unwrap();
    let (native_global, object) = native.container.append_publication_envelope(&body).unwrap();
    assert_eq!(native_global, global);
    let payload = native.container.read(object).unwrap();
    assert_eq!(
        crate::storage_reclamation::relocate_payload(&payload, &std::collections::BTreeMap::new())
            .unwrap(),
        payload
    );
    new.ingest(object, &payload, native_global).unwrap();
}
