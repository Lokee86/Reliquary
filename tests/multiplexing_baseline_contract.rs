//! Phase 0 characterization: one REL already has independent session-ID mechanics.
//! This is deliberately below the future GatewayRuntime/InstanceRuntime API.
//! Multi-instance synchronization and authorization belong to target-architecture tests.
use reliquary_memory::{Cva, InteractionError, InteractionRole, InteractionRuntime};

#[test]
fn one_rel_can_keep_distinct_open_conversations_without_crossing_ancestry() {
    let path = std::env::temp_dir().join(format!(
        "reliquary-multiplex-baseline-{}.rel",
        uuid::Uuid::new_v4()
    ));
    let mut runtime = InteractionRuntime::new(Cva::create(&path).unwrap());

    runtime.open_session("conversation-a".into(), None).unwrap();
    runtime.open_session("conversation-b".into(), None).unwrap();

    for (conversation, message) in [
        ("conversation-a", "message-a"),
        ("conversation-b", "message-b"),
    ] {
        runtime
            .begin_message(conversation, message.into(), InteractionRole::User, 1)
            .unwrap();
        runtime
            .append_text(conversation, message, conversation)
            .unwrap();
        let receipt = runtime.complete_message(conversation, message).unwrap();
        assert_eq!(receipt.turn.node.conversation_id, conversation);
        assert_eq!(receipt.turn.node.parent_id, None);
    }

    assert_eq!(
        runtime
            .session("conversation-a")
            .unwrap()
            .leaf_message_id
            .as_deref(),
        Some("message-a")
    );
    assert_eq!(
        runtime
            .session("conversation-b")
            .unwrap()
            .leaf_message_id
            .as_deref(),
        Some("message-b")
    );
    runtime.close_session("conversation-a").unwrap();
    assert!(runtime.session("conversation-b").is_some());

    let cva = runtime.into_cva();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = InteractionRuntime::new(Cva::open(&path).unwrap());
    assert!(matches!(
        reopened.open_session("conversation-a".into(), None),
        Err(InteractionError::ResumeRequired)
    ));
    reopened
        .open_session("conversation-a".into(), Some("message-a".into()))
        .unwrap();
    reopened
        .open_session("conversation-b".into(), Some("message-b".into()))
        .unwrap();
    assert_eq!(
        reopened
            .session("conversation-b")
            .unwrap()
            .leaf_message_id
            .as_deref(),
        Some("message-b")
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
