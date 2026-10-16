use crate::lexical_search::lexical_candidates_parts;
use crate::{Cva, FragmentConfig, MemoryDraft, MemoryId, Phylactery};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-lexical-index-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn add_fragment(cva: &mut Cva, conversation: &str, text: &str) {
    for index in 0..8 {
        cva.append_node(
            format!("{conversation}-n{index}"),
            conversation.into(),
            (index > 0).then(|| format!("{conversation}-n{}", index - 1)),
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            text,
        )
        .unwrap();
    }
    cva.materialize_path_fragments(
        conversation,
        &format!("{conversation}-n7"),
        FragmentConfig::default(),
        false,
    )
    .unwrap();
}

fn memory_draft(key: &str, title: &str, content: &str, archived: bool) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: content.into(),
        scope: "test".into(),
        lifecycle_state: "extracted".into(),
        archived,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: key.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn add_rel_memory(
    cva: &mut Cva,
    key: &str,
    title: &str,
    content: &str,
    archived: bool,
) -> MemoryId {
    cva.publish_memory(None, 0, memory_draft(key, title, content, archived))
        .unwrap()
        .0
        .id
}

fn add_phy_memory(
    phy: &mut Phylactery,
    key: &str,
    title: &str,
    content: &str,
    archived: bool,
) -> MemoryId {
    phy.publish_memory(None, 0, memory_draft(key, title, content, archived))
        .unwrap()
        .0
        .id
}

#[test]
fn complete_memory_lexical_lane_keeps_matches_beyond_public_limit() {
    let mut rel = Cva::create(test_path("memory-complete.rel")).unwrap();
    let mut expected = Vec::new();
    for index in 0..32 {
        expected.push(add_rel_memory(
            &mut rel,
            &format!("complete-{index}"),
            &format!("Shared needle {index}"),
            "complete lane evidence",
            false,
        ));
    }
    let archived = add_rel_memory(
        &mut rel,
        "archived-complete",
        "Shared needle",
        "complete lane evidence",
        true,
    );
    let before = (
        rel.rel_turn_count(),
        rel.memory_version(),
        rel.graph_version(),
    );
    let complete = rel.complete_memory_lexical_matches("needle").unwrap();
    assert_eq!(complete.len(), 32);
    assert!(!complete.iter().any(|hit| hit.memory_id == archived));
    let public = rel.search_memories("needle", 10).unwrap();
    assert_eq!(public.len(), 10);
    assert_eq!(
        public.iter().map(|hit| hit.memory.id).collect::<Vec<_>>(),
        complete
            .iter()
            .take(10)
            .map(|hit| hit.memory_id)
            .collect::<Vec<_>>(),
    );
    let mut actual = complete.iter().map(|hit| hit.memory_id).collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(
        before,
        (
            rel.rel_turn_count(),
            rel.memory_version(),
            rel.graph_version()
        )
    );
}

#[test]
fn indexed_lexical_results_match_full_scan_exactly() {
    let path = test_path("equivalence.cva");
    let mut cva = Cva::create(path).unwrap();
    add_fragment(&mut cva, "a", "alpha alphabet alpha beta");
    add_fragment(&mut cva, "b", "alpha beta beta beta");
    add_fragment(&mut cva, "c", "gamma beta delta");

    for query in ["alpha", "beta alpha", "alphabet", "gamma delta", "missing"] {
        let indexed = cva.lexical_candidates(query, 30).unwrap();
        let scanned =
            lexical_candidates_parts(&cva.archive, &mut cva.container, query, 30).unwrap();
        assert_eq!(indexed.len(), scanned.len(), "query={query}");
        for (indexed, scanned) in indexed.iter().zip(&scanned) {
            assert_eq!(indexed.fragment, scanned.fragment, "query={query}");
            assert_eq!(
                indexed.score.to_bits(),
                scanned.score.to_bits(),
                "query={query}"
            );
        }
    }
}

#[test]
fn lexical_index_is_disposable_and_rebuilds_after_reopen() {
    let path = test_path("reopen.cva");
    let mut cva = Cva::create(&path).unwrap();
    add_fragment(&mut cva, "c1", "reopenable lexical state");
    cva.sync().unwrap();
    let before = fs::metadata(&path).unwrap().len();

    assert_eq!(cva.lexical_candidates("reopenable", 10).unwrap().len(), 1);
    cva.sync().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), before);
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.lexical_candidates("reopenable", 10).unwrap().len(),
        1
    );
    reopened.sync().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), before);
}

