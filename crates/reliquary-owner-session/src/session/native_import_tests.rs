use super::*;
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory.join(format!(
            "native-import-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn turn(id: &str, parent_id: Option<&str>) -> TurnInput {
    TurnInput {
        id: id.into(),
        conversation_id: "conversation".into(),
        parent_id: parent_id.map(str::to_owned),
        role: "user".into(),
        principal_id: Some("alice".into()),
        timestamp_ns: 1,
        content: format!("content {id}"),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}

#[test]
fn import_start_uses_owned_clocks_reopens_idempotently_and_fences_ordinary_writes() {
    let path = Temp::new();
    let owner = [117; 16];
    let key = "ad".repeat(32);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let start;
    {
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        writer
            .accept_turn_and_advance_branch(
                turn("turn-1", None),
                Branch {
                    id: "main".into(),
                    conversation_id: "conversation".into(),
                    leaf_node_id: "turn-1".into(),
                    canonical: true,
                },
            )
            .unwrap();
        let status = writer
            .start_native_import(&key, "fixture/account", "schema-1", 10)
            .unwrap();
        let ImportEpochStatus::Open(accepted) = status else {
            panic!("new import must open an epoch");
        };
        assert_eq!(accepted.format_version, 2);
        assert_eq!(accepted.pre.archive_version, Some(2));
        assert_eq!(accepted.pre.memory_version, 0);
        assert_eq!(accepted.pre.entity_version, None);
        assert_eq!(accepted.pre.relationship_version, None);
        assert_eq!(accepted.pre.graph_version, 0);
        assert_eq!(accepted.pre.memory_graph_version, None);
        assert_eq!(accepted.pre.vector_version, Some(0));
        start = accepted;
        let before = writer
            .container
            .read_frames()
            .unwrap()
            .into_iter()
            .map(|frame| frame.payload)
            .collect::<Vec<_>>();
        assert!(
            writer
                .accept_turn_and_advance_branch(
                    turn("turn-2", Some("turn-1")),
                    Branch {
                        id: "main".into(),
                        conversation_id: "conversation".into(),
                        leaf_node_id: "turn-2".into(),
                        canonical: true,
                    },
                )
                .is_err()
        );
        assert_eq!(
            writer
                .container
                .read_frames()
                .unwrap()
                .into_iter()
                .map(|frame| frame.payload)
                .collect::<Vec<_>>(),
            before
        );
    }
    {
        let mut writer = RelWriter::open_path(&path.0, admission.clone(), [], []).unwrap();
        let before = writer
            .container
            .read_frames()
            .unwrap()
            .into_iter()
            .map(|frame| frame.payload)
            .collect::<Vec<_>>();
        assert_eq!(
            writer
                .start_native_import(&key, "fixture/account", "schema-1", 999)
                .unwrap(),
            ImportEpochStatus::Open(start.clone())
        );
        assert_eq!(
            writer
                .container
                .read_frames()
                .unwrap()
                .into_iter()
                .map(|frame| frame.payload)
                .collect::<Vec<_>>(),
            before
        );
        assert!(
            writer
                .start_native_import(&key, "other/account", "schema-1", 1000)
                .is_err()
        );
        assert_eq!(
            writer
                .container
                .read_frames()
                .unwrap()
                .into_iter()
                .map(|frame| frame.payload)
                .collect::<Vec<_>>(),
            before
        );
    }
}

#[test]
fn legacy_import_marker_remains_a_fence_but_cannot_resume_mutation() {
    let path = Temp::new();
    let owner = [118; 16];
    let key = "be".repeat(32);
    {
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let legacy = crate::records::ImportStart {
            format_version: 1,
            owner_uuid: owner,
            import_id: crate::records::import_id(owner, &key),
            import_key: key.clone(),
            source_namespace: "fixture/account".into(),
            source_format: "schema-1".into(),
            started_at_ns: 10,
            pre: crate::records::ImportWatermarks {
                global_version: 0,
                archive_version: Some(0),
                memory_version: 0,
                entity_version: Some(0),
                relationship_version: Some(0),
                graph_version: 0,
                memory_graph_version: Some(0),
                vector_version: Some(0),
            },
        };
        let json = serde_json::to_vec(&legacy).unwrap();
        let mut payload = b"RELIEPS1".to_vec();
        payload.extend_from_slice(&(json.len() as u32).to_le_bytes());
        payload.extend_from_slice(&json);
        writer.container.append(&payload).unwrap();
        writer.container.sync().unwrap();
    }
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path.0, admission, [], []).unwrap();
    let before = writer
        .container
        .read_frames()
        .unwrap()
        .into_iter()
        .map(|frame| frame.payload)
        .collect::<Vec<_>>();
    assert!(
        writer
            .start_native_import(&key, "fixture/account", "schema-1", 20)
            .unwrap_err()
            .to_string()
            .contains("fencing")
    );
    assert!(
        writer
            .accept_turn_and_advance_branch(
                turn("turn-1", None),
                Branch {
                    id: "main".into(),
                    conversation_id: "conversation".into(),
                    leaf_node_id: "turn-1".into(),
                    canonical: true,
                },
            )
            .is_err()
    );
    assert_eq!(
        writer
            .container
            .read_frames()
            .unwrap()
            .into_iter()
            .map(|frame| frame.payload)
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn import_scoped_turn_episode_and_import_priority_queue_replay_exactly() {
    let path = Temp::new();
    let owner = [119; 16];
    let key = "cf".repeat(32);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let episode_id;
    {
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let ImportEpochStatus::Open(start) = writer
            .start_native_import(&key, "fixture/account", "schema-1", 10)
            .unwrap()
        else {
            panic!("new import must open an epoch");
        };
        let input = turn("import-turn", None);
        let branch = Branch {
            id: "import-main".into(),
            conversation_id: "conversation".into(),
            leaf_node_id: "import-turn".into(),
            canonical: true,
        };
        let accepted = writer
            .accept_native_import_turn(&start.import_id, input.clone(), branch.clone())
            .unwrap();
        assert_eq!(accepted.id, "import-turn");
        episode_id = writer
            .finalize_native_import_episode(
                &start.import_id,
                "conversation",
                "import-turn",
                "import-turn",
                1,
            )
            .unwrap();
        writer
            .accept_native_import_turn(
                &start.import_id,
                turn("import-next", Some("import-turn")),
                Branch {
                    leaf_node_id: "import-next".into(),
                    ..branch.clone()
                },
            )
            .unwrap();
        let before_retry = writer.container.latest_version();
        writer
            .accept_native_import_turn(&start.import_id, input, branch)
            .unwrap();
        assert_eq!(
            writer.container.latest_version(),
            before_retry,
            "exact turn retry must not append"
        );
        writer
            .finalize_native_import_episode(
                &start.import_id,
                "conversation",
                "import-turn",
                "import-turn",
                1,
            )
            .unwrap();
        assert_eq!(writer.container.latest_version(), before_retry);
        let work = writer
            .read_session()
            .insomnia_work(WorkEpisodeId(episode_id.0))
            .unwrap();
        assert_eq!(work.priority, InsomniaPriority::Import);
        assert_eq!(
            work.state,
            reliquary_insomnia_store::InsomniaWorkState::Pending
        );
    }
    let reopened = RelSession::open_path(&path.0, &admission, [], []).unwrap();
    let work = reopened.insomnia_work(WorkEpisodeId(episode_id.0)).unwrap();
    assert_eq!(work.priority, InsomniaPriority::Import);
    assert_eq!(
        work.state,
        reliquary_insomnia_store::InsomniaWorkState::Pending
    );
}

#[test]
fn import_receipt_cannot_be_substituted_from_another_epoch() {
    let path = Temp::new();
    let owner = [120; 16];
    let key = "d1".repeat(32);
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
    let ImportEpochStatus::Open(start) = writer
        .start_native_import(&key, "fixture/account", "schema-1", 10)
        .unwrap()
    else {
        panic!("new import must open an epoch");
    };
    let before = writer
        .container
        .read_frames()
        .unwrap()
        .into_iter()
        .map(|frame| frame.payload)
        .collect::<Vec<_>>();
    let receipt = reliquary_archive::ImportReceipt {
        schema_version: 1,
        owner_uuid: owner,
        import_key: [0x22; 32],
        source_namespace: "fixture/account".into(),
        source_format: "schema-1".into(),
        source_digest: [0x22; 32],
        conversations: Vec::new(),
        receipt_digest: [0; 32],
    };
    assert!(
        writer
            .commit_native_import_receipt(&start.import_id, receipt)
            .is_err()
    );
    assert_eq!(
        writer
            .container
            .read_frames()
            .unwrap()
            .into_iter()
            .map(|frame| frame.payload)
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn reopen_rejects_archive_import_receipt_without_an_epoch() {
    let path = Temp::new();
    let owner = [121; 16];
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .session
        .archive
        .put_import_receipt(
            &mut writer.container,
            reliquary_archive::ImportReceipt {
                schema_version: 1,
                owner_uuid: owner,
                import_key: [1; 32],
                source_digest: [1; 32],
                source_namespace: "fixture/account".into(),
                source_format: "schema-1".into(),
                conversations: Vec::new(),
                receipt_digest: [0; 32],
            },
        )
        .unwrap();
    drop(writer);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(matches!(
        RelSession::open_path(&path.0, &admission, [], []),
        Err(OwnerSessionError::Record(message)) if message.contains("no owner epoch")
    ));
    // A matching marker appended later cannot legitimize the earlier receipt.
    let mut container = Container::open_writer(&path.0).unwrap();
    let key = "01".repeat(32);
    let mut imports = ImportEpochStore::open(&container).unwrap();
    imports
        .append_start(
            &mut container,
            ImportStart {
                format_version: 2,
                owner_uuid: owner,
                import_id: import_id(owner, &key),
                import_key: key,
                source_namespace: "fixture/account".into(),
                source_format: "schema-1".into(),
                started_at_ns: 1,
                pre: ImportWatermarks::default(),
            },
        )
        .unwrap();
    drop(container);
    assert!(matches!(
        RelSession::open_path(&path.0, &admission, [], []),
        Err(OwnerSessionError::Record(message)) if message.contains("physical position")
    ));
}
