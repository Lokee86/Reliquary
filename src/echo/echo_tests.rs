use crate::{Cva, EchoError, EchoEvent, EchoEventKind};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn test_path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reliquary-echo-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join("project.rel")
}

fn event(sequence: u64, kind: EchoEventKind, content: &str) -> EchoEvent {
    EchoEvent {
        conversation_id: "conversation-1".into(),
        message_id: "assistant-1".into(),
        sequence,
        timestamp_ns: 123 + sequence as i64,
        model_round: Some(1),
        kind,
        correlation_id: None,
        name: None,
        content: content.into(),
    }
}

#[test]
fn echo_events_persist_inside_the_rel_in_sequence_order() {
    let path = test_path();
    let mut rel = Cva::create_project(&path).unwrap();
    rel.put_echo_event(event(2, EchoEventKind::ReasoningTrace, "private trace"))
        .unwrap();
    rel.put_echo_event(event(0, EchoEventKind::ReasoningSummary, "summary"))
        .unwrap();
    rel.put_echo_event(event(1, EchoEventKind::Commentary, "checking memory"))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let reopened = Cva::open(&path).unwrap();
    let events = reopened.echo_events("conversation-1", "assistant-1");
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].sequence, 0);
    assert_eq!(events[1].sequence, 1);
    assert_eq!(events[2].sequence, 2);
    assert_eq!(events[2].content, "private trace");
}

#[test]
fn identical_echo_sequence_is_idempotent_but_conflicts_are_rejected() {
    let path = test_path();
    let mut rel = Cva::create_project(&path).unwrap();
    let first = event(0, EchoEventKind::ReasoningSummary, "summary");
    assert!(rel.put_echo_event(first.clone()).unwrap());
    assert!(!rel.put_echo_event(first).unwrap());

    let conflict = event(0, EchoEventKind::Commentary, "different");
    let error = rel.put_echo_event(conflict).unwrap_err();
    assert!(matches!(
        error,
        crate::CvaError::Echo(EchoError::ConflictingSequence)
    ));
}

#[test]
fn tool_and_activity_echo_records_use_typed_binary_fields() {
    let path = test_path();
    let mut rel = Cva::create_project(&path).unwrap();
    rel.put_echo_event(EchoEvent {
        conversation_id: "conversation-1".into(),
        message_id: "assistant-1".into(),
        sequence: 0,
        timestamp_ns: 1,
        model_round: Some(1),
        kind: EchoEventKind::ToolCall,
        correlation_id: Some("call-1".into()),
        name: Some("memory_search".into()),
        content: "query=routing".into(),
    })
    .unwrap();
    drop(rel);

    let reopened = Cva::open(&path).unwrap();
    let record = reopened
        .echo_events("conversation-1", "assistant-1")
        .pop()
        .unwrap();
    assert_eq!(record.kind, EchoEventKind::ToolCall);
    assert_eq!(record.correlation_id.as_deref(), Some("call-1"));
    assert_eq!(record.name.as_deref(), Some("memory_search"));
    assert_eq!(record.content, "query=routing");
}

#[test]
fn receiver_acceptance_echo_persists_and_is_never_rendered_as_context() {
    let path = test_path();
    let mut rel = Cva::create_project(&path).unwrap();
    let mut ack = event(0, EchoEventKind::ToolContextAccepted, "private manifest");
    ack.correlation_id = Some("delivery".into());
    ack.name = Some("receiver.v1".into());
    rel.put_echo_event(ack.clone()).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    let events = reopened.echo_events("conversation-1", "assistant-1");
    assert_eq!(events, vec![ack]);
    assert_eq!(
        crate::render_assistant_content_from_echo("answer", &events),
        "answer"
    );
}
