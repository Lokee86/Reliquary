use reliquary_memory::{
    Gateway, HostCredentialBinding, NativeImportFormat, NativeImportPlan, NativeImportState,
    NativeImportWindow, OwnerGrant, Phylactery, ProductErrorKind, Rel, TrustedHostAdmission,
};
use std::{fs, path::PathBuf};

#[test]
fn authenticated_normalized_import_preserves_forks_and_replays_after_reopen() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("oc03-native-import-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("normalized.json");
    let rel_path = dir.join("owner.rel");
    let phy_path = dir.join("owner.phy");
    let fixture = r#"{
        "schema": "reliquary-native-live-2day-v1",
        "window_start": "1970-01-01T00:00:01Z",
        "window_end_exclusive": "1970-01-01T00:00:04+00:00",
        "omitted_missing_timestamps": 2,
        "conversations": [{"id":"conversation", "title":"Fixture"}],
        "turns": [
            {"conversation_id":"conversation","id":"a","message_id":"message-a","original_parent_id":"hidden","role":"user","timestamp_ns":1000000001,"content":"Question","branch_id":"main","canonical":true,"asset_refs":[{"kind":"image"}]},
            {"conversation_id":"conversation","id":"b","parent_id":"a","role":"assistant","timestamp_ns":2000000000,"content":"First reply","branch_id":"main","canonical":true},
            {"conversation_id":"conversation","id":"c","parent_id":"a","role":"assistant","timestamp_ns":3000000000,"content":"Alternate reply","branch_id":"fork","canonical":false}
        ],
        "branches": [
            {"conversation_id":"conversation","id":"main","leaf_node_id":"b","canonical":true},
            {"conversation_id":"conversation","id":"fork","leaf_node_id":"c","canonical":false},
            {"conversation_id":"conversation","id":"alias","leaf_node_id":"c","canonical":false}
        ],
        "episodes": [
            {"conversation_id":"conversation","start_node_id":"a","end_node_id":"b","turn_count":2},
            {"conversation_id":"conversation","start_node_id":"c","end_node_id":"c","turn_count":1}
        ]
    }"#;
    fs::write(&source, fixture).unwrap();
    let window =
        NativeImportWindow::from_rfc3339("1970-01-01T00:00:01Z", "1970-01-01T00:00:04Z").unwrap();
    let plan = || {
        NativeImportPlan::from_path(&source, NativeImportFormat::Normalized, Some(window)).unwrap()
    };
    assert_eq!(plan().coverage().omitted_asset_references, 1);
    assert_eq!(plan().coverage().omitted_missing_timestamps, 2);
    assert_eq!(
        plan().parent_mappings()[0].original_parent_id.as_deref(),
        Some("hidden")
    );

    let rel = Rel::create(&rel_path).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    let first = {
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let phy = gateway.mount_phylactery(&phy, true).unwrap();
        let oper_token = "o".repeat(48);
        let view_token = "v".repeat(48);
        let other_token = "p".repeat(48);
        let (adapter, control) = TrustedHostAdmission::new([
            HostCredentialBinding::new(
                "reliquary-cli-local",
                &oper_token,
                [
                    OwnerGrant::OperateRel(owner),
                    OwnerGrant::OperatePhylactery(phy),
                ],
            ),
            HostCredentialBinding::new("viewer", &view_token, [OwnerGrant::ViewRel(owner)]),
            HostCredentialBinding::new(
                "other-operator",
                &other_token,
                [OwnerGrant::OperateRel(owner)],
            ),
        ])
        .install(&gateway)
        .unwrap();
        let oper = adapter.authenticate(&oper_token).unwrap();
        let view = adapter.authenticate(&view_token).unwrap();
        let other = adapter.authenticate(&other_token).unwrap();
        let import_id = plan().import_id(owner);
        let before = fs::metadata(&rel_path).unwrap().len();
        fs::write(
            &source,
            fixture.replace("\"parent_id\":\"a\"", "\"parent_id\":\"missing\""),
        )
        .unwrap();
        assert_eq!(
            NativeImportPlan::from_path(&source, NativeImportFormat::Normalized, Some(window))
                .err()
                .unwrap()
                .kind(),
            ProductErrorKind::InvalidInput
        );
        assert_eq!(fs::metadata(&rel_path).unwrap().len(), before);
        fs::write(&source, fixture).unwrap();
        assert_eq!(
            view.stage_native_import(owner, plan()).unwrap_err().kind(),
            ProductErrorKind::Unauthorized
        );
        assert!(
            view.native_import_status(owner, &import_id)
                .unwrap()
                .is_none()
        );
        let result = oper.stage_native_import(owner, plan()).unwrap();
        assert_eq!(result.import_id, import_id);
        assert_eq!(result.coverage.turns, 3);
        assert_eq!(result.coverage.branches, 3);
        assert_eq!(result.episode_ids.len(), 2);
        assert_eq!(
            view.native_import_status(owner, &import_id)
                .unwrap()
                .unwrap()
                .state,
            NativeImportState::Staged
        );
        let management = view.workspace_management(owner).unwrap();
        assert_eq!(
            management.conversations(0, 8).unwrap().items[0].branch.id,
            "main"
        );
        management.select_conversation("conversation", "c").unwrap();
        let transcript = management.transcript(0, 8).unwrap().items;
        assert_eq!(
            transcript
                .iter()
                .map(|turn| turn.node_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(transcript[0].timestamp_ns, 1_000_000_001);
        assert_eq!(
            transcript[0].principal_id.as_deref(),
            Some("reliquary-cli-local")
        );
        assert_eq!(
            other.stage_native_import(owner, plan()).unwrap_err().kind(),
            ProductErrorKind::Conflict
        );
        control.revoke(&view_token).unwrap();
        assert_eq!(
            view.native_import_status(owner, &import_id)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Revoked
        );
        result
    };
    let staged_bytes = fs::read(&rel_path).unwrap();
    {
        let gateway = Gateway::new();
        let owner = gateway
            .mount_rel(&Rel::open(&rel_path).unwrap(), true)
            .unwrap();
        let token = "r".repeat(48);
        let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "reliquary-cli-local",
            &token,
            [OwnerGrant::OperateRel(owner)],
        )])
        .install(&gateway)
        .unwrap();
        let instance = adapter.authenticate(&token).unwrap();
        let replay = instance.stage_native_import(owner, plan()).unwrap();
        assert!(replay.already_staged);
        assert_eq!(replay.receipt_digest, first.receipt_digest);
        assert_eq!(replay.episode_ids, first.episode_ids);
        fs::write(&source, fixture.replace("Fixture", "Changed title")).unwrap();
        assert_eq!(
            instance
                .stage_native_import(owner, plan())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
    }
    assert_eq!(fs::read(&rel_path).unwrap(), staged_bytes);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn archive_vectors_publish_through_a_staged_import_without_processing() {
    use reliquary_memory::{
        EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingNormalization, SwitchboardEmbeddingMode,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Default)]
    struct Endpoint {
        calls: AtomicUsize,
        archive_calls: AtomicUsize,
    }
    impl EmbeddingEndpoint for Endpoint {
        fn dimensions(&self) -> u32 {
            2
        }

        fn normalization(&self) -> EmbeddingNormalization {
            EmbeddingNormalization::L2
        }

        fn supports_bounded_embedding(&self) -> bool {
            true
        }

        fn embed(
            &self,
            _: SwitchboardEmbeddingMode,
            _: &[String],
        ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
            panic!("Archive inference must be bounded")
        }

        fn embed_bounded(
            &self,
            mode: SwitchboardEmbeddingMode,
            texts: &[String],
            bound: usize,
        ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            assert!(bound >= texts.len() * 8);
            for text in texts {
                if text.starts_with("user: ") {
                    assert_eq!(mode, SwitchboardEmbeddingMode::Document);
                    assert!(text.len() <= 12_000);
                    self.archive_calls.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(vec![vec![1.0, 0.0]; texts.len()])
        }
    }

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("staged-archive-vectors-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("source.json");
    let rel_path = dir.join("owner.rel");
    let turn = format!(
        "{} quasar {} {}",
        "prefix ".repeat(2_200),
        "λ ".repeat(44 * 1024),
        "end"
    );
    let quasar = turn.find("quasar").unwrap() as u64;
    let turn_bytes = turn.len() as u64;
    fs::write(
        &source,
        serde_json::to_vec(&serde_json::json!({
            "schema": "reliquary-native-live-2day-v1",
            "window_start": "1970-01-01T00:00:01Z",
            "window_end_exclusive": "1970-01-01T00:00:04Z",
            "conversations": [{"id": "conversation"}],
            "turns": [{
                "conversation_id": "conversation", "id": "a", "role": "user",
                "timestamp_ns": 1_000_000_001_i64, "content": turn, "branch_id": "main",
                "canonical": true
            }],
            "branches": [{
                "conversation_id": "conversation", "id": "main",
                "leaf_node_id": "a", "canonical": true
            }],
            "episodes": [{
                "conversation_id": "conversation", "start_node_id": "a",
                "end_node_id": "a", "turn_count": 1
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let plan = NativeImportPlan::from_path(&source, NativeImportFormat::Normalized, None).unwrap();
    let rel = Rel::create(&rel_path).unwrap();
    let endpoint = Arc::new(Endpoint::default());
    let (publication, staged_id) = {
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let (adapter, _) = TrustedHostAdmission::new([
            HostCredentialBinding::new(
                "reliquary-cli-local",
                &"o".repeat(48),
                [OwnerGrant::OperateRel(owner)],
            ),
            HostCredentialBinding::new("viewer", &"v".repeat(48), [OwnerGrant::ViewRel(owner)]),
        ])
        .install(&gateway)
        .unwrap();
        let operator = adapter.authenticate(&"o".repeat(48)).unwrap();
        let viewer = adapter.authenticate(&"v".repeat(48)).unwrap();
        let staged = operator.stage_native_import(owner, plan).unwrap();
        assert_eq!(
            viewer
                .publish_native_import_archive_vectors(owner, &staged.import_id, endpoint.clone())
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        assert!(
            operator
                .publish_native_import_archive_vectors(owner, &"1".repeat(64), endpoint.clone())
                .is_err()
        );
        assert_eq!(endpoint.calls.load(Ordering::Relaxed), 0);
        let published = operator
            .publish_native_import_archive_vectors(owner, &staged.import_id, endpoint.clone())
            .unwrap();
        assert!(published.fragment_count > 1);
        assert_eq!(
            operator
                .native_import_status(owner, &staged.import_id)
                .unwrap()
                .unwrap()
                .state,
            NativeImportState::Staged
        );
        (published, staged.import_id)
    };

    drop(rel);
    let gateway = Gateway::new();
    let owner = gateway
        .mount_rel(&Rel::open(&rel_path).unwrap(), true)
        .unwrap();
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "reliquary-cli-local",
        &"o".repeat(48),
        [OwnerGrant::OperateRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let operator = adapter.authenticate(&"o".repeat(48)).unwrap();
    let hits = operator
        .search_archive_hybrid(owner, publication.profile_id, "quasar", &[0.8, 0.6], 32)
        .unwrap();
    assert_eq!(hits.generation_id, publication.generation_id);
    assert_eq!(hits.hits.len(), publication.fragment_count);
    let matched = hits.hits[0].passage.expect("exact matched passage");
    assert!(hits.hits[0].lexical_score.is_some());
    assert!(matched.start_byte <= quasar && quasar + 6 <= matched.end_byte);
    assert!(matched.start_byte > 0);
    let mut passages = hits
        .hits
        .iter()
        .map(|hit| {
            assert_eq!((&*hit.start_node_id, &*hit.end_node_id), ("a", "a"));
            hit.passage.expect("passage source")
        })
        .collect::<Vec<_>>();
    passages.sort_by_key(|passage| passage.start_byte);
    let mut cursor = 0;
    for passage in passages {
        assert_eq!(passage.parent_id, matched.parent_id);
        assert_eq!(passage.start_byte, cursor);
        cursor = passage.end_byte;
    }
    assert_eq!(cursor, turn_bytes);
    let before = fs::metadata(&rel_path).unwrap().len();
    assert_eq!(
        operator
            .publish_native_import_archive_vectors(owner, &staged_id, endpoint.clone())
            .unwrap(),
        publication
    );
    assert_eq!(fs::metadata(&rel_path).unwrap().len(), before);
    assert_eq!(
        endpoint.archive_calls.load(Ordering::Relaxed),
        publication.fragment_count
    );
    assert_eq!(
        operator
            .native_import_status(owner, &staged_id)
            .unwrap()
            .unwrap()
            .state,
        NativeImportState::Staged
    );
    drop(operator);
    drop(adapter);
    drop(gateway);
    let info = Rel::open(&rel_path).unwrap().info().unwrap();
    assert_eq!(info.memory_version, 0);
    assert_eq!(info.graph_version, 0);
    fs::remove_file(rel_path).unwrap();
    fs::remove_file(source).unwrap();
    fs::remove_dir(dir).unwrap();
}
