use crate::{
    ArchiveError, Cva, DEFAULT_EPISODE_INACTIVITY_NS, EpisodeBoundary, EpisodeConfig,
    EpisodeOrigin, EpisodePolicy,
};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-episodes-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append_turn(
    cva: &mut Cva,
    conversation: &str,
    id: &str,
    parent: Option<&str>,
    role: &str,
    timestamp_ns: i64,
    content: &str,
) {
    cva.append_node(
        id.to_owned(),
        conversation.to_owned(),
        parent.map(str::to_owned),
        role.to_owned(),
        timestamp_ns,
        content,
    )
    .unwrap();
}

#[test]
fn episodes_pack_whole_response_cycles_and_leave_only_the_tail_open() {
    let path = test_path("packing.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "first");
    append_turn(
        &mut cva,
        "c1",
        "a0",
        Some("u0"),
        "assistant",
        20,
        "reply one",
    );
    append_turn(&mut cva, "c1", "u1", Some("a0"), "user", 30, "second");
    append_turn(
        &mut cva,
        "c1",
        "a1",
        Some("u1"),
        "assistant",
        40,
        "reply two",
    );
    append_turn(&mut cva, "c1", "u2", Some("a1"), "user", 50, "third");
    append_turn(
        &mut cva,
        "c1",
        "a2",
        Some("u2"),
        "assistant",
        60,
        "reply three",
    );

    let result = cva
        .materialize_path_episodes(
            "c1",
            "a2",
            EpisodeConfig { max_input_bytes: 1 },
            EpisodeOrigin::Live,
            None,
        )
        .unwrap();

    assert_eq!(result.created.len(), 2);
    assert!(result.has_open_tail);
    assert_eq!(result.open_from_node_id.as_deref(), Some("u2"));
    assert_eq!(result.created[0].start_node_id, "u0");
    assert_eq!(result.created[0].end_node_id, "a0");
    assert_eq!(result.created[1].start_node_id, "u1");
    assert_eq!(result.created[1].end_node_id, "a1");
    assert!(
        result
            .created
            .iter()
            .all(|episode| episode.boundary == EpisodeBoundary::Size)
    );
}

#[test]
fn forced_tail_finalization_does_not_close_the_conversation() {
    let path = test_path("appendable.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "remember this");
    append_turn(&mut cva, "c1", "a0", Some("u0"), "assistant", 20, "ack");

    let first = cva
        .materialize_path_episodes(
            "c1",
            "a0",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::CreateMemory, 25)),
        )
        .unwrap();
    assert_eq!(first.created.len(), 1);
    assert_eq!(first.created[0].boundary, EpisodeBoundary::CreateMemory);
    assert!(!first.has_open_tail);

    append_turn(
        &mut cva,
        "c1",
        "u1",
        Some("a0"),
        "user",
        30,
        "continue later",
    );
    append_turn(
        &mut cva,
        "c1",
        "a1",
        Some("u1"),
        "assistant",
        40,
        "continued",
    );

    let open = cva
        .materialize_path_episodes(
            "c1",
            "a1",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            None,
        )
        .unwrap();
    assert!(open.created.is_empty());
    assert!(open.has_open_tail);
    assert_eq!(open.open_from_node_id.as_deref(), Some("u1"));

    let inactive = cva
        .materialize_path_episodes(
            "c1",
            "a1",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 55)),
        )
        .unwrap();
    assert_eq!(inactive.created.len(), 1);
    assert_eq!(inactive.created[0].start_node_id, "u1");
    assert_eq!(inactive.created[0].end_node_id, "a1");
    assert_eq!(inactive.created[0].boundary, EpisodeBoundary::Inactivity);

    cva.sync().unwrap();
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.stats().episodes, 2);
    assert_eq!(reopened.episodes_for_conversation("c1").len(), 2);
}

