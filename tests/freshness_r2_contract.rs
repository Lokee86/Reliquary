//! R2 durability and provenance acceptance: invalid operations leave owner bytes
//! unchanged, accepted delivery retries use their original durable receipt after
//! graph changes and reopen, and Freshness does not allocate semantic versions.
use reliquary_memory::{AcceptedMemoryUseReceipt, Cva, GraphRelationKind, MemoryDraft, MemoryId};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("freshness-r2-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
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
        content: format!("R2 accepted evidence {name}"),
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
fn invalid_batches_never_write_and_old_receipt_survives_graph_changes_and_reopen() {
    let temp = Temp::new();
    let path = temp.path();
    let mut owner = Cva::create(&path).unwrap();
    let (left, _) = owner.publish_memory(None, 0, draft("left")).unwrap();
    let (right, _) = owner.publish_memory(None, 0, draft("right")).unwrap();
    // A decayed source makes an accidental duplicate +25 observable.
    for n in 0..350 {
        owner
            .append_node(
                format!("r2-turn-{n}"),
                "r2".into(),
                None,
                if n % 2 == 0 { "user" } else { "assistant" }.into(),
                n as i64,
                "accepted",
            )
            .unwrap();
    }
    let accepted_turn = owner.rel_turn_count();
    let origin = owner.owner_uuid().unwrap();
    let accepted = AcceptedMemoryUseReceipt {
        owner_uuid: origin,
        accepted_turn,
        use_id: "r2-owner-accepted-use".into(),
        memories: vec![left.id],
    };
    let clean = fs::read(&path).unwrap();
    let version = owner.latest_global_version();
    let initial = owner.freshness_record(left.id).unwrap();

    let wrong_owner = AcceptedMemoryUseReceipt {
        owner_uuid: [0xff; 16],
        ..accepted.clone()
    };
    assert!(owner.record_accepted_memory_use(wrong_owner, 1).is_err());
    let wrong_cut = AcceptedMemoryUseReceipt {
        accepted_turn: accepted_turn + 1,
        ..accepted.clone()
    };
    assert!(owner.record_accepted_memory_use(wrong_cut, 1).is_err());
    let unknown_memory = AcceptedMemoryUseReceipt {
        memories: vec![left.id, MemoryId([0xff; 32])],
        ..accepted.clone()
    };
    assert!(owner.record_accepted_memory_use(unknown_memory, 2).is_err());
    assert_eq!(fs::read(&path).unwrap(), clean);
    assert_eq!(owner.latest_global_version(), version);
    assert_eq!(owner.freshness_record(left.id), Some(initial));

    assert_eq!(
        owner
            .record_accepted_memory_use(accepted.clone(), 1)
            .unwrap(),
        1
    );
    assert_eq!(owner.latest_global_version(), version);
    let accepted_record = owner.freshness_record(left.id).unwrap();
    assert_eq!(
        accepted_record.score, 90,
        "350 decay turns then one accepted +25"
    );
    assert_ne!(accepted_record, initial);
    let accepted_bytes = fs::read(&path).unwrap();

    // A genuine retry is resolved by the original receipt *before* consulting
    // the changed topology or recalculating event candidates.
    owner
        .set_memory_relation(
            left.id,
            right.id,
            GraphRelationKind::Factual,
            true,
            owner.graph_version(),
        )
        .unwrap();
    let after_graph = fs::read(&path).unwrap();
    assert_ne!(after_graph, accepted_bytes);
    assert_eq!(
        owner
            .record_accepted_memory_use(accepted.clone(), 3)
            .unwrap(),
        0
    );
    assert_eq!(fs::read(&path).unwrap(), after_graph);
    assert_eq!(owner.freshness_record(left.id).unwrap(), accepted_record);

    // Same source identity with a different accepted cut or source set is a
    // collision, not an opportunity to reinforce another Memory.
    let conflict_turn = AcceptedMemoryUseReceipt {
        accepted_turn: 0,
        ..accepted.clone()
    };
    let conflict_source = AcceptedMemoryUseReceipt {
        memories: vec![right.id],
        ..accepted.clone()
    };
    assert!(owner.record_accepted_memory_use(conflict_turn, 1).is_err());
    assert!(
        owner
            .record_accepted_memory_use(conflict_source, 1)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), after_graph);
    drop(owner);

    let mut reopened = Cva::open(&path).unwrap();
    let after_open = fs::read(&path).unwrap();
    assert_eq!(
        after_open, after_graph,
        "read-only open must not fabricate a receipt"
    );
    assert_eq!(reopened.record_accepted_memory_use(accepted, 1).unwrap(), 0);
    assert_eq!(fs::read(&path).unwrap(), after_open);
    assert_eq!(reopened.freshness_record(left.id).unwrap(), accepted_record);
}
