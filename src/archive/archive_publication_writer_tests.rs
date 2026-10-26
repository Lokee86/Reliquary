use crate::*;

#[test]
fn ordinary_writes_emit_one_native_publication_and_retries_append_nothing() {
    let dir = std::env::temp_dir().join(format!("archive-native-writer-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("writer.rel");
    let mut owner = Cva::create_project(&path).unwrap();
    let before = owner.container.chunks().unwrap().len();
    owner
        .append_node(
            "n".into(),
            "c".into(),
            None,
            "user".into(),
            -7,
            "first body",
        )
        .unwrap();
    assert_eq!(
        owner.container.chunks().unwrap().len() - before,
        2,
        "one content frame and one publication"
    );
    let branch = Branch {
        id: "b".into(),
        conversation_id: "c".into(),
        leaf_node_id: "n".into(),
        canonical: true,
    };
    let before = owner.container.chunks().unwrap().len();
    owner.append_branch(branch.clone()).unwrap();
    assert_eq!(owner.container.chunks().unwrap().len() - before, 1);
    let before = owner.container.chunks().unwrap().len();
    owner
        .append_node(
            "n".into(),
            "c".into(),
            None,
            "user".into(),
            -7,
            "first body",
        )
        .unwrap();
    owner.append_branch(branch).unwrap();
    assert_eq!(owner.container.chunks().unwrap().len(), before);
    let file = owner
        .store_file("f.txt".into(), None, b"attachment")
        .unwrap();
    let before = owner.container.chunks().unwrap().len();
    assert_eq!(
        owner
            .store_file("f.txt".into(), None, b"attachment")
            .unwrap(),
        file
    );
    assert_eq!(owner.container.chunks().unwrap().len(), before);
    owner
        .ingest_turn(IncomingTurn {
            id: "n2".into(),
            conversation_id: "c".into(),
            parent_id: Some("n".into()),
            role: "assistant".into(),
            principal_id: None,
            timestamp_ns: 8,
            content: "turn body".into(),
            attachments: vec![IncomingAttachment {
                filename: "f.txt".into(),
                mime_type: None,
                bytes: b"attachment".to_vec(),
            }],
            project_attachments: vec![],
        })
        .unwrap();
    for version in owner.archive.record_versions().to_vec() {
        let payload = owner.container.read(version.record).unwrap();
        let envelope = reliquary_container::publication_envelope::decode(&payload)
            .unwrap()
            .expect("native publication");
        assert_eq!(envelope.global_version, version.global_version);
        assert!(matches!(
            envelope.clock,
            reliquary_container::publication_envelope::PublicationClock::Allocate { .. }
        ));
        let body =
            crate::archive_codec::publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .unwrap();
        assert_eq!(body.archive_version(), version.archive_version);
    }
    for object in owner.container.chunks().unwrap() {
        let bytes = owner.container.read(object).unwrap();
        assert!(!bytes.starts_with(b"CVAAREC1"));
        assert!(!bytes.starts_with(b"CVAVERS"));
        assert!(!bytes.starts_with(b"CVANODE"));
        assert!(!bytes.starts_with(b"CVATURN"));
    }
    let head = owner.archive_version();
    owner.sync().unwrap();
    drop(owner);
    let reopened = Cva::open_project(&path).unwrap();
    assert_eq!(reopened.archive_version(), head);
    assert_eq!(reopened.files_for_source("c", "n2"), vec![file]);
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clock_exhaustion_rejects_before_content_or_publication_append() {
    for local_exhaustion in [true, false] {
        let dir =
            std::env::temp_dir().join(format!("archive-native-overflow-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("writer.rel");
        let mut owner = Cva::create_project(&path).unwrap();
        owner.sync().unwrap();
        let before = std::fs::read(&path).unwrap();
        let chunks = owner.container.chunks().unwrap();
        if local_exhaustion {
            owner.archive.next_archive_version = u64::MAX;
        } else {
            let start = owner.container.next_version_candidate();
            owner
                .container
                .commit_embedded_version_range_at(
                    start,
                    usize::try_from(u64::MAX - start).unwrap(),
                    None,
                )
                .unwrap();
        }
        let global = owner.latest_global_version();
        assert!(
            owner
                .append_node(
                    "n".into(),
                    "c".into(),
                    None,
                    "user".into(),
                    1,
                    "new content"
                )
                .is_err()
        );
        assert!(owner.store_file("f".into(), None, b"new file").is_err());
        assert!(
            owner
                .ingest_turn(IncomingTurn {
                    id: "t".into(),
                    conversation_id: "c".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: None,
                    timestamp_ns: 2,
                    content: "new turn".into(),
                    attachments: vec![IncomingAttachment {
                        filename: "a".into(),
                        mime_type: None,
                        bytes: b"new attachment".to_vec()
                    }],
                    project_attachments: vec![]
                })
                .is_err()
        );
        owner.sync().unwrap();
        assert_eq!(owner.container.chunks().unwrap(), chunks);
        assert_eq!(owner.latest_global_version(), global);
        assert!(owner.archive.record_versions().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(owner);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
