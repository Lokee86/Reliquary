use reliquary_memory::{Cva, imports::*};
use std::fs;
fn run(incoming: NativeImport, expected_nodes: usize) -> ImportReceipt {
    let path = std::env::temp_dir().join(format!("p5g-public-{}.rel", uuid::Uuid::new_v4()));
    let mut rel = Cva::create(&path).unwrap();
    let receipt = rel
        .import_native(&incoming, 1_800_000_000_000_000_000)
        .unwrap();
    assert_eq!(rel.stats().nodes, expected_nodes);
    for conversation in &receipt.conversations {
        for branch in &conversation.branches {
            if let Some(leaf) = &branch.leaf_node_id {
                let turns = rel
                    .conversation_turns(&conversation.conversation_id, leaf)
                    .unwrap();
                assert!(turns.iter().all(|t| !t.content.contains("PRIVATE MARKER")));
            }
        }
    }
    let versions = (rel.archive_version(), rel.latest_global_version());
    drop(rel);
    let mut rel = Cva::open(&path).unwrap();
    assert_eq!(
        rel.import_native(&incoming, 1_900_000_000_000_000_000)
            .unwrap(),
        receipt
    );
    assert_eq!(
        versions,
        (rel.archive_version(), rel.latest_global_version())
    );
    drop(rel);
    assert!(!String::from_utf8_lossy(&fs::read(&path).unwrap()).contains("PRIVATE MARKER"));
    fs::remove_file(path).unwrap();
    receipt
}
#[test]
fn chatgpt_golden_branch_profile() {
    let incoming = normalize_chatgpt(
        "chatgpt/account-1",
        include_bytes!("fixtures/native_import/chatgpt-mapping-text-v1.json"),
    )
    .unwrap();
    let c = &incoming.conversations[0];
    assert_eq!(
        c.messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        ["First answer", "Revised answer", "Question"]
    );
    assert_eq!(c.branches.len(), 2);
    assert_eq!(
        c.branches
            .iter()
            .filter(|b| b.canonical)
            .map(|b| b.leaf_message_id.as_str())
            .collect::<Vec<_>>(),
        ["a2"]
    );
    run(incoming, 3);
}
#[test]
fn claude_golden_text_profile() {
    let incoming = normalize_claude(
        "claude/account-1",
        include_bytes!("fixtures/native_import/claude-uuid-text-v1.json"),
    )
    .unwrap();
    assert_eq!(
        incoming.conversations[0]
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        ["Question", "Answer"]
    );
    assert_eq!(
        incoming.conversations[0].messages[1].parent_id.as_deref(),
        Some("cu")
    );
    run(incoming, 2);
}
#[test]
fn hermes_golden_export_profile() {
    let incoming = normalize_hermes_jsonl(
        "hermes/db-1",
        include_bytes!("fixtures/native_import/hermes-session-jsonl-text-v1.jsonl"),
    )
    .unwrap();
    assert_eq!(incoming.conversations[0].messages[1].id, "11");
    assert_eq!(
        incoming.conversations[0].messages[1].parent_id.as_deref(),
        Some("10")
    );
    assert_eq!(incoming.conversations[0].messages[1].content, "Answer");
    run(incoming, 2);
}
#[test]
fn structured_agent_golden_revision_artifact_profile() {
    let incoming = normalize_agent(include_bytes!(
        "fixtures/native_import/structured-agent-v1.json"
    ))
    .unwrap();
    assert!(incoming.conversations[0].messages[1].deleted);
    assert_eq!(
        incoming.conversations[0].messages[2].replaces.as_deref(),
        Some("removed")
    );
    assert_eq!(
        incoming.conversations[0].messages[2].artifacts[0].bytes,
        b"abc"
    );
    let receipt = run(incoming, 2);
    assert_eq!(
        receipt.import_key,
        "2eb82c1bbaa6b0e444a44b0c0515454e3644f778a856a36068bef4d048f965c5"
    );
}
#[test]
fn unsupported_profiles_fail_explicitly() {
    assert!(normalize_agent(br#"{"schema_version":99,"source_namespace":"x","source_format":"x","conversations":[]}"#).is_err());
    let mut chat: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/native_import/chatgpt-mapping-text-v1.json"
    ))
    .unwrap();
    chat[0]["mapping"]["u"]["message"]["content"]["content_type"] = "multimodal_text".into();
    assert!(matches!(
        normalize_chatgpt("x", &serde_json::to_vec(&chat).unwrap()),
        Err(ImportError::Unsupported(_))
    ));
    for (field, value) in [
        ("channel", serde_json::json!("unknown")),
        ("recipient", serde_json::json!("tool-name")),
    ] {
        let mut source: serde_json::Value = serde_json::from_slice(include_bytes!(
            "fixtures/native_import/chatgpt-mapping-text-v1.json"
        ))
        .unwrap();
        source[0]["mapping"]["u"]["message"][field] = value;
        assert!(matches!(
            normalize_chatgpt("x", &serde_json::to_vec(&source).unwrap()),
            Err(ImportError::Unsupported(_))
        ));
    }
    let mut claude: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/native_import/claude-uuid-text-v1.json"
    ))
    .unwrap();
    claude[0]["chat_messages"][0]["attachments"] = serde_json::json!({"unexpected": "shape"});
    assert!(matches!(
        normalize_claude("x", &serde_json::to_vec(&claude).unwrap()),
        Err(ImportError::Unsupported(_))
    ));
    let mut hermes: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/native_import/hermes-session-jsonl-text-v1.jsonl"
    ))
    .unwrap();
    hermes["messages"][0].as_object_mut().unwrap().remove("id");
    assert!(matches!(
        normalize_hermes_jsonl("x", &serde_json::to_vec(&hermes).unwrap()),
        Err(ImportError::Unsupported(_))
    ));
    assert!(
        normalize_hermes_jsonl(
            "x",
            br#"{"session_id":"x","index":1,"role":"user","text":"prompt only"}"#
        )
        .is_err()
    );
}
