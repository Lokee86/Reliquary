use super::evidence::{parse_evidence_requests, resolve_evidence_parts};
use crate::{Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, FragmentConfig};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-insomnia-evidence-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append(
    cva: &mut Cva,
    conversation: &str,
    id: &str,
    parent: Option<&str>,
    role: &str,
    time: i64,
    content: &str,
) {
    cva.append_node(
        id.into(),
        conversation.into(),
        parent.map(str::to_owned),
        role.into(),
        time,
        content,
    )
    .unwrap();
}

#[test]
fn evidence_resolves_exact_turn_range_and_lexical_search() {
    let path = test_path("reads.cva");
    let mut cva = Cva::create(path).unwrap();
    append(
        &mut cva,
        "old",
        "u0",
        None,
        "user",
        1,
        "Deployment question.",
    );
    append(
        &mut cva,
        "old",
        "a0",
        Some("u0"),
        "assistant",
        2,
        "The deployment target is Windows and Linux.",
    );
    append(&mut cva, "old", "u1", Some("a0"), "user", 3, "Okay.");
    cva.materialize_path_fragments("old", "u1", FragmentConfig::default(), true)
        .unwrap();

    append(
        &mut cva,
        "current",
        "u2",
        None,
        "user",
        10,
        "Remember the deployment target we discussed.",
    );
    let episode = cva
        .materialize_path_episodes(
            "current",
            "u2",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::CreateMemory, 11)),
        )
        .unwrap()
        .created
        .remove(0);

    let requests = parse_evidence_requests(&json!({
        "evidence_requests": [
            {"kind":"turn", "conversation_id":"old", "node_id":"a0"},
            {"kind":"conversation_range", "conversation_id":"old", "start_node_id":"u0", "end_node_id":"u1"},
            {"kind":"archive_search", "query":"deployment target", "limit":1}
        ]
    }))
    .unwrap();
    let (results, turns) = resolve_evidence_parts(
        &crate::archive::ArchiveReadView::new(&cva.owner.archive, &cva.owner.container),
        &mut cva.lexical_index,
        &episode,
        &requests,
    )
    .unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|result| result.error.is_none()));
    assert!(
        turns
            .iter()
            .any(|turn| turn.conversation_id == "old" && turn.node_id == "a0")
    );
    assert!(turns.len() <= crate::MAX_INSOMNIA_EVIDENCE_TURNS);
}

#[test]
fn evidence_request_count_is_bounded() {
    let value = json!({"evidence_requests": [
        {"kind":"turn","conversation_id":"c","node_id":"1"},
        {"kind":"turn","conversation_id":"c","node_id":"2"},
        {"kind":"turn","conversation_id":"c","node_id":"3"},
        {"kind":"turn","conversation_id":"c","node_id":"4"},
        {"kind":"turn","conversation_id":"c","node_id":"5"}
    ]});
    assert!(parse_evidence_requests(&value).is_err());
}
