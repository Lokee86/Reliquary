//! Divergent REL reconciliation rebases Freshness events by accepted Archive identity.
use reliquary_memory::{AcceptedMemoryUseReceipt, Cva, MemoryDraft, MemoryId};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("freshness-reconcile-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Reconcile fixture".into(),
        content: "One durable fact".into(),
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
        source_time_ns: Some(0),
        mutation_id: "fixture-memory".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn add_turn(rel: &mut Cva, conversation: &str, node: &str, i: u64) {
    rel.append_node(
        node.into(),
        conversation.into(),
        None,
        if i % 2 == 0 { "user" } else { "assistant" }.into(),
        i as i64,
        "accepted activity",
    )
    .unwrap();
}

fn accepted_use(rel: &mut Cva, id: MemoryId, identity: &str) {
    rel.record_accepted_memory_use(
        AcceptedMemoryUseReceipt {
            owner_uuid: rel.owner_uuid().unwrap(),
            accepted_turn: rel.rel_turn_count(),
            use_id: identity.into(),
            memories: vec![id],
        },
        1,
    )
    .unwrap();
}

#[test]
fn active_birth_rebases_to_its_exact_destination_cut() {
    let fixture = Fixture::new();
    let left_path = fixture.path("birth-left.rel");
    let right_path = fixture.path("birth-right.rel");
    let merged_path = fixture.path("birth-merged.rel");
    Cva::create(&left_path).unwrap().sync().unwrap();
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    add_turn(&mut left, "left", "left-before-birth", 0);
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    add_turn(&mut right, "right", "right-1", 0);
    add_turn(&mut right, "right", "right-2", 1);
    let (memory, created) = right.publish_memory(None, 0, draft()).unwrap();
    assert!(created);
    let id = memory.id;
    assert_eq!(
        right.freshness_record(id).unwrap().admitted_at_turn,
        Some(2)
    );
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let merged = Cva::open(&merged_path).unwrap();
    let record = merged.freshness_record(id).unwrap();
    assert_eq!(record.admitted_at_turn, Some(3));
    assert!(!merged.freshness_records_at(2).unwrap().contains_key(&id));
    assert_eq!(merged.freshness_record_at(id, 3).unwrap().unwrap(), record);
}

#[test]
fn divergent_access_events_replay_at_their_rebased_archive_cuts() {
    let fixture = Fixture::new();
    let left_path = fixture.path("left.rel");
    let right_path = fixture.path("right.rel");
    let merged_path = fixture.path("merged.rel");
    let mut base = Cva::create(&left_path).unwrap();
    let (memory, created) = base.publish_memory(None, 0, draft()).unwrap();
    assert!(created);
    let id = memory.id;
    for i in 0..999 {
        add_turn(&mut base, "base", &format!("base-{i}"), i);
    }
    base.sync().unwrap();
    drop(base);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    for i in 0..9 {
        add_turn(&mut left, "left", &format!("left-{i}"), i);
    }
    assert_eq!(left.rel_turn_count(), 1008);
    accepted_use(&mut left, id, "left-delivery");
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    add_turn(&mut right, "right", "right-0", 0);
    add_turn(&mut right, "right", "right-1", 1);
    assert_eq!(right.rel_turn_count(), 1001);
    accepted_use(&mut right, id, "right-delivery");
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let merged = Cva::open(&merged_path).unwrap();
    // Left event at 1008; the right event's source turn 1001 rebases to 1010.
    // At that boundary one decay point elapses before its +25 effect.
    let after_left = merged.freshness_record_at(id, 1009).unwrap().unwrap();
    assert_eq!(after_left.score_at(1009).unwrap(), 25);
    let after_right = merged.freshness_record_at(id, 1010).unwrap().unwrap();
    assert_eq!(after_right.score_at(1010).unwrap(), 49);
    assert_eq!(merged.rel_turn_count(), 1010);
}

#[test]
fn accepted_community_proof_survives_two_divergent_reconciliations() {
    use reliquary_memory::{GraphRelationKind, GraphRelationOrigin};
    let fixture = Fixture::new();
    let base_path = fixture.path("proof-base.rel");
    let left_path = fixture.path("proof-left.rel");
    let right_path = fixture.path("proof-right.rel");
    let merged_path = fixture.path("proof-merged.rel");
    let extension_path = fixture.path("proof-extension.rel");
    let second_path = fixture.path("proof-second.rel");
    let mut base = Cva::create(&base_path).unwrap();
    let (a, _) = base.publish_memory(None, 0, draft()).unwrap();
    let mut other = draft();
    other.title = "Second proof fixture".into();
    other.content = "Connected durable fact".into();
    other.mutation_id = "proof-peer".into();
    let (b, _) = base.publish_memory(None, 0, other).unwrap();
    base.set_memory_relation_with_origin(
        a.id,
        b.id,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        base.graph_version(),
    )
    .unwrap();
    let snapshot = base.refresh_communities_leiden().unwrap();
    assert_eq!(snapshot.derived_graph_version, base.memory_graph_version());
    for i in 0..1000 {
        add_turn(&mut base, "base", &format!("proof-base-{i}"), i);
    }
    accepted_use(&mut base, a.id, "common-community-use");
    let source_scores = [a.id, b.id].map(|id| base.freshness_record(id).unwrap().score);
    base.sync().unwrap();
    drop(base);
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();
    for (path, prefix) in [(&left_path, "left"), (&right_path, "right")] {
        let mut branch = Cva::open(path).unwrap();
        add_turn(&mut branch, prefix, &format!("proof-{prefix}"), 0);
        branch.sync().unwrap();
    }
    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let merged = Cva::open(&merged_path).unwrap();
    assert!(
        merged.community_snapshot().is_none(),
        "reconciliation preserves accepted event proof without manufacturing a Community snapshot"
    );
    for (id, expected) in [a.id, b.id].into_iter().zip(source_scores) {
        assert_eq!(merged.freshness_record(id).unwrap().score, expected);
    }
    drop(merged);
    fs::copy(&merged_path, &extension_path).unwrap();
    {
        let mut merged = Cva::open(&merged_path).unwrap();
        add_turn(&mut merged, "merged", "proof-post-merge-left", 0);
        merged.sync().unwrap();
        let mut extension = Cva::open(&extension_path).unwrap();
        add_turn(&mut extension, "extension", "proof-post-merge-right", 0);
        extension.sync().unwrap();
    }
    Cva::reconcile(&merged_path, &extension_path, &second_path).unwrap();
    let mut second = Cva::open(&second_path).unwrap();
    for (id, expected) in [a.id, b.id].into_iter().zip(source_scores) {
        assert_eq!(second.freshness_record(id).unwrap().score, expected);
    }
    let bytes = fs::read(&second_path).unwrap();
    assert_eq!(
        second
            .record_accepted_memory_use(
                AcceptedMemoryUseReceipt {
                    owner_uuid: second.owner_uuid().unwrap(),
                    accepted_turn: 1000,
                    use_id: "common-community-use".into(),
                    memories: vec![a.id],
                },
                2
            )
            .unwrap(),
        0
    );
    assert_eq!(fs::read(&second_path).unwrap(), bytes);
}