#[test]
fn lexical_index_incrementally_adds_new_fragments() {
    let path = test_path("incremental.cva");
    let mut cva = Cva::create(path).unwrap();
    add_fragment(&mut cva, "first", "firstunique durable state");
    assert_eq!(cva.lexical_candidates("firstunique", 10).unwrap().len(), 1);

    add_fragment(&mut cva, "second", "secondunique durable state");
    let second = cva.lexical_candidates("secondunique", 10).unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].fragment.conversation_id, "second");

    let both = cva.lexical_candidates("durable state", 10).unwrap();
    assert_eq!(both.len(), 2);
}

#[test]
fn memory_lexical_index_refreshes_from_full_memory_text_and_excludes_archived() {
    let mut cva = Cva::create(test_path("memory-refresh.rel")).unwrap();
    let first = add_rel_memory(&mut cva, "m1", "Alpha system", "durable cedar state", false);
    let archived = add_rel_memory(&mut cva, "m2", "Alpha archive", "durable cedar state", true);

    let alpha = cva.search_memories("alpha", 10).unwrap();
    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].memory.id, first);
    assert!(alpha.iter().all(|hit| hit.memory.id != archived));

    let second = add_rel_memory(&mut cva, "m3", "Beta system", "durable cedar state", false);
    let durable = cva.search_memories("durable cedar", 10).unwrap();
    assert_eq!(durable.len(), 2);
    assert!(durable.iter().any(|hit| hit.memory.id == first));
    assert!(durable.iter().any(|hit| hit.memory.id == second));
}

#[test]
fn memory_lexical_index_is_disposable_for_rel_and_phy() {
    let rel_path = test_path("memory-reopen.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    add_rel_memory(
        &mut rel,
        "rel-memory",
        "Reliquary",
        "reopenable memory lexical state",
        false,
    );
    rel.sync().unwrap();
    let rel_size = fs::metadata(&rel_path).unwrap().len();
    assert_eq!(rel.search_memories("reopenable", 10).unwrap().len(), 1);
    rel.sync().unwrap();
    assert_eq!(fs::metadata(&rel_path).unwrap().len(), rel_size);
    drop(rel);
    let mut rel = Cva::open(&rel_path).unwrap();
    assert_eq!(rel.search_memories("reopenable", 10).unwrap().len(), 1);

    let phy_path = test_path("memory-reopen.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    add_phy_memory(
        &mut phy,
        "phy-memory",
        "Preference",
        "reopenable private lexical state",
        false,
    );
    phy.sync().unwrap();
    let phy_size = fs::metadata(&phy_path).unwrap().len();
    assert_eq!(phy.search_memories("reopenable", 10).unwrap().len(), 1);
    phy.sync().unwrap();
    assert_eq!(fs::metadata(&phy_path).unwrap().len(), phy_size);
    drop(phy);
    let mut phy = Phylactery::open(&phy_path).unwrap();
    assert_eq!(phy.search_memories("reopenable", 10).unwrap().len(), 1);
}
#[test]
fn memory_lexical_index_updates_slots_incrementally_across_archive_state_changes() {
    let mut cva = Cva::create(test_path("memory-incremental.rel")).unwrap();
    let first = add_rel_memory(&mut cva, "m1", "Alpha system", "durable cedar state", false);
    add_rel_memory(&mut cva, "m2", "Beta system", "durable cedar state", false);

    assert_eq!(cva.search_memories("durable", 10).unwrap().len(), 2);
    assert_eq!(cva.lexical_index.memory_indexed_slots(), 2);
    assert_eq!(cva.lexical_index.indexed_memory_version(), 2);

    let archived = memory_draft("m1-archive", "Alpha system", "durable cedar state", true);
    cva.publish_memory(Some(first), 1, archived).unwrap();
    assert!(cva.search_memories("alpha", 10).unwrap().is_empty());
    assert_eq!(cva.lexical_index.memory_indexed_slots(), 2);
    assert_eq!(cva.lexical_index.indexed_memory_version(), 3);

    let restored = memory_draft("m1-restore", "Alpha system", "durable cedar state", false);
    cva.publish_memory(Some(first), 2, restored).unwrap();
    let alpha = cva.search_memories("alpha", 10).unwrap();
    assert_eq!(alpha.len(), 1);
    assert_eq!(alpha[0].memory.id, first);
    assert_eq!(cva.lexical_index.memory_indexed_slots(), 2);
    assert_eq!(cva.lexical_index.indexed_memory_version(), 4);
}
