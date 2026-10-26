use crate::Cva;
use std::fs;
use uuid::Uuid;

fn rel() -> Cva {
    let dir = std::env::temp_dir().join(format!("reliquary-archive-search-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    Cva::create_project(dir.join("project.rel")).unwrap()
}

#[test]
fn archive_search_reads_raw_turns_without_materialized_fragments() {
    let mut rel = rel();
    rel.append_node(
        "u1".into(),
        "history".into(),
        None,
        "user".into(),
        1,
        "historical alphaarchivetoken detail",
    )
    .unwrap();
    rel.append_node(
        "a1".into(),
        "history".into(),
        Some("u1".into()),
        "assistant".into(),
        2,
        "ack",
    )
    .unwrap();

    assert_eq!(rel.stats().fragments, 0);
    let hits = rel.search_archive("alphaarchivetoken", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].fragment.conversation_id, "history");
    assert_eq!(hits[0].fragment.start_node_id, "u1");
    assert_eq!(hits[0].fragment.end_node_id, "u1");
    assert!(hits[0].text.contains("alphaarchivetoken"));
    assert_eq!(rel.stats().fragments, 0);
}

#[test]
fn archive_search_indexes_new_live_turns_incrementally() {
    let mut rel = rel();
    rel.append_node(
        "u1".into(),
        "history".into(),
        None,
        "user".into(),
        1,
        "firstarchivetoken",
    )
    .unwrap();
    assert_eq!(rel.search_archive("firstarchivetoken", 5).unwrap().len(), 1);

    rel.append_node(
        "a1".into(),
        "history".into(),
        Some("u1".into()),
        "assistant".into(),
        2,
        "secondarchivetoken",
    )
    .unwrap();
    let hits = rel.search_archive("secondarchivetoken", 5).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].fragment.start_node_id, "a1");
}

#[test]
fn archive_search_prefers_newer_equal_score_turns() {
    let mut rel = rel();
    for (id, timestamp) in [("old", 1), ("new", 2)] {
        rel.append_node(
            id.into(),
            format!("conversation-{id}"),
            None,
            "user".into(),
            timestamp,
            "sharedarchivetoken",
        )
        .unwrap();
    }

    let hits = rel.search_archive("sharedarchivetoken", 5).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].fragment.start_node_id, "new");
    assert_eq!(hits[1].fragment.start_node_id, "old");
}
