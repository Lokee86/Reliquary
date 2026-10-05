//! R2 proof separation: existing legacy Memories may be deliberately enrolled,
//! but can never become fictitious first-cycle Dream publications.
use super::*;

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let dir =
            std::env::temp_dir().join(format!("freshness-r2-legacy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("owner.rel")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn draft(name: &str) -> crate::MemoryDraft {
    crate::MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Legacy baseline: {name}"),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: name.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn legacy_baseline_is_explicit_non_birth_idempotent_and_preserved_on_reopen() {
    let temp = Temp::new();
    let path = temp.path();
    let mut rel = Cva::create(&path).unwrap();
    // Simulate a genuinely pre-Freshness accepted Memory without calling the
    // present-day facade's first-publication intent adapter.
    let (older, created) = crate::cva_memory_publish::publish_memory_parts(
        &rel.archive,
        &mut rel.memories,
        &mut rel.container,
        None,
        0,
        draft("older"),
    )
    .unwrap();
    assert!(created);
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    assert!(rel.freshness_record(older.id).is_none());
    assert!(!rel.freshness_is_publication_birth(older.id));
    rel.append_node(
        "baseline-turn".into(),
        "r2".into(),
        None,
        "user".into(),
        10,
        "accepted legacy baseline",
    )
    .unwrap();
    let at = rel.rel_turn_count();
    let version = rel.latest_global_version();
    assert!(rel.enroll_legacy_freshness_baseline(older.id).unwrap());
    let baseline = rel.freshness_record(older.id).unwrap();
    assert_eq!(baseline.admitted_at_turn, Some(at));
    assert_eq!(baseline.score, 100);
    assert!(!rel.freshness_is_publication_birth(older.id));
    assert_eq!(rel.latest_global_version(), version);

    // A forged first-cycle pass may not use a baseline-enrolled legacy Memory.
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), older.id);
    assert!(
        rel.begin_initial_dream_pass(
            pass_id,
            older.id,
            1,
            at,
            std::collections::BTreeSet::new(),
            rel.graph_version()
        )
        .is_err()
    );

    rel.append_node(
        "later".into(),
        "r2".into(),
        None,
        "assistant".into(),
        11,
        "next accepted turn",
    )
    .unwrap();
    assert!(!rel.enroll_legacy_freshness_baseline(older.id).unwrap());
    assert_eq!(
        rel.freshness_record(older.id).unwrap().admitted_at_turn,
        Some(at)
    );
    drop(rel);

    let mut reopened = Cva::open(&path).unwrap();
    assert!(!reopened.freshness_is_publication_birth(older.id));
    assert_eq!(
        reopened
            .freshness_record(older.id)
            .unwrap()
            .admitted_at_turn,
        Some(at)
    );
    assert!(!reopened.enroll_legacy_freshness_baseline(older.id).unwrap());

    let (newborn, _) = reopened.publish_memory(None, 0, draft("newborn")).unwrap();
    assert!(reopened.freshness_is_publication_birth(newborn.id));
    let before = std::fs::read(&path).unwrap();
    assert!(
        reopened
            .enroll_legacy_freshness_baseline(newborn.id)
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
