use reliquary_memory::{
    Gateway, HostCredentialBinding, NativeImportFormat, NativeImportPlan, NativeImportState,
    NativeImportWindow, OwnerGrant, OwnerScope, Phylactery, ProductErrorKind, Rel,
    TrustedHostAdmission,
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

    let rel = Rel::create(&rel_path, OwnerScope::Project).unwrap();
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
