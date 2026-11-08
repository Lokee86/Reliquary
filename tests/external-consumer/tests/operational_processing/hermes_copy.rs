use super::*;

#[path = "hermes_copy/maintenance.rs"]
mod maintenance;
use maintenance::assert_divergent_copy_preserves_receipts;
use reliquary_memory::{
    HermesCopiedMessage, HermesCopiedRole, HermesCopiedSnapshot, HermesCopyScope, InspectionItem,
    InspectionQuery, Instance, NativeImportState, OwnerId, OwnerKind, SourceAvailability,
    SourceTurn,
};

const TOKEN: &str = "external-hermes-copy-operator-admission";
const OTHER: &str = "external-hermes-copy-other-admission";

fn mount(rel: &Rel) -> (OwnerId, Instance, Instance) {
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(rel, true).unwrap();
    let (admission, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new(
            "operator",
            TOKEN,
            [OwnerGrant::OperateRel(owner), OwnerGrant::ManageRel(owner)],
        ),
        HostCredentialBinding::new("other", OTHER, [OwnerGrant::OperateRel(owner)]),
    ])
    .install(&gateway)
    .unwrap();
    (
        owner,
        admission.authenticate(TOKEN).unwrap(),
        admission.authenticate(OTHER).unwrap(),
    )
}

fn copied(scope: &HermesCopyScope, branch: &str, uid: &str) -> HermesCopiedSnapshot {
    let first = uid == "accepted";
    HermesCopiedSnapshot {
        scope: scope.clone(),
        branch_id: Some(branch.into()),
        source_revision: None,
        messages: vec![HermesCopiedMessage {
            message_uid: uid.into(),
            role: if first {
                HermesCopiedRole::User
            } else {
                HermesCopiedRole::Assistant
            },
            timestamp_ns: if first { 1_000_000_000 } else { 2_000_000_125 },
            content: if first {
                PROJECT
            } else {
                "The checklist was received."
            }
            .into(),
            author: None,
        }],
    }
}

fn inspect(instance: &Instance, owner: OwnerId, scope: &HermesCopyScope, uid: &str) -> SourceTurn {
    let page = instance
        .inspect_owner(
            owner,
            OwnerKind::Rel,
            &InspectionQuery::SourceTurn {
                conversation: scope.conversation_id(),
                node: uid.into(),
            },
            None,
            1,
        )
        .unwrap();
    let [InspectionItem::Turn(turn)] = page.items.as_slice() else {
        panic!("expected one exact copied turn");
    };
    turn.clone()
}

fn fixture_response(request: &Value, _: bool) -> Value {
    if request["input"].is_array() {
        return fixture_reply(request, false);
    }
    let schema = &request["response_format"]["json_schema"];
    let schema_name = schema["name"].as_str().unwrap();
    let assistant_only = schema_name == "insomnia_authority_disposition_ledger"
        && schema["schema"]["properties"]["turns"]["required"]
            .as_array()
            .is_some_and(Vec::is_empty);
    let output = if assistant_only {
        json!({"turns": {}, "evidence_requests": []})
    } else {
        let base = fixture_reply(request, false);
        let mut output: Value =
            serde_json::from_str(base["choices"][0]["message"]["content"].as_str().unwrap())
                .unwrap();
        match schema_name {
            "insomnia_authority_disposition_ledger" => {
                output["turns"]["accepted"]
                    .as_array_mut()
                    .unwrap()
                    .truncate(1);
            }
            "insomnia_memory_ownership" => {
                output["ownership"].as_object_mut().unwrap().remove("g001");
            }
            "insomnia_memory_wording" => output["candidates"].as_array_mut().unwrap().truncate(1),
            _ => panic!("unexpected fixture schema"),
        }
        output
    };
    json!({"choices":[{"message":{"content":output.to_string()}}]})
}

