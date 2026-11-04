use crate::{Archive, EchoError, EchoEvent, EchoEventKind, EchoStore, TurnInput};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("echo-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn event(sequence: u64, kind: EchoEventKind) -> EchoEvent {
    let call_kind = matches!(kind, EchoEventKind::ToolCall);
    let correlated = matches!(
        kind,
        EchoEventKind::ToolCall
            | EchoEventKind::ToolResult
            | EchoEventKind::ToolContextAccepted
            | EchoEventKind::ActivityStarted
            | EchoEventKind::ActivityCompleted
            | EchoEventKind::ActivityFailed
    );
    EchoEvent {
        conversation_id: "conversation-echo".into(),
        message_id: "assistant-1".into(),
        sequence,
        timestamp_ns: 1_700_000_000,
        model_round: Some(2),
        kind,
        correlation_id: correlated.then(|| "call-9".into()),
        name: call_kind.then(|| "search".into()),
        content: format!("evidence-{sequence}"),
    }
}

#[test]
fn echo_is_separate_source_keyed_evidence_and_reopens_without_activity_effects() {
    let temp = TestDirectory::new();
    let path = temp.0.join("echo.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xE1; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let acceptance = archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "user-1".into(),
                conversation_id: "conversation-echo".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal-1".into()),
                timestamp_ns: 10,
                content: "source turn".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(acceptance.activity_position, Some(1));

    let mut echo = EchoStore::default();
    let summary = event(0, EchoEventKind::ReasoningSummary);
    let tool_call = event(1, EchoEventKind::ToolCall);
    assert!(echo.put(&mut container, summary.clone()).unwrap());
    assert!(echo.put(&mut container, tool_call.clone()).unwrap());
    assert!(!echo.put(&mut container, summary).unwrap());
    let conflicting = EchoEvent {
        content: "different accepted evidence".into(),
        ..tool_call
    };
    assert!(matches!(
        echo.put(&mut container, conflicting),
        Err(EchoError::ConflictingSequence)
    ));
    container.sync().unwrap();
    drop(echo);
    drop(archive);
    drop(container);

    let reopened_container = Container::open_read_only(&path).unwrap();
    let reopened_archive = Archive::open(&reopened_container).unwrap();
    let reopened_echo = EchoStore::open(&reopened_container).unwrap();
    assert_eq!(reopened_archive.node_count(), 1);
    assert_eq!(reopened_archive.rel_turn_count(), 1);
    assert_eq!(reopened_echo.owner_id(), Some([0xE1; 16]));
    assert_eq!(
        reopened_echo.for_turn("conversation-echo", "assistant-1"),
        vec![
            event(0, EchoEventKind::ReasoningSummary),
            event(1, EchoEventKind::ToolCall),
        ]
    );
}

#[test]
fn echo_rejects_missing_tool_proof_before_appending() {
    let temp = TestDirectory::new();
    let path = temp.0.join("invalid-echo.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xE2; 16],
    )
    .unwrap();
    let mut echo = EchoStore::default();
    let mut invalid = event(0, EchoEventKind::ToolCall);
    invalid.correlation_id = None;
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        echo.put(&mut container, invalid),
        Err(EchoError::InvalidRecord("tool call ID is required"))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn fresh_echo_store_replays_before_checking_sequence() {
    let temp = TestDirectory::new();
    let path = temp.0.join("existing-echo.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xE3; 16],
    )
    .unwrap();
    let original = event(0, EchoEventKind::ReasoningSummary);
    EchoStore::default()
        .put(&mut container, original.clone())
        .unwrap();
    let before = container.read_frames().unwrap().len();
    let mut fresh = EchoStore::default();
    let conflict = EchoEvent {
        content: "conflict".into(),
        ..original.clone()
    };
    assert!(matches!(
        fresh.put(&mut container, conflict),
        Err(EchoError::ConflictingSequence)
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    assert!(!fresh.put(&mut container, original.clone()).unwrap());
    assert_eq!(
        EchoStore::open(&container)
            .unwrap()
            .for_turn("conversation-echo", "assistant-1"),
        vec![original]
    );
}

#[test]
fn echo_rejects_other_rel_scope_with_same_owner_id() {
    let temp = TestDirectory::new();
    let mut first = Container::create(
        &temp.0.join("project.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xED; 16],
    )
    .unwrap();
    let mut echo = EchoStore::default();
    echo.put(&mut first, event(0, EchoEventKind::ReasoningSummary))
        .unwrap();
    let mut other = Container::create(
        &temp.0.join("organization.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Organization),
        [0xED; 16],
    )
    .unwrap();
    assert!(matches!(
        echo.put(&mut other, event(1, EchoEventKind::ReasoningSummary)),
        Err(EchoError::WrongOwner)
    ));
    assert!(other.read_frames().unwrap().is_empty());
}
