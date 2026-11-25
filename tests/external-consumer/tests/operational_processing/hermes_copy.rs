use super::*;

#[path = "hermes_copy/maintenance.rs"]
mod maintenance;
use maintenance::assert_divergent_copy_preserves_receipts;
use reliquary_memory::{
    HermesCopiedEcho, HermesCopiedEchoKind, HermesCopiedMessage, HermesCopiedRole,
    HermesCopiedSnapshot, HermesCopyScope, HermesCopySourceChange, InspectionItem, InspectionQuery,
    Instance, NativeImportState, OwnerId, OwnerKind, SourceAvailability, SourceTurn, TemporalBasis,
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
    let second = || {
        NativeImportPlan::from_hermes_copy_extension_with_echo(
            copied(&scope, "known-branch", "next"),
            predecessor,
            vec![HermesCopiedEcho {
                message_uid: "next".into(),
                sequence: 1,
                timestamp_ns: 2_000_000_125,
                model_round: 0,
                kind: HermesCopiedEchoKind::ReasoningSummary {
                    redacted_summary: "Checklist follow-up echo.".into(),
                },
            }],
        )
        .unwrap()
    };
    let rel = Rel::create(&path).unwrap();
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
    assert!(
        instance
            .publish_archive_vectors(owner, &runtime, None)
            .unwrap()
            .is_some()
    );
    let unchanged = length();
    drop((instance, foreign, rel));
    assert_divergent_copy_preserves_receipts(&path, &root, predecessor, accepted.source_sha256);

    let rel = Rel::open(&path).unwrap();
    let info = rel.info().unwrap();
    assert_eq!(info.rel_turn_count, Some(2));
    assert_eq!(info.memory_count, 1);
    let (owner, instance, foreign) = mount(&rel);
    let conversation = scope.conversation_id();
    let cut = info.archive_version.unwrap();
    let rows = instance
        .read_hermes_archive_range(owner, &conversation, "accepted", "next", cut)
        .unwrap()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source.source_sha256, predecessor);
    assert_eq!(rows[1].source.source_sha256, accepted.source_sha256);
    assert_eq!(rows[1].source.message_uid, "next");
    assert_eq!(rows[1].echo.len(), 1);
    assert!(
        foreign
            .read_hermes_archive_range(owner, &conversation, "accepted", "next", cut)
            .unwrap()
            .is_none()
    );
    assert!(
        instance
            .stage_native_import(owner, second())
            .unwrap()
            .already_completed
    );
    assert_eq!(length(), unchanged);
    let mut chat = instance
        .begin_chat_retrieval(owner, "archive-admission")
        .unwrap();
    assert_eq!(
        chat.search_chat_archive(
            "checklist",
            TemporalBasis::Source,
            3_000_000_000,
            "UTC",
            &runtime
        )
        .unwrap_err()
        .kind(),
        ProductErrorKind::TemporalQueryRequired,
    );
    let hits = chat
        .search_chat_archive(
            "checklist today",
            TemporalBasis::Source,
            3_000_000_000,
            "UTC",
            &runtime,
        )
        .unwrap()
        .hits;
    let reference = &hits.first().expect("a temporal Archive hit").reference;
    assert!(
        !chat
            .preview_archive_reference(reference)
            .unwrap()
            .is_empty()
    );
    assert!(chat.read_archive_reference(reference).is_err());
    assert!(chat.accept_archive_results(&["forged".into()]).is_err());
    chat.accept_archive_results(std::slice::from_ref(reference))
        .unwrap();
    assert!(!chat.read_archive_reference(reference).unwrap().is_empty());
    let new_chat = instance
        .begin_chat_retrieval(owner, "different-admission")
        .unwrap();
    assert!(new_chat.read_archive_reference(reference).is_err());
    drop(new_chat);

    let other_scope = HermesCopyScope::new("profile".into(), "independent".into()).unwrap();
    let independent = instance
        .stage_native_import(
            owner,
            NativeImportPlan::from_hermes_copy(copied(&other_scope, "known-branch", "next"))
                .unwrap(),
        )
        .unwrap();
    let rewind = || HermesCopySourceChange::Rewind {
        last_surviving_message_uid: Some("accepted".into()),
    };
    let apply_rewind = |operator: &Instance, selected: &HermesCopyScope, head| {
        operator.apply_hermes_copy_source_change(owner, selected, head, "rewind-1", rewind())
    };
    assert_eq!(
        apply_rewind(&instance, &scope, accepted.source_sha256)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict,
    );
    process_copy(&instance, owner, &runtime, &independent.import_id);
    let unchanged = length();
    for (operator, selected, head) in [
        (&foreign, &scope, accepted.source_sha256),
        (&instance, &other_scope, accepted.source_sha256),
        (&instance, &scope, predecessor),
    ] {
        assert_eq!(
            apply_rewind(operator, selected, head).unwrap_err().kind(),
            ProductErrorKind::Conflict
        );
    }
    assert_eq!(length(), unchanged);
    let rewound = apply_rewind(&instance, &scope, accepted.source_sha256).unwrap();
    let rewound_head = rewound.resulting_head_sha256.unwrap();
    assert_eq!(
        rewound.last_surviving_message_uid.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        instance
            .read_hermes_copy_active_message_uids(owner, &scope, rewound_head, 0, 128)
            .unwrap(),
        Some(vec![String::from("accepted")])
    );
    assert_eq!(
        instance
            .read_hermes_copy_active_message_uids(owner, &scope, accepted.source_sha256, 0, 128)
            .unwrap(),
        None
    );
    assert_eq!(
        instance
            .read_hermes_copy_source(owner, &scope, rewound_head, "accepted", "accepted")
            .unwrap()
            .unwrap()[0]
            .source
            .source_sha256,
        predecessor
    );
    let rewound_length = length();
    assert_eq!(
        apply_rewind(&instance, &scope, accepted.source_sha256).unwrap(),
        rewound
    );
    assert_eq!(
        instance
            .apply_hermes_copy_source_change(
                owner,
                &scope,
                accepted.source_sha256,
                "rewind-1",
                HermesCopySourceChange::Delete,
            )
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict,
    );
    assert_eq!(length(), rewound_length);
    assert!(chat.read_archive_reference(reference).is_err());
    let mut search_after_rewind = instance
        .begin_chat_retrieval(owner, "after-rewind")
        .unwrap();
    let new_hits = search_after_rewind
        .search_chat_archive(
            "checklist today",
            TemporalBasis::Source,
            3_000_000_000,
            "UTC",
            &runtime,
        )
        .unwrap()
        .hits;
    assert!(
        new_hits.is_empty(),
        "the indexed window crossed the removed suffix"
    );
    drop(search_after_rewind);
    assert_eq!(inspect(&instance, owner, &scope, "next").content, None);
    assert_eq!(
        inspect(&instance, owner, &scope, "next").availability,
        SourceAvailability::Terminal
    );
    assert!(
        instance
            .read_hermes_copy_source(owner, &scope, accepted.source_sha256, "next", "next")
            .unwrap()
            .is_none()
    );
    assert!(
        instance
            .read_hermes_archive_range(owner, &conversation, "accepted", "next", cut)
            .unwrap()
            .is_none()
    );
    assert!(
        instance
            .read_hermes_archive_range(owner, &conversation, "accepted", "accepted", cut)
            .unwrap()
            .is_some()
    );
    assert!(
        instance
            .read_hermes_copy_source(
                owner,
                &other_scope,
                independent.source_sha256,
                "next",
                "next"
            )
            .unwrap()
            .is_some()
    );
    drop((chat, instance, foreign, rel));

    let rel = Rel::open(&path).unwrap();
    let (owner, instance, foreign) = mount(&rel);
    assert_eq!(
        apply_rewind(&instance, &scope, accepted.source_sha256).unwrap(),
        rewound
    );
    assert_eq!(length(), rewound_length);
    let planned = |head, uid| {
        NativeImportPlan::from_hermes_copy_extension(copied(&scope, "known-branch", uid), head)
            .unwrap()
    };
    for candidate in [
        planned(accepted.source_sha256, "fresh"),
        planned(predecessor, "fresh"),
        planned(rewound_head, "next"),
    ] {
        assert_eq!(
            instance
                .stage_native_import(owner, candidate)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
    }
    assert_eq!(length(), rewound_length);
    let fresh = instance
        .stage_native_import(owner, planned(rewound_head, "fresh"))
        .unwrap();
    assert_eq!(
        inspect(&instance, owner, &scope, "fresh")
            .parent_node_id
            .as_deref(),
        Some("accepted")
    );
    process_copy(&instance, owner, &runtime, &fresh.import_id);
    assert!(
        instance
            .stage_native_import(owner, planned(rewound_head, "fresh"))
            .unwrap()
            .already_completed
    );

    let emptied = instance
        .apply_hermes_copy_source_change(
            owner,
            &scope,
            fresh.source_sha256,
            "rewind-empty",
            HermesCopySourceChange::Rewind {
                last_surviving_message_uid: None,
            },
        )
        .unwrap();
    let from_empty = instance
        .stage_native_import(
            owner,
            planned(emptied.resulting_head_sha256.unwrap(), "fresh-root"),
        )
        .unwrap();
    assert_eq!(
        inspect(&instance, owner, &scope, "fresh-root").parent_node_id,
        None
    );
    process_copy(&instance, owner, &runtime, &from_empty.import_id);
    let delete = || {
        instance
            .apply_hermes_copy_source_change(
                owner,
                &scope,
                predecessor,
                "delete-1",
                HermesCopySourceChange::Delete,
            )
            .unwrap()
    };
    let deleted = delete();
    assert_eq!(deleted.resulting_head_sha256, None);
    let deleted_length = length();
    assert_eq!(delete(), deleted);
    assert_eq!(length(), deleted_length);
    assert_eq!(
        apply_rewind(&instance, &scope, accepted.source_sha256).unwrap(),
        rewound
    );
    assert_eq!(
        instance
            .apply_hermes_copy_source_change(
                owner,
                &scope,
                from_empty.source_sha256,
                "late-rewind",
                HermesCopySourceChange::Rewind {
                    last_surviving_message_uid: None
                },
            )
            .unwrap_err()
            .kind(),
        ProductErrorKind::Conflict
    );
    assert_eq!(length(), deleted_length);
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
    assert!(
        instance
            .read_hermes_archive_range(owner, &conversation, "accepted", "next", cut)
            .unwrap()
            .is_none()
    );
    drop((instance, foreign, rel));

    let rel = Rel::open(&path).unwrap();
    let (owner, instance, _) = mount(&rel);
    for uid in ["accepted", "next", "fresh", "fresh-root"] {
        let turn = inspect(&instance, owner, &scope, uid);
        assert_eq!(turn.availability, SourceAvailability::Terminal);
        assert_eq!(turn.content, None);
        assert_eq!(turn.imported.unwrap().message_id, uid);
    }
    assert!(
        instance
            .read_hermes_copy_source(
                owner,
                &other_scope,
                independent.source_sha256,
                "next",
                "next"
            )
            .unwrap()
            .is_some()
    );
}

fn process_copy(instance: &Instance, owner: OwnerId, runtime: &ConfiguredRuntime, id: &str) {
    let report = instance
        .process_and_drain(
            owner,
            runtime,
            ProcessOptions {
                import_id: Some(id.into()),
                destination: None,
                max_episodes: 1,
            },
        )
        .unwrap();
    assert_eq!(report.state, ProcessingState::Ready, "{report:?}");
}
