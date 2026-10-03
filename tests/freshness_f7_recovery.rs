//! Public owner-level recovery path: a returned accepted-use receipt is durable
//! without relying on a caller-level sync, and replay after reopen cannot reapply it.
use reliquary_memory::{AcceptedMemoryUseReceipt, Cva, GraphRelationKind, MemoryDraft};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("freshness-f7-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("owner.rel")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Recovery fixture: {name}"),
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

fn add_turns(rel: &mut Cva, count: usize) {
    for n in 0..count {
        rel.append_node(
            format!("turn-{n}"),
            "recovery".into(),
            None,
            if n % 2 == 0 { "user" } else { "assistant" }.into(),
            n as i64,
            "accepted owner activity",
        )
        .unwrap();
    }
}

#[test]
fn accepted_use_effect_batches_recover_and_retry_after_close_without_caller_sync() {
    let temp = Temp::new();
    let path = temp.path();
    let mut rel = Cva::create(&path).unwrap();
    let (left, _) = rel.publish_memory(None, 0, draft("left")).unwrap();
    let (right, _) = rel.publish_memory(None, 0, draft("right")).unwrap();
    rel.set_memory_relation(
        left.id,
        right.id,
        GraphRelationKind::Factual,
        true,
        rel.graph_version(),
    )
    .unwrap();
    add_turns(&mut rel, 2_100);
    let receipt = AcceptedMemoryUseReceipt {
        owner_uuid: rel.owner_uuid().unwrap(),
        accepted_turn: rel.rel_turn_count(),
        use_id: "crash-after-return".into(),
        memories: vec![left.id, right.id],
    };
    assert_eq!(
        rel.record_accepted_memory_use(receipt.clone(), 4).unwrap(),
        2
    );
    let committed = [left.id, right.id].map(|id| rel.freshness_record(id).unwrap());
    assert!(committed.iter().all(|record| record.score > -100));
    drop(rel); // no explicit caller-level sync after accepted-use return

    let mut reopened = Cva::open(&path).unwrap();
    let recovered = [left.id, right.id].map(|id| reopened.freshness_record(id).unwrap());
    assert_eq!(recovered, committed);
    assert_eq!(reopened.record_accepted_memory_use(receipt, 1).unwrap(), 0);
    let after_retry = [left.id, right.id].map(|id| reopened.freshness_record(id).unwrap());
    assert_eq!(after_retry, committed);
}