#[test]
fn import_end_finalizes_available_tail_without_terminal_conversation_state() {
    let path = test_path("import.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "imported", "u0", None, "user", 10, "old question");
    append_turn(
        &mut cva,
        "imported",
        "a0",
        Some("u0"),
        "assistant",
        20,
        "old answer",
    );

    let imported = cva
        .materialize_path_episodes(
            "imported",
            "a0",
            EpisodeConfig::default(),
            EpisodeOrigin::Import,
            Some((EpisodeBoundary::ImportEnd, 1000)),
        )
        .unwrap();
    assert_eq!(imported.created.len(), 1);
    assert_eq!(imported.created[0].origin, EpisodeOrigin::Import);
    assert_eq!(imported.created[0].boundary, EpisodeBoundary::ImportEnd);

    append_turn(
        &mut cva,
        "imported",
        "u1",
        Some("a0"),
        "user",
        30,
        "later recovered turn",
    );
    append_turn(
        &mut cva,
        "imported",
        "a1",
        Some("u1"),
        "assistant",
        40,
        "later recovered answer",
    );
    let later = cva
        .materialize_path_episodes(
            "imported",
            "a1",
            EpisodeConfig::default(),
            EpisodeOrigin::Import,
            Some((EpisodeBoundary::ImportEnd, 1100)),
        )
        .unwrap();
    assert_eq!(later.created.len(), 1);
    assert_eq!(later.created[0].start_node_id, "u1");
}

#[test]
fn inactivity_finalization_is_derived_from_durable_source_time() {
    let path = test_path("inactivity.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "question");
    append_turn(&mut cva, "c1", "a0", Some("u0"), "assistant", 20, "answer");
    let policy = EpisodePolicy::default();

    let early = cva
        .finalize_inactive_path_episodes(
            "c1",
            "a0",
            policy,
            EpisodeOrigin::Live,
            20 + DEFAULT_EPISODE_INACTIVITY_NS - 1,
        )
        .unwrap();
    assert!(early.is_none());
    assert_eq!(cva.stats().episodes, 0);

    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    let due = reopened
        .finalize_inactive_path_episodes(
            "c1",
            "a0",
            policy,
            EpisodeOrigin::Live,
            20 + DEFAULT_EPISODE_INACTIVITY_NS,
        )
        .unwrap()
        .unwrap();
    assert_eq!(due.created.len(), 1);
    assert_eq!(due.created[0].boundary, EpisodeBoundary::Inactivity);
}

#[test]
fn explicit_finalization_closes_only_the_current_tail_and_allows_continuation() {
    let path = test_path("explicit.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "first");
    append_turn(&mut cva, "c1", "a0", Some("u0"), "assistant", 20, "reply");

    let explicit = cva
        .finalize_explicit_path("c1", "a0", EpisodeConfig::default(), 25)
        .unwrap();
    assert_eq!(explicit.created.len(), 1);
    assert_eq!(explicit.created[0].boundary, EpisodeBoundary::Explicit);
    assert!(!explicit.has_open_tail);

    append_turn(&mut cva, "c1", "u1", Some("a0"), "user", 30, "second");
    append_turn(
        &mut cva,
        "c1",
        "a1",
        Some("u1"),
        "assistant",
        40,
        "reply two",
    );
    let open = cva
        .materialize_path_episodes(
            "c1",
            "a1",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            None,
        )
        .unwrap();
    assert!(open.created.is_empty());
    assert!(open.has_open_tail);
    assert_eq!(open.open_from_node_id.as_deref(), Some("u1"));
}

#[test]
fn create_memory_helper_only_finalizes_available_source() {
    let path = test_path("create-memory.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "remember this");
    append_turn(&mut cva, "c1", "a0", Some("u0"), "assistant", 20, "ack");
    let result = cva
        .finalize_create_memory_path("c1", "a0", EpisodeConfig::default(), 25)
        .unwrap();
    assert_eq!(result.created.len(), 1);
    assert_eq!(result.created[0].boundary, EpisodeBoundary::CreateMemory);
    assert_eq!(cva.stats().episodes, 1);

    let repeated = cva
        .finalize_create_memory_path("c1", "a0", EpisodeConfig::default(), 30)
        .unwrap();
    assert!(repeated.created.is_empty());
    assert!(!repeated.has_open_tail);
}

#[test]
fn episode_authority_rejects_a_branch_that_diverges_before_the_completed_prefix() {
    let path = test_path("branch-conflict.cva");
    let mut cva = Cva::create(&path).unwrap();
    append_turn(&mut cva, "c1", "u0", None, "user", 10, "question");
    append_turn(&mut cva, "c1", "left", Some("u0"), "assistant", 20, "left");
    cva.materialize_path_episodes(
        "c1",
        "left",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap();

    append_turn(
        &mut cva,
        "c1",
        "right",
        Some("u0"),
        "assistant",
        21,
        "right",
    );
    let error = cva
        .materialize_path_episodes(
            "c1",
            "right",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 31)),
        )
        .unwrap_err();
    assert!(matches!(error, ArchiveError::EpisodeBranchConflict));
}
