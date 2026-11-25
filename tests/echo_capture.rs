use reliquary_memory::{
    Gateway, HermesCopiedEcho, HermesCopiedEchoKind, HermesCopiedMessage, HermesCopiedRole,
    HermesCopiedSnapshot, HermesCopiedToolOutcome, HermesCopyScope, HostCredentialBinding,
    NativeImportPlan, OwnerGrant, ProductErrorKind, Rel, TrustedHostAdmission,
};
use std::{fs, path::PathBuf};

#[test]
fn copied_hermes_echo_is_exact_owner_bound_and_durable() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("hermes-echo-{}.rel", std::process::id()));
    let scope = HermesCopyScope::new("profile".into(), "session".into()).unwrap();
    let snapshot = || HermesCopiedSnapshot {
        scope: scope.clone(),
        branch_id: None,
        source_revision: None,
        messages: vec![
            ("u", HermesCopiedRole::User, "question"),
            ("c", HermesCopiedRole::Commentary, "checking"),
            ("f", HermesCopiedRole::Assistant, "answer"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (uid, role, content))| HermesCopiedMessage {
            message_uid: uid.into(),
            role,
            timestamp_ns: i as i64 + 1,
            content: content.into(),
            author: None,
        })
        .collect(),
    };
    let evidence = |result: &str| {
        [
            HermesCopiedEchoKind::ModelRoundStarted,
            HermesCopiedEchoKind::ReasoningSummary {
                redacted_summary: "Used the owner index".into(),
            },
            HermesCopiedEchoKind::ToolCall {
                call_id: "call-1".into(),
                tool_name: "search".into(),
                redacted_arguments: "topic=[redacted]".into(),
            },
            HermesCopiedEchoKind::ToolResult {
                call_id: "call-1".into(),
                outcome: HermesCopiedToolOutcome::Succeeded,
                redacted_result: result.into(),
            },
            HermesCopiedEchoKind::ModelRoundCompleted,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, kind)| HermesCopiedEcho {
            message_uid: "f".into(),
            sequence: i as u64,
            timestamp_ns: i as i64 + 2,
            model_round: 1,
            kind,
        })
        .collect::<Vec<_>>()
    };
    let plan = |result| {
        NativeImportPlan::from_hermes_copy_with_echo(snapshot(), evidence(result)).unwrap()
    };
    let expected = evidence("5 matches");
    let digest = plan("5 matches").source_sha256();
    assert_ne!(digest, plan("7 matches").source_sha256());
    assert_eq!(
        digest,
        NativeImportPlan::from_hermes_copy_with_echo(
            snapshot(),
            expected.iter().rev().cloned().collect(),
        )
        .unwrap()
        .source_sha256()
    );
    assert_eq!(
        NativeImportPlan::from_hermes_copy(snapshot())
            .unwrap()
            .source_sha256(),
        NativeImportPlan::from_hermes_copy_with_echo(snapshot(), vec![])
            .unwrap()
            .source_sha256()
    );
    let mut foreign_anchor = expected.clone();
    foreign_anchor[3].message_uid = "u".into();
    assert!(NativeImportPlan::from_hermes_copy_with_echo(snapshot(), foreign_anchor).is_err());

    let token = "hermes-echo-operator-credential".repeat(2);
    let other_token = "hermes-echo-other-credential".repeat(2);
    let open = |rel: &Rel, write| {
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(rel, write).unwrap();
        let (admission, control) = TrustedHostAdmission::new([
            HostCredentialBinding::new("operator", &token, [OwnerGrant::OperateRel(owner)]),
            HostCredentialBinding::new("other", &other_token, [OwnerGrant::ViewRel(owner)]),
        ])
        .install(&gateway)
        .unwrap();
        (
            owner,
            admission.authenticate(&token).unwrap(),
            admission.authenticate(&other_token).unwrap(),
            control,
        )
    };
    {
        let rel = Rel::create(&path).unwrap();
        let (owner, instance, _, _) = open(&rel, true);
        let stored = instance
            .stage_native_import(owner, plan("5 matches"))
            .unwrap();
        assert_eq!(stored.source_sha256, digest);
        assert_eq!(stored.episode_ids.len(), 1);
        let length = fs::metadata(&path).unwrap().len();
        let retry = instance
            .stage_native_import(owner, plan("5 matches"))
            .unwrap();
        assert!(retry.already_staged);
        assert_eq!(
            instance
                .stage_native_import(owner, plan("7 matches"))
                .unwrap_err()
                .kind(),
            ProductErrorKind::Conflict
        );
        assert_eq!(fs::metadata(&path).unwrap().len(), length);
    }
    {
        let rel = Rel::open(&path).unwrap();
        let cut = rel.info().unwrap().archive_version.unwrap();
        let (owner, instance, other, control) = open(&rel, false);
        assert!(
            other
                .read_hermes_copy_source(owner, &scope, digest, "u", "f")
                .unwrap()
                .is_none()
        );
        let rows = instance
            .read_hermes_copy_source(owner, &scope, digest, "u", "f")
            .unwrap()
            .unwrap();
        assert_eq!(
            rows.iter()
                .map(|turn| turn.archive.role.as_str())
                .collect::<Vec<_>>(),
            ["user", "commentary", "assistant"]
        );
        assert_eq!(rows[2].echo, expected);
        assert_eq!(rows[2].source.message_uid, "f");
        assert_eq!(rows[2].source.source_sha256, digest);
        assert_eq!(
            instance
                .read_hermes_archive_range(owner, &scope.conversation_id(), "u", "f", cut)
                .unwrap()
                .unwrap(),
            rows
        );
        assert!(
            instance
                .read_hermes_archive_range(owner, &scope.conversation_id(), "u", "f", 0)
                .unwrap()
                .is_none()
        );
        assert!(
            instance
                .read_hermes_archive_range(owner, &scope.conversation_id(), "f", "u", cut)
                .unwrap()
                .is_none()
        );
        control.revoke(&token).unwrap();
        assert!(
            instance
                .read_hermes_copy_source(owner, &scope, digest, "u", "f")
                .is_err()
        );
    }
    fs::remove_file(path).unwrap();
}