#[test]
fn hermes_copy_extension_preserves_exact_replay_and_terminal_scope() {
    let fixture = Fixture::start_with(fixture_response);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "hp04-hermes-extension-{}-{}",
            std::process::id(),
            fixture.listener.port()
        ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("copied.rel");
    let runtime = configured_fixture(
        &root.join("processing.cfg"),
        &format!("http://{}/v1", fixture.listener),
        &[
            (ModelSlot::General, "fixture"),
            (ModelSlot::EntityResolution, "fixture"),
        ],
        true,
    );
    let scope = HermesCopyScope::new("profile".into(), "session".into()).unwrap();
    let initial =
        || NativeImportPlan::from_hermes_copy(copied(&scope, "known-branch", "accepted")).unwrap();
    let predecessor = initial().source_sha256();
    let append = |branch, uid| {
        NativeImportPlan::from_hermes_copy_extension(copied(&scope, branch, uid), predecessor)
            .unwrap()
    };
    let second = || append("known-branch", "next");
    let rel = Rel::create(&path, OwnerScope::Project).unwrap();
    let (owner, instance, foreign) = mount(&rel);
    let first = instance.stage_native_import(owner, initial()).unwrap();
    let length = || fs::metadata(&path).unwrap().len();
    let unchanged = length();
    assert_eq!(
        instance
            .stage_native_import(owner, second())
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    assert_eq!(length(), unchanged);
    let process = |id: &str| {
        instance
            .process_and_drain(
                owner,
                &runtime,
                ProcessOptions {
                    import_id: Some(id.into()),
                    destination: None,
                    max_episodes: 1,
                },
            )
            .unwrap()
    };
    let report = process(&first.import_id);
    assert_eq!(report.state, ProcessingState::Ready, "{report:?}");
    assert_eq!(report.insomnia.rel_memories, 1);

    let unchanged = length();
    let mut edited = copied(&scope, "known-branch", "accepted");
    edited.messages[0].content = "An edited body with the same logical ID.".into();
    for (operator, plan) in [
        (&foreign, second()),
        (&instance, append("rewound-branch", "next")),
        (
            &instance,
            NativeImportPlan::from_hermes_copy_extension(edited, predecessor).unwrap(),
        ),
    ] {
        assert_eq!(
            operator
                .stage_native_import(owner, plan)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
    }
    assert_eq!(length(), unchanged);
    let accepted = instance.stage_native_import(owner, second()).unwrap();
    assert_eq!(accepted.coverage.turns, 1);
    assert_eq!(accepted.episode_ids.len(), 1);
    let next = inspect(&instance, owner, &scope, "next");
    assert_eq!(next.parent_node_id.as_deref(), Some("accepted"));
    assert_eq!(next.timestamp_ns, 2_000_000_125);
    let imported = next.imported.unwrap();
    assert_eq!(imported.message_id, "next");
    assert_eq!(imported.source_branch_id.as_deref(), Some("known-branch"));
    assert_eq!(imported.source_digest, accepted.source_sha256);
    let unchanged = length();
    assert!(
        instance
            .stage_native_import(owner, second())
            .unwrap()
            .already_staged
    );
    assert!(
        instance
            .stage_native_import(owner, initial())
            .unwrap()
            .already_completed
    );
    assert_eq!(length(), unchanged);

    let report = process(&accepted.import_id);
    assert_eq!(report.state, ProcessingState::Ready, "{report:?}");
    assert_eq!(report.insomnia.rel_memories, 0);
    assert_eq!(
        instance
            .native_import_status(owner, &accepted.import_id)
            .unwrap()
            .unwrap()
            .state,
        NativeImportState::Completed
    );
    let unchanged = length();
    assert_eq!(
        instance
            .stage_native_import(owner, append("known-branch", "stale"))
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    assert_eq!(length(), unchanged);
    drop((instance, foreign, rel));
    assert_divergent_copy_preserves_receipts(&path, &root, predecessor, accepted.source_sha256);

    let rel = Rel::open(&path).unwrap();
    let info = rel.info().unwrap();
    assert_eq!(info.rel_turn_count, Some(2));
    assert_eq!(info.memory_count, 1);
    let (owner, instance, _) = mount(&rel);
    assert!(
        instance
            .stage_native_import(owner, second())
            .unwrap()
            .already_completed
    );
    assert_eq!(length(), unchanged);
    assert!(
        instance
            .tombstone_hermes_copy(owner, &scope, predecessor)
            .unwrap()
    );
    assert!(
        !instance
            .tombstone_hermes_copy(owner, &scope, accepted.source_sha256)
            .unwrap()
    );
    assert_eq!(
        instance
            .stage_native_import(owner, second())
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    drop((instance, rel));

    let rel = Rel::open(&path).unwrap();
    let (owner, instance, _) = mount(&rel);
    for uid in ["accepted", "next"] {
        let turn = inspect(&instance, owner, &scope, uid);
        assert_eq!(turn.availability, SourceAvailability::Terminal);
        assert_eq!(turn.content, None);
        assert_eq!(turn.imported.unwrap().message_id, uid);
    }
}
